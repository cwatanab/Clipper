use arboard::Clipboard;

use crate::darkmode;
use crate::state::{self, SafeHWND, SafeWndProc, SafeHBRUSH, SafeHFONT, MIGEMO_DICT, APP_STATE, BRUSH_BG, BRUSH_CTRL, EDIT_HWND, FONT_EDIT, FONT_LISTBOX, LISTBOX_HWND, OLD_EDIT_PROC, WM_CLIPBOARD_CHANGED, WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET};
use crate::state::Mode;
use crate::ui;
use crate::util;
use crate::win32;

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn edit_subclass_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    if msg == win32::WM_KEYDOWN {
        state::log_debug(&format!("Edit KeyDown: vk={}", wparam));
        match wparam {
            38 => { ui::move_listbox_selection(-1); return 0; }
            40 => { ui::move_listbox_selection(1); return 0; }
            13 => { ui::on_select(); return 0; }
            27 => { ui::hide_window(); return 0; }
            _ => {}
        }

        let ctrl_pressed = (unsafe { win32::GetKeyState(0x11) } & 0x8000u16 as i16) != 0;
        if ctrl_pressed {
            match wparam {
                0x4E | 0x4A => { ui::move_listbox_selection(1); return 0; }
                0x50 | 0x4B => { ui::move_listbox_selection(-1); return 0; }
                _ => {}
            }
        }
    }

    let old_proc_opt = OLD_EDIT_PROC.get();
    if let Some(SafeWndProc(old_proc)) = old_proc_opt {
        unsafe { old_proc(hwnd, msg, wparam, lparam) }
    } else {
        unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn window_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    match msg {
        win32::WM_CREATE => {
            state::log_debug("WM_CREATE message received.");
            let hinstance = unsafe { win32::GetModuleHandleW(std::ptr::null()) };

            let hwnd_edit = unsafe {
                win32::CreateWindowExW(
                    win32::WS_EX_CLIENTEDGE,
                    util::to_wstring("EDIT").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::ES_AUTOHSCROLL | win32::ES_LEFT | 0x0004,
                    0, 0, 0, 0,
                    hwnd,
                    101 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            state::log_debug(&format!("Edit control created: {:?}", hwnd_edit));

            unsafe { win32::ImmAssociateContext(hwnd_edit, std::ptr::null_mut()) };

            let hwnd_listbox = unsafe {
                win32::CreateWindowExW(
                    win32::WS_EX_CLIENTEDGE,
                    util::to_wstring("LISTBOX").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::WS_VSCROLL | win32::LBS_NOTIFY | win32::LBS_HASSTRINGS | win32::LBS_NOINTEGRALHEIGHT,
                    0, 0, 0, 0,
                    hwnd,
                    102 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            state::log_debug(&format!("ListBox control created: {:?}", hwnd_listbox));

            darkmode::apply_to_control(hwnd_edit);
            darkmode::apply_to_control(hwnd_listbox);

            let font_edit = util::create_ui_font("Segoe UI", -18);
            let font_listbox = util::create_ui_font("Segoe UI", -16);

            let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut rc) };
            let cw = rc.right - rc.left;
            let ch = rc.bottom - rc.top;
            let margin = 4;
            let edit_h = 28;
            let gap = 4;

            unsafe {
                win32::MoveWindow(hwnd_edit, margin, margin, cw - margin * 2, edit_h, 1);
                win32::MoveWindow(hwnd_listbox, margin, margin + edit_h + gap, cw - margin * 2, ch - margin * 2 - edit_h - gap, 1);
                win32::SendMessageW(hwnd_edit, win32::WM_SETFONT, font_edit as win32::WPARAM, 1);
                win32::SendMessageW(hwnd_listbox, win32::WM_SETFONT, font_listbox as win32::WPARAM, 1);

                let old_proc = win32::SetWindowLongPtrW(hwnd_edit, win32::GWLP_WNDPROC, edit_subclass_proc as *const () as win32::LONG_PTR);
                let _ = OLD_EDIT_PROC.set(SafeWndProc(std::mem::transmute(old_proc)));
                state::log_debug(&format!("Edit subclass applied. Old proc: {:?}", old_proc));

                let brush_bg = win32::GetSysColorBrush(win32::COLOR_3DFACE as i32);
                let brush_ctrl = win32::GetSysColorBrush(win32::COLOR_WINDOW as i32);

                let _ = EDIT_HWND.set(SafeHWND(hwnd_edit));
                let _ = LISTBOX_HWND.set(SafeHWND(hwnd_listbox));
                let _ = BRUSH_BG.set(SafeHBRUSH(brush_bg));
                let _ = BRUSH_CTRL.set(SafeHBRUSH(brush_ctrl));
                let _ = FONT_EDIT.set(SafeHFONT(font_edit));
                let _ = FONT_LISTBOX.set(SafeHFONT(font_listbox));

                state::log_debug("CONTROLS successfully stored inside OnceLocks.");
            }
        }
        win32::WM_COMMAND => {
            let ctrl_id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if ctrl_id == 101 && code == win32::EN_CHANGE as usize {
                ui::update_listbox_items(MIGEMO_DICT.get());
            } else if ctrl_id == 102 {
                if code == 2 { // LBN_DBLCLK
                    ui::on_select();
                }
            }
        }
        win32::WM_ACTIVATE => {
            state::log_debug(&format!("WM_ACTIVATE: wparam={}", wparam));
            if wparam == win32::WA_INACTIVE {
                let is_visible = {
                    let state_guard = APP_STATE.lock().unwrap();
                    state_guard.as_ref().map_or(false, |s| s.visible)
                };
                if is_visible {
                    state::log_debug("Window inactive, hiding window...");
                    ui::hide_window();
                }
            } else {
                if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                    unsafe { win32::SetFocus(*hwnd_edit) };
                    state::log_debug("SetFocus called on Edit control.");
                }
            }
        }
        win32::WM_TRAYICON => {
            if lparam == win32::WM_RBUTTONUP as win32::LPARAM {
                ui::show_tray_menu(hwnd);
            } else if lparam == win32::WM_LBUTTONDBLCLK as win32::LPARAM {
                ui::trigger_app(Mode::Snippet, std::ptr::null_mut());
            }
        }
        WM_TRIGGER_SNIPPET => {
            let active_hwnd = wparam as win32::HWND;
            ui::trigger_app(Mode::Snippet, active_hwnd);
        }
        WM_TRIGGER_HISTORY => {
            let active_hwnd = wparam as win32::HWND;
            ui::trigger_app(Mode::History, active_hwnd);
        }
        WM_CLIPBOARD_CHANGED => {
            let mut clipboard = Clipboard::new().unwrap();
            if let Ok(text) = clipboard.get_text() {
                let mut state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &mut *state_guard {
                    state.last_clipboard_value = text.clone();
                    if let Some(pos) = state.history.iter().position(|x| x == &text) {
                        state.history.remove(pos);
                    }
                    state.history.push_front(text);
                    if state.history.len() > 100 {
                        state.history.pop_back();
                    }
                    util::save_history(&state.history);
                }
            }
        }
        win32::WM_DESTROY => {
            if let Some(SafeHFONT(font)) = FONT_EDIT.get() {
                unsafe { win32::DeleteObject(*font) };
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX.get() {
                unsafe { win32::DeleteObject(*font) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_BG.get() {
                unsafe { win32::DeleteObject(*brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_CTRL.get() {
                unsafe { win32::DeleteObject(*brush) };
            }
            unsafe { win32::PostQuitMessage(0) };
        }
        _ => return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}
