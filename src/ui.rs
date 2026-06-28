use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use rustmigemo::migemo::compact_dictionary::CompactDictionary;

use crate::filter;
use crate::state::{self, Mode, SafeHWND, MIGEMO_DICT, APP_STATE, EDIT_HWND, LISTBOX_HWND, MAIN_HWND};
use crate::util;
use crate::win32;

pub fn update_listbox_items(dict_opt: Option<&CompactDictionary>) {
    if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
        let len = unsafe { win32::GetWindowTextLengthW(*hwnd_edit) } as usize;
        let mut buf = vec![0u16; len + 1];
        unsafe { win32::GetWindowTextW(*hwnd_edit, buf.as_mut_ptr(), (len + 1) as i32) };
        let query_text = String::from_utf16_lossy(&buf[..len]);

        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            let filtered = filter::filter_items(&query_text, state, dict_opt);
            state.current_results = filtered.clone();
            std::mem::drop(state_guard);

            unsafe {
                win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                for item in &filtered {
                    let item_w = util::to_wstring(item);
                    win32::SendMessageW(*hwnd_listbox, win32::LB_ADDSTRING, 0, item_w.as_ptr() as win32::LPARAM);
                }
                if !filtered.is_empty() {
                    win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, 0, 0);
                }
            }
        }
    }
}

pub fn move_listbox_selection(dir: i32) {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        let count = unsafe { win32::SendMessageW(*hwnd_listbox, 0x018B /* LB_GETCOUNT */, 0, 0) } as isize;
        if count > 0 {
            let dir_isize = dir as isize;
            let next = if cur == win32::LB_ERR {
                if dir_isize > 0 { 0 } else { count - 1 }
            } else {
                (cur + dir_isize + count) % count
            };
            unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, next as usize, 0) };
        }
    }
}

pub fn on_select() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        state::log_debug(&format!("on_select: cur={}", cur));
        if cur != win32::LB_ERR {
            let len = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETTEXTLEN, cur as usize, 0) } as usize;
            let mut buf = vec![0u16; len + 1];
            unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETTEXT, cur as usize, buf.as_mut_ptr() as win32::LPARAM) };

            let selected_text = String::from_utf16_lossy(&buf[..len]);
            state::log_debug(&format!("on_select selected text: {}", selected_text));

            let mut final_text = selected_text.clone();
            let mut last_active = None;
            {
                let mut state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &mut *state_guard {
                    if state.mode == Mode::Snippet {
                        if let Some((_, template)) = state.snippets.iter().find(|(name, _)| name == &selected_text) {
                            final_text = util::render_template(template, &state.last_clipboard_value);
                        }
                    }
                    last_active = state.last_active_window;
                }
            }

            state::log_debug(&format!("on_select final text: {}", final_text));

            let mut success = false;
            for _ in 0..10 {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.set_text(final_text.clone()).is_ok() {
                        success = true;
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
            state::log_debug(&format!("Clipboard write success: {}", success));

            restore_focus(last_active);
            hide_window();
            simulate_paste();
        }
    }
}

pub fn trigger_app(mode: Mode, active_hwnd: win32::HWND) {
    state::log_debug(&format!("trigger_app called. Mode={:?}, active_hwnd={:?}", mode, active_hwnd));

    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.mode = mode;
            if mode == Mode::Snippet {
                state.snippets = util::load_snippets();
            }
            state.visible = true;

            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                if !active_hwnd.is_null() && active_hwnd != *hwnd_main {
                    state.last_active_window = Some(active_hwnd as usize);
                } else {
                    let cur_active = unsafe { win32::GetForegroundWindow() };
                    if cur_active != *hwnd_main {
                        state.last_active_window = Some(cur_active as usize);
                    }
                }
            }
        }
    }

    if let (Some(SafeHWND(hwnd_main)), Some(SafeHWND(hwnd_edit))) = (MAIN_HWND.get(), EDIT_HWND.get()) {
        let monitor_w = unsafe { win32::GetSystemMetrics(0) };
        let monitor_h = unsafe { win32::GetSystemMetrics(1) };
        let w = 400;
        let h = 300;

        let (mut x, mut y) = ((monitor_w - w) / 2, (monitor_h - h) / 2);
        if !active_hwnd.is_null() {
            let tid = unsafe { win32::GetWindowThreadProcessId(active_hwnd, std::ptr::null_mut()) };
            let mut gui: win32::GUITHREADINFO = unsafe { std::mem::zeroed() };
            gui.cbSize = std::mem::size_of::<win32::GUITHREADINFO>() as u32;
            if unsafe { win32::GetGUIThreadInfo(tid, &mut gui) } != 0 && !gui.hwndCaret.is_null() {
                let mut pt = win32::POINT { x: gui.rcCaret.left, y: gui.rcCaret.bottom };
                unsafe { win32::ClientToScreen(gui.hwndCaret, &mut pt) };
                x = pt.x;
                y = pt.y + 4;
            }
        }

        if x + w > monitor_w { x = monitor_w - w; }
        if y + h > monitor_h { y = monitor_h - h; }
        if x < 0 { x = 0; }
        if y < 0 { y = 0; }

        unsafe {
            win32::SetWindowPos(*hwnd_main, std::ptr::null_mut(), x, y, w, h, 0x0040 /* SWP_SHOWWINDOW */);

            let foreground = win32::GetForegroundWindow();
            if !foreground.is_null() && foreground != *hwnd_main {
                let cur_thread = win32::GetCurrentThreadId();
                let fg_thread = win32::GetWindowThreadProcessId(foreground, std::ptr::null_mut());
                win32::AttachThreadInput(cur_thread, fg_thread, 1);
                win32::SetForegroundWindow(*hwnd_main);
                win32::AttachThreadInput(cur_thread, fg_thread, 0);
            } else {
                win32::SetForegroundWindow(*hwnd_main);
            }

            win32::SetWindowTextW(*hwnd_edit, util::to_wstring("").as_ptr());
            win32::SetFocus(*hwnd_edit);
            win32::ImmAssociateContext(*hwnd_edit, std::ptr::null_mut());
        }
        update_listbox_items(MIGEMO_DICT.get());
    }
}

pub fn hide_window() {
    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.visible = false;
        }
    }
    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
        unsafe { win32::ShowWindow(*hwnd_main, 0) };
    }
}

pub fn restore_focus(last_active_window: Option<usize>) {
    if let Some(hwnd_usize) = last_active_window {
        let hwnd = hwnd_usize as win32::HWND;
        if unsafe { win32::IsWindow(hwnd) } != 0 {
            unsafe { win32::SetForegroundWindow(hwnd) };
        }
    }
}

pub fn simulate_paste() {
    thread::sleep(Duration::from_millis(150));
    let inputs = [
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_CONTROL, w_scan: 0, dw_flags: 0, time: 0, dw_extra_info: 0,
                }
            }
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V, w_scan: 0, dw_flags: 0, time: 0, dw_extra_info: 0,
                }
            }
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V, w_scan: 0, dw_flags: win32::KEYEVENTF_KEYUP, time: 0, dw_extra_info: 0,
                }
            }
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_CONTROL, w_scan: 0, dw_flags: win32::KEYEVENTF_KEYUP, time: 0, dw_extra_info: 0,
                }
            }
        }
    ];
    unsafe { win32::SendInput(4, inputs.as_ptr(), std::mem::size_of::<win32::INPUT>() as i32) };
}

pub fn show_tray_menu(hwnd: win32::HWND) {
    let mut pt = win32::POINT { x: 0, y: 0 };
    unsafe { win32::GetCursorPos(&mut pt) };

    let menu = unsafe { win32::CreatePopupMenu() };
    unsafe {
        win32::AppendMenuW(menu, 0, 1001, util::to_wstring("Show Snippets (Shift x2)").as_ptr());
        win32::AppendMenuW(menu, 0, 1002, util::to_wstring("Show History (Ctrl x2)").as_ptr());
        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());
        win32::AppendMenuW(menu, 0, 1003, util::to_wstring("Exit").as_ptr());
        win32::SetForegroundWindow(hwnd);
    };

    let cmd = unsafe {
        win32::TrackPopupMenu(
            menu, win32::TPM_RETURNCMD | win32::TPM_LEFTALIGN,
            pt.x, pt.y, 0, hwnd, std::ptr::null(),
        )
    };
    unsafe { win32::DestroyMenu(menu) };

    if cmd == 1001 {
        trigger_app(Mode::Snippet, std::ptr::null_mut());
    } else if cmd == 1002 {
        trigger_app(Mode::History, std::ptr::null_mut());
    } else if cmd == 1003 {
        unsafe { win32::PostQuitMessage(0) };
    }
}
