use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicU32, Ordering};

use arboard::Clipboard;
use crate::filter;
use crate::state::{self, Mode, SafeHWND, APP_STATE, EDIT_HWND, LISTBOX_HWND, MAIN_HWND};
use crate::util;
use crate::win32;

static FILTER_GEN: AtomicU32 = AtomicU32::new(0);

pub fn update_listbox_items() {
    if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(_hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
        let len = unsafe { win32::GetWindowTextLengthW(*hwnd_edit) } as usize;
        let mut buf = vec![0u16; len + 1];
        unsafe { win32::GetWindowTextW(*hwnd_edit, buf.as_mut_ptr(), (len + 1) as i32) };
        let query_text = String::from_utf16_lossy(&buf[..len]);

        let generation = FILTER_GEN.fetch_add(1, Ordering::SeqCst) + 1;

        let mut mode = Mode::Snippet;
        let mut current_folder = String::new();
        let mut snippets = std::sync::Arc::new(Vec::new());
        let mut history = std::sync::Arc::new(std::collections::VecDeque::new());

        {
            let mut state_guard = APP_STATE.lock().unwrap();
            if let Some(state) = &mut *state_guard {
                state.filter_generation = generation;
                mode = state.mode;
                current_folder = state.current_folder.clone();
                snippets = std::sync::Arc::clone(&state.snippets);
                history = std::sync::Arc::clone(&state.history);
            }
        }

        thread::spawn(move || {
            // Debounce delay to let fast typing complete
            thread::sleep(Duration::from_millis(50));

            if FILTER_GEN.load(Ordering::SeqCst) != generation {
                return; // Newer input arrived
            }

            // Create thread-local state copy for filtering without blocking main AppState mutex
            let temp_state = crate::state::AppState {
                history,
                snippets,
                mode,
                visible: true,
                current_results: Vec::new(),
                current_full_paths: Vec::new(),
                last_clipboard_value: String::new(),
                last_active_window: None,
                is_dark: false,
                current_folder,
                top_index: 0,
                filter_generation: generation,
            };

            let dict = state::get_migemo_dict();
            let (display_items, full_paths) = filter::filter_items(&query_text, &temp_state, dict.as_deref());

            if FILTER_GEN.load(Ordering::SeqCst) != generation {
                return; // Newer input arrived
            }

            let mut state_guard = APP_STATE.lock().unwrap();
            if let Some(state) = &mut *state_guard {
                if state.filter_generation == generation {
                    state.current_results = display_items;
                    state.current_full_paths = full_paths;

                    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                        unsafe {
                            win32::PostMessageW(*hwnd_main, crate::state::WM_FILTER_COMPLETE, generation as usize, 0);
                        }
                    }
                }
            }
        });
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
            crate::wndproc::update_top_index();
        }
    }
}

pub fn on_select() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        state::log_debug(&format!("on_select: cur={}", cur));
        if cur != win32::LB_ERR {
            let mut target_path = String::new();
            let mut mode = Mode::Snippet;
            
            {
                let state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &*state_guard {
                    mode = state.mode;
                    if (cur as usize) < state.current_full_paths.len() {
                        target_path = state.current_full_paths[cur as usize].clone();
                    }
                }
            }

            if mode == Mode::Snippet {
                if target_path == ".." {
                    // Go up to parent folder
                    let mut state_guard = APP_STATE.lock().unwrap();
                    if let Some(state) = &mut *state_guard {
                        if let Some(pos) = state.current_folder.rfind('/') {
                            state.current_folder = state.current_folder[..pos].to_string();
                        } else {
                            state.current_folder.clear();
                        }
                    }
                    std::mem::drop(state_guard);
                    
                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, util::to_wstring("").as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    update_listbox_items();
                    update_search_cue_banner();
                    return;
                } else if target_path.starts_with("dir:") {
                    // Enter subfolder
                    let folder = target_path["dir:".len()..].to_string();
                    let mut state_guard = APP_STATE.lock().unwrap();
                    if let Some(state) = &mut *state_guard {
                        state.current_folder = folder;
                    }
                    std::mem::drop(state_guard);
                    
                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, util::to_wstring("").as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    update_listbox_items();
                    update_search_cue_banner();
                    return;
                }
            }

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
                        if let Some((_, template)) = state.snippets.iter().find(|(name, _)| name == &target_path) {
                            final_text = util::render_template(template, &state.last_clipboard_value);
                        }
                    } else {
                        final_text = target_path.clone();
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

pub fn delete_selected_item() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        if cur != win32::LB_ERR {
            let mut target_text = String::new();
            let mut is_history = false;
            
            {
                let state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &*state_guard {
                    is_history = state.mode == Mode::History;
                    if is_history && (cur as usize) < state.current_full_paths.len() {
                        target_text = state.current_full_paths[cur as usize].clone();
                    }
                }
            }

            if is_history && !target_text.is_empty() {
                let mut state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &mut *state_guard {
                    let history = std::sync::Arc::make_mut(&mut state.history);
                    if let Some(pos) = history.iter().position(|x| x == &target_text) {
                        history.remove(pos);
                        util::save_history(history);
                    }
                }
                std::mem::drop(state_guard);
                
                update_listbox_items();
                
                // Keep the selection at the same position or move it up if we deleted the last item
                let count = unsafe { win32::SendMessageW(*hwnd_listbox, 0x018B /* LB_GETCOUNT */, 0, 0) } as isize;
                if count > 0 {
                    let next_sel = std::cmp::min(cur, count - 1);
                    unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, next_sel as usize, 0) };
                }
                crate::wndproc::update_top_index();
            }
        }
    }
}

pub fn trigger_app(mode: Mode, active_hwnd: win32::HWND) {
    state::log_debug(&format!("trigger_app called. Mode={:?}, active_hwnd={:?}", mode, active_hwnd));

    let is_dark = crate::darkmode::is_dark_mode();

    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.mode = mode;
            state.is_dark = is_dark;
            if mode == Mode::Snippet {
                state.snippets = std::sync::Arc::new(util::load_snippets());
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
        crate::wndproc::update_theme_resources(*hwnd_main, is_dark);

        let monitor_w = unsafe { win32::GetSystemMetrics(0) };
        let monitor_h = unsafe { win32::GetSystemMetrics(1) };
        let w = 450; // Increased width slightly as requested
        let max_rows = state::CONFIG.get().map_or(15, |c| c.max_rows);
        let h = (max_rows as i32) * 30 + 43;

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
            } else {
                // Fallback: use mouse cursor position when caret cannot be retrieved (e.g. in terminals)
                let mut pt = win32::POINT { x: 0, y: 0 };
                unsafe { win32::GetCursorPos(&mut pt) };
                x = pt.x;
                y = pt.y + 10;
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
        update_listbox_items();
        update_search_cue_banner();
    }
}

pub fn hide_window() {
    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.visible = false;
            // Clear search result lists to free up memory immediately
            state.current_results.clear();
            state.current_full_paths.clear();
        }
    }
    
    // Clear Migemo dictionary from memory
    state::clear_migemo_dict();

    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
        unsafe { win32::ShowWindow(*hwnd_main, 0) };
    }

    // Empty working set memory on Windows to trim resource footprint
    #[cfg(target_os = "windows")]
    unsafe {
        let handle = win32::GetCurrentProcess();
        win32::SetProcessWorkingSetSize(handle, !0, !0);
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
        // App title header (disabled)
        win32::AppendMenuW(menu, 0x0001 | 0x0002, 0, util::to_wstring("Clipper").as_ptr());
        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());

        // Basic actions
        win32::AppendMenuW(menu, 0, 1001, util::to_wstring("スニペットを表示 (Shift連打)").as_ptr());
        win32::AppendMenuW(menu, 0, 1002, util::to_wstring("クリップボード履歴を表示 (Ctrl連打)").as_ptr());
        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());

        // Utility actions
        win32::AppendMenuW(menu, 0, 1004, util::to_wstring("設定ファイルを開く").as_ptr());
        win32::AppendMenuW(menu, 0, 1005, util::to_wstring("スニペットフォルダを開く").as_ptr());
        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());

        // Exit
        win32::AppendMenuW(menu, 0, 1003, util::to_wstring("終了").as_ptr());
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
    } else if cmd == 1004 {
        let path = crate::config::Config::get_path();
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    } else if cmd == 1005 {
        let path = util::get_app_dir().join("snippets");
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    } else if cmd == 1003 {
        unsafe { win32::PostQuitMessage(0) };
    }
}

pub fn update_search_cue_banner() {
    if let (Some(SafeHWND(hwnd_edit)), Some(state_guard)) = (EDIT_HWND.get(), APP_STATE.lock().unwrap().as_ref()) {
        let cue_text = match state_guard.mode {
            Mode::Snippet => {
                if state_guard.current_folder.is_empty() {
                    "スニペット (Root) - 検索 (Migemo)...".to_string()
                } else {
                    format!("スニペット [{}] - 検索 (Migemo)...", state_guard.current_folder)
                }
            }
            Mode::History => "クリップボード履歴 - 検索 (Migemo)...".to_string(),
        };
        let cue_w = util::to_wstring(&cue_text);
        unsafe {
            win32::SendMessageW(*hwnd_edit, 0x1501 /* EM_SETCUEBANNER */, 1, cue_w.as_ptr() as win32::LPARAM);
        }
    }
}
