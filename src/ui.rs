use crate::filter;
use crate::state::{self, EDIT_HWND, LISTBOX_HWND, MAIN_HWND, Mode, SafeHWND, lock_state};
use crate::util;
use crate::win32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

static FILTER_GEN: AtomicU32 = AtomicU32::new(0);
static EMPTY_WSTR: &[u16] = &[0u16];

pub fn update_listbox_items() {
    if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(_hwnd_listbox))) =
        (EDIT_HWND.get(), LISTBOX_HWND.get())
    {
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
            let mut state_guard = lock_state();
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
                current_selection: String::new(),
                last_active_window: None,
                is_dark: false,
                current_folder,
                top_index: 0,
                filter_generation: generation,
            };

            let dict = state::get_migemo_dict();
            let (display_items, full_paths) =
                filter::filter_items(&query_text, &temp_state, dict.as_deref());

            if FILTER_GEN.load(Ordering::SeqCst) != generation {
                return; // Newer input arrived
            }

            let mut state_guard = lock_state();
            if let Some(state) = &mut *state_guard
                && state.filter_generation == generation
            {
                state.current_results = display_items;
                state.current_full_paths = full_paths;

                if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                    unsafe {
                        win32::PostMessageW(
                            *hwnd_main,
                            crate::state::WM_FILTER_COMPLETE,
                            generation as usize,
                            0,
                        );
                    }
                }
            }
        });
    }
}

pub fn move_listbox_selection(dir: i32) {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        let count = unsafe {
            win32::SendMessageW(*hwnd_listbox, 0x018B /* LB_GETCOUNT */, 0, 0)
        } as isize;
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
                let state_guard = lock_state();
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
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        let mut parts = util::split_path(&state.current_folder);
                        if parts.len() > 1 {
                            parts.pop();
                            state.current_folder = util::join_path(&parts);
                        } else {
                            state.current_folder.clear();
                        }
                    }
                    std::mem::drop(state_guard);

                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, EMPTY_WSTR.as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    update_listbox_items();
                    update_search_cue_banner();
                    return;
                } else if target_path.starts_with("dir:") {
                    // Enter subfolder
                    let folder = target_path["dir:".len()..].to_string();
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        state.current_folder = folder;
                    }
                    std::mem::drop(state_guard);

                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, EMPTY_WSTR.as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    update_listbox_items();
                    update_search_cue_banner();
                    return;
                }
            }

            let len = unsafe {
                win32::SendMessageW(*hwnd_listbox, win32::LB_GETTEXTLEN, cur as usize, 0)
            } as usize;
            let mut buf = vec![0u16; len + 1];
            unsafe {
                win32::SendMessageW(
                    *hwnd_listbox,
                    win32::LB_GETTEXT,
                    cur as usize,
                    buf.as_mut_ptr() as win32::LPARAM,
                )
            };

            let selected_text = String::from_utf16_lossy(&buf[..len]);
            state::log_debug(&format!("on_select selected text: {}", selected_text));

            let mut final_text = selected_text.clone();
            {
                let mut state_guard = lock_state();
                if let Some(state) = &mut *state_guard {
                    if state.mode == Mode::Snippet {
                        if let Some((_, template)) =
                            state.snippets.iter().find(|(name, _)| name == &target_path)
                        {
                            final_text = util::render_template(template, &state.current_selection);
                        }
                    } else {
                        final_text = target_path.clone();
                    }
                }
            }

            state::log_debug(&format!("on_select final text: {}", final_text));

            let mut success = false;
            for _ in 0..10 {
                if util::set_clipboard_text(&final_text) {
                    success = true;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            state::log_debug(&format!("Clipboard write success: {}", success));

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
                let state_guard = lock_state();
                if let Some(state) = &*state_guard {
                    is_history = state.mode == Mode::History;
                    if is_history && (cur as usize) < state.current_full_paths.len() {
                        target_text = state.current_full_paths[cur as usize].clone();
                    }
                }
            }

            if is_history && !target_text.is_empty() {
                let mut state_guard = lock_state();
                if let Some(state) = &mut *state_guard {
                    let history = std::sync::Arc::make_mut(&mut state.history);
                    if let Some(pos) = history.iter().position(|x| x == &target_text) {
                        history.remove(pos);
                        if state::SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed) {
                            util::save_history(history);
                        }
                    }
                }
                std::mem::drop(state_guard);

                update_listbox_items();

                // Keep the selection at the same position or move it up if we deleted the last item
                let count = unsafe {
                    win32::SendMessageW(*hwnd_listbox, 0x018B /* LB_GETCOUNT */, 0, 0)
                } as isize;
                if count > 0 {
                    let next_sel = std::cmp::min(cur, count - 1);
                    unsafe {
                        win32::SendMessageW(
                            *hwnd_listbox,
                            win32::LB_SETCURSEL,
                            next_sel as usize,
                            0,
                        )
                    };
                }
                crate::wndproc::update_top_index();
            }
        }
    }
}

pub fn trigger_app(mode: Mode, active_hwnd: win32::HWND) {
    state::log_debug(&format!(
        "trigger_app called. Mode={:?}, active_hwnd={:?}",
        mode, active_hwnd
    ));

    let is_dark = crate::darkmode::is_dark_active();

    let selection = util::get_clipboard_text().unwrap_or_default();

    {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.mode = mode;
            state.is_dark = is_dark;
            state.current_selection = selection;
            if mode == Mode::Snippet {
                state.snippets = std::sync::Arc::new(util::load_snippets());
            }
            state.visible = true;
            state::LAST_SHOW_TIME.store(
                unsafe { win32::GetTickCount() },
                std::sync::atomic::Ordering::Relaxed,
            );

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

    if let (Some(SafeHWND(hwnd_main)), Some(SafeHWND(hwnd_edit))) =
        (MAIN_HWND.get(), EDIT_HWND.get())
    {
        let target_hwnd = if !active_hwnd.is_null() {
            active_hwnd
        } else {
            *hwnd_main
        };
        let scale = if !target_hwnd.is_null() {
            let dpi = unsafe { win32::GetDpiForWindow(target_hwnd) };
            if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 }
        } else {
            let dpi = unsafe { win32::GetDpiForWindow(*hwnd_main) };
            dpi as f32 / 96.0
        };

        let base_width = state::CONFIG.get().map_or(380.0, |c| c.width);
        let w = (base_width * scale) as i32; // Width with generous margins (scaled)
        let max_rows = state::CONFIG.get().map_or(15, |c| c.max_rows);
        let item_h = (26.0 * scale) as i32;
        let base_h = (52.0 * scale) as i32;
        let h = (max_rows as i32) * item_h + base_h;

        let mut work_rect = win32::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut got_monitor = false;

        if !target_hwnd.is_null() {
            unsafe {
                let h_monitor =
                    win32::MonitorFromWindow(target_hwnd, win32::MONITOR_DEFAULTTONEAREST);
                if !h_monitor.is_null() {
                    let mut mi: win32::MONITORINFO = std::mem::zeroed();
                    mi.cbSize = std::mem::size_of::<win32::MONITORINFO>() as u32;
                    if win32::GetMonitorInfoW(h_monitor, &mut mi) != 0 {
                        work_rect = mi.rcWork;
                        got_monitor = true;
                    }
                }
            }
        }

        if !got_monitor {
            let monitor_w = unsafe { win32::GetSystemMetrics(0) };
            let monitor_h = unsafe { win32::GetSystemMetrics(1) };
            work_rect = win32::RECT {
                left: 0,
                top: 0,
                right: monitor_w,
                bottom: monitor_h,
            };
        }

        let (mut x, mut y) = (
            work_rect.left + (work_rect.right - work_rect.left - w) / 2,
            work_rect.top + (work_rect.bottom - work_rect.top - h) / 2,
        );

        if !active_hwnd.is_null() {
            let tid = unsafe { win32::GetWindowThreadProcessId(active_hwnd, std::ptr::null_mut()) };
            let mut gui: win32::GUITHREADINFO = unsafe { std::mem::zeroed() };
            gui.cbSize = std::mem::size_of::<win32::GUITHREADINFO>() as u32;
            let gui_ok = unsafe { win32::GetGUIThreadInfo(tid, &mut gui) } != 0;

            if gui_ok && !gui.hwndCaret.is_null() {
                // 1. Standard Win32 caret API
                let mut pt = win32::POINT {
                    x: gui.rcCaret.left,
                    y: gui.rcCaret.bottom,
                };
                unsafe { win32::ClientToScreen(gui.hwndCaret, &mut pt) };
                x = pt.x;
                y = pt.y + (4.0 * scale) as i32;
            } else {
                // 2. MSAA IAccessible fallback (works for some apps that don't use Win32 caret)
                let focus_hwnd = if gui_ok && !gui.hwndFocus.is_null() {
                    gui.hwndFocus
                } else {
                    active_hwnd
                };
                if let Some((cx, cy, _cw, ch)) = win32::get_caret_rect_accessible(focus_hwnd) {
                    x = cx;
                    y = cy + ch + (4.0 * scale) as i32;
                } else {
                    // 3. Mouse cursor position as last resort
                    let mut pt = win32::POINT { x: 0, y: 0 };
                    unsafe { win32::GetCursorPos(&mut pt) };
                    x = pt.x;
                    y = pt.y + (10.0 * scale) as i32;
                }
            }
        }

        if x + w > work_rect.right {
            x = work_rect.right - w;
        }
        if y + h > work_rect.bottom {
            y = work_rect.bottom - h;
        }
        if x < work_rect.left {
            x = work_rect.left;
        }
        if y < work_rect.top {
            y = work_rect.top;
        }

        unsafe {
            win32::SetWindowTextW(*hwnd_edit, EMPTY_WSTR.as_ptr());

            let mut state_guard = lock_state();
            let display_items_for_listbox = if let Some(state) = &mut *state_guard {
                let generation = FILTER_GEN.fetch_add(1, Ordering::SeqCst) + 1;
                state.filter_generation = generation;

                let dict = state::get_migemo_dict();
                let (display_items, full_paths) = filter::filter_items("", state, dict.as_deref());
                state.current_results = display_items;
                state.current_full_paths = full_paths;
                Some(state.current_results.clone())
            } else {
                None
            };
            std::mem::drop(state_guard);

            if let Some(items) = display_items_for_listbox
                && let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get()
            {
                win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 0, 0);
                win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                for item in &items {
                    let item_w = util::to_wstring(item);
                    win32::SendMessageW(
                        *hwnd_listbox,
                        win32::LB_ADDSTRING,
                        0,
                        item_w.as_ptr() as win32::LPARAM,
                    );
                }
                if !items.is_empty() {
                    win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, 0, 0);
                }
                win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 1, 0);
                win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
            }

            win32::SetWindowPos(
                *hwnd_main,
                std::ptr::null_mut(),
                x,
                y,
                w,
                h,
                0x0040, /* SWP_SHOWWINDOW */
            );
            crate::wndproc::update_theme_resources(*hwnd_main, is_dark);

            let foreground = win32::GetForegroundWindow();
            if !foreground.is_null() && foreground != *hwnd_main {
                win32::SetForegroundWindow(*hwnd_main);

                let foreground_now = win32::GetForegroundWindow();
                if foreground_now != *hwnd_main {
                    // Alt key down & up trick to bypass SetForegroundWindow restrictions
                    let inputs = [
                        win32::INPUT {
                            r#type: win32::INPUT_KEYBOARD,
                            u: win32::INPUT_union {
                                ki: win32::KEYBDINPUT {
                                    w_vk: 18, // VK_MENU (ALT)
                                    w_scan: 0,
                                    dw_flags: 0,
                                    time: 0,
                                    dw_extra_info: 0,
                                },
                            },
                        },
                        win32::INPUT {
                            r#type: win32::INPUT_KEYBOARD,
                            u: win32::INPUT_union {
                                ki: win32::KEYBDINPUT {
                                    w_vk: 18, // VK_MENU (ALT)
                                    w_scan: 0,
                                    dw_flags: win32::KEYEVENTF_KEYUP,
                                    time: 0,
                                    dw_extra_info: 0,
                                },
                            },
                        },
                    ];
                    win32::SendInput(
                        2,
                        inputs.as_ptr(),
                        std::mem::size_of::<win32::INPUT>() as i32,
                    );
                    win32::SetForegroundWindow(*hwnd_main);
                }
            } else {
                win32::SetForegroundWindow(*hwnd_main);
            }

            win32::SetFocus(*hwnd_edit);
            win32::ImmAssociateContext(*hwnd_edit, std::ptr::null_mut());
        }
        update_search_cue_banner();
        crate::hook::install_mouse_hook();
    }
}

pub fn hide_window() {
    crate::hook::uninstall_mouse_hook();
    let mut last_active = None;
    {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.visible = false;
            // Clear search result lists to free up memory immediately
            state.current_results.clear();
            state.current_full_paths.clear();
            state.snippets = std::sync::Arc::new(Vec::new());

            // Capture and clear the last active window
            last_active = state.last_active_window.take();
        }
    }

    // Clear Migemo dictionary from memory
    state::clear_migemo_dict();

    // Restore focus to the last active window
    restore_focus(last_active);

    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
        unsafe { win32::ShowWindow(*hwnd_main, 0) };
    }

    // Trim the working set to free up physical memory immediately
    unsafe {
        let h_process = win32::GetCurrentProcess();
        win32::SetProcessWorkingSetSize(h_process, !0, !0);
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
                    w_vk: win32::VK_CONTROL,
                    w_scan: 0,
                    dw_flags: 0,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V,
                    w_scan: 0,
                    dw_flags: 0,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V,
                    w_scan: 0,
                    dw_flags: win32::KEYEVENTF_KEYUP,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_CONTROL,
                    w_scan: 0,
                    dw_flags: win32::KEYEVENTF_KEYUP,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        },
    ];
    unsafe {
        win32::SendInput(
            4,
            inputs.as_ptr(),
            std::mem::size_of::<win32::INPUT>() as i32,
        )
    };
}

pub fn show_tray_menu(hwnd: win32::HWND) {
    let mut pt = win32::POINT { x: 0, y: 0 };
    unsafe { win32::GetCursorPos(&mut pt) };

    let menu = unsafe { win32::CreatePopupMenu() };
    unsafe {
        // Utility actions
        win32::AppendMenuW(
            menu,
            0,
            1004,
            util::to_wstring("設定ファイルを開く").as_ptr(),
        );
        win32::AppendMenuW(
            menu,
            0,
            1005,
            util::to_wstring("スニペットフォルダを開く").as_ptr(),
        );
        win32::AppendMenuW(
            menu,
            0,
            1006,
            util::to_wstring("スニペットを再読み込み").as_ptr(),
        );
        win32::AppendMenuW(
            menu,
            0,
            1007,
            util::to_wstring("このアプリについて").as_ptr(),
        );

        // Checkbox item for saving history to file
        let is_save = state::SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed);
        let check_flag = if is_save {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_flag,
            1008,
            util::to_wstring("履歴をファイルに保存する").as_ptr(),
        );

        win32::AppendMenuW(
            menu,
            0,
            1009,
            util::to_wstring("履歴をクリア").as_ptr(),
        );

        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());

        // Exit
        win32::AppendMenuW(menu, 0, 1003, util::to_wstring("終了").as_ptr());
        win32::SetForegroundWindow(hwnd);
    };

    let cmd = unsafe {
        win32::TrackPopupMenu(
            menu,
            win32::TPM_RETURNCMD | win32::TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        )
    };
    unsafe { win32::DestroyMenu(menu) };

    if cmd == 1004 {
        let path = crate::config::Config::get_path();
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    } else if cmd == 1005 {
        let path = util::get_app_dir().join("snippets");
        let _ = std::fs::create_dir_all(&path);
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    } else if cmd == 1006 {
        let count = {
            let mut state_guard = lock_state();
            if let Some(state) = &mut *state_guard {
                let snippets = util::load_snippets();
                let len = snippets.len();
                state.snippets = std::sync::Arc::new(snippets);
                len
            } else {
                0
            }
        };
        let mut nid: win32::NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = win32::NIF_INFO;
        let title_w = util::to_wstring("Clipper");
        let title_len = std::cmp::min(title_w.len(), 63);
        nid.szInfoTitle[..title_len].copy_from_slice(&title_w[..title_len]);
        let msg_w = util::to_wstring(&format!("スニペットを再読み込みしました（{}件）", count));
        let msg_len = std::cmp::min(msg_w.len(), 255);
        nid.szInfo[..msg_len].copy_from_slice(&msg_w[..msg_len]);
        nid.dwInfoFlags = win32::NIIF_INFO;
        unsafe { win32::Shell_NotifyIconW(win32::NIM_MODIFY, &nid) };
    } else if cmd == 1007 {
        let version = env!("CARGO_PKG_VERSION");
        let title_w = util::to_wstring(&format!("Clipper v{}", version));
        let message_w = util::to_wstring(
            "Clipper - Snippet & Clipboard Manager\n\n\
            【簡単な使い方】\n\
            ・Shiftキーを2回連打: スニペット検索ウィンドウを表示\n\
            ・Ctrlキーを2回連打: クリップボード履歴ウィンドウを表示\n\n\
            ・候補選択: ↑ / ↓ または Ctrl+P / Ctrl+N\n\
            ・自動ペースト: Enterキー\n\
            ・閉じる: Escキー または ウィンドウ外をクリック",
        );
        unsafe {
            win32::MessageBoxW(
                hwnd,
                message_w.as_ptr(),
                title_w.as_ptr(),
                win32::MB_OK | win32::MB_ICONINFORMATION,
            );
        }
    } else if cmd == 1008 {
        let new_val = !state::SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed);
        state::SAVE_HISTORY_TO_FILE.store(new_val, std::sync::atomic::Ordering::Relaxed);

        // Update config.toml
        if let Some(config) = state::CONFIG.get() {
            let mut new_config = config.clone();
            new_config.save_history = new_val;
            new_config.save();
        }

        // If toggled to ON, immediately save current in-memory history to disk
        if new_val {
            let history_to_save = {
                let state_guard = lock_state();
                state_guard
                    .as_ref()
                    .map(|s| std::sync::Arc::clone(&s.history))
            };
            if let Some(history_arc) = history_to_save {
                util::save_history(&history_arc);
            }
        }
    } else if cmd == 1009 {
        let title_w = util::to_wstring("Clipper");
        let question_w = util::to_wstring("クリップボード履歴をすべてクリアしますか？\n（この操作は取り消せません）");
        let res = unsafe {
            win32::MessageBoxW(
                hwnd,
                question_w.as_ptr(),
                title_w.as_ptr(),
                win32::MB_YESNO | win32::MB_ICONQUESTION,
            )
        };
        if res == win32::IDYES {
            // Clear in-memory history
            let mut state_guard = lock_state();
            if let Some(state) = &mut *state_guard {
                state.history = std::sync::Arc::new(std::collections::VecDeque::new());
                state.current_results.clear();
                state.current_full_paths.clear();
            }
            std::mem::drop(state_guard);

            // Persist empty history to disk
            util::save_history(&std::collections::VecDeque::new());

            // Update UI listbox if visible
            update_listbox_items();

            // Display balloon notification
            let mut nid: win32::NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
            nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = win32::NIF_INFO;
            let title_w = util::to_wstring("Clipper");
            let title_len = std::cmp::min(title_w.len(), 63);
            nid.szInfoTitle[..title_len].copy_from_slice(&title_w[..title_len]);
            let msg_w = util::to_wstring("履歴をクリアしました");
            let msg_len = std::cmp::min(msg_w.len(), 255);
            nid.szInfo[..msg_len].copy_from_slice(&msg_w[..msg_len]);
            nid.dwInfoFlags = win32::NIIF_INFO;
            unsafe { win32::Shell_NotifyIconW(win32::NIM_MODIFY, &nid) };
        }
    } else if cmd == 1003 {
        unsafe { win32::PostQuitMessage(0) };
    }
}

pub fn update_search_cue_banner() {
    if let (Some(SafeHWND(hwnd_edit)), Some(state_guard)) = (EDIT_HWND.get(), lock_state().as_ref())
    {
        let cue_holder;
        let cue_text = match state_guard.mode {
            Mode::Snippet => {
                if state_guard.current_folder.is_empty() {
                    "スニペット (Root) - 検索 (Migemo)..."
                } else {
                    cue_holder = format!(
                        "スニペット [{}] - 検索 (Migemo)...",
                        state_guard.current_folder
                    );
                    &cue_holder
                }
            }
            Mode::History => "クリップボード履歴 - 検索 (Migemo)...",
        };
        let cue_w = util::to_wstring(cue_text);
        unsafe {
            win32::SendMessageW(
                *hwnd_edit,
                0x1501, /* EM_SETCUEBANNER */
                1,
                cue_w.as_ptr() as win32::LPARAM,
            );
        }
    }
}

pub fn show_notification(title: &str, message: &str, is_error: bool) {
    if let Some(SafeHWND(hwnd)) = MAIN_HWND.get() {
        let mut nid: win32::NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
        nid.hWnd = *hwnd;
        nid.uID = 1;
        nid.uFlags = win32::NIF_INFO;

        let title_w = util::to_wstring(title);
        let title_len = std::cmp::min(title_w.len(), 63);
        nid.szInfoTitle[..title_len].copy_from_slice(&title_w[..title_len]);

        let msg_w = util::to_wstring(message);
        let msg_len = std::cmp::min(msg_w.len(), 255);
        nid.szInfo[..msg_len].copy_from_slice(&msg_w[..msg_len]);

        nid.dwInfoFlags = if is_error {
            win32::NIIF_ERROR
        } else {
            win32::NIIF_INFO
        };
        unsafe { win32::Shell_NotifyIconW(win32::NIM_MODIFY, &nid) };
    }
}
