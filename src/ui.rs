use crate::filter;
use crate::state::{self, EDIT_HWND, LISTBOX_HWND, MAIN_HWND, Mode, SafeHWND, lock_state};
use crate::util;
use crate::win32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct UiMetrics {
    pub margin: i32,
    pub tab_bar_h: i32,
    pub edit_container_h: i32,
    pub edit_h: i32,
    pub item_h: i32,
    pub edit_container_y: i32,
    pub listbox_y: i32,
    pub listbox_h: i32,
    pub width: i32,
    pub height: i32,
}

impl UiMetrics {
    pub fn get(scale: f32) -> Self {
        let base_width = state::CONFIG.get().map_or(380.0, |c| c.width);
        let max_rows = state::CONFIG.get().map_or(16, |c| c.max_rows);
        let show_tabs = state::SHOW_TABS.load(Ordering::Relaxed);

        let margin = (6.0 * scale).round() as i32;
        let tab_bar_h = if show_tabs { (28.0 * scale).round() as i32 } else { 0 };
        let gap = (4.0 * scale).round() as i32;
        let edit_container_h = (34.0 * scale).round() as i32;
        let edit_h = (26.0 * scale).round() as i32;
        let item_h = (26.0 * scale).round() as i32;

        let edit_container_y = if show_tabs {
            margin + tab_bar_h + gap
        } else {
            margin
        };
        let listbox_y = edit_container_y + edit_container_h + gap;
        let listbox_h = (max_rows as i32) * item_h;
        let width = (base_width * scale).round() as i32;
        let height = listbox_y + listbox_h + margin;

        Self {
            margin,
            tab_bar_h,
            edit_container_h,
            edit_h,
            item_h,
            edit_container_y,
            listbox_y,
            listbox_h,
            width,
            height,
        }
    }
}

pub fn calculate_window_dimensions(scale: f32) -> (i32, i32) {
    let m = UiMetrics::get(scale);
    (m.width, m.height)
}
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
                fifo_lifo_mode: state::FifoLifoMode::None,
                fifo_lifo_queue: std::collections::VecDeque::new(),
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
            unsafe {
                win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
            }
            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                unsafe {
                    win32::InvalidateRect(*hwnd_main, std::ptr::null(), 1);
                }
            }
        }
    }
}

pub fn navigate_parent_folder() {
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
                    navigate_parent_folder();
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

pub fn delete_selected_items(delete_count: usize) {
    if delete_count == 0 {
        return;
    }

    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        if cur != win32::LB_ERR {
            let mut history_to_save = None;
            let mut next_sel = cur;

            {
                let mut state_guard = lock_state();
                if let Some(state) = &mut *state_guard {
                    if state.mode == Mode::History {
                        let history = std::sync::Arc::make_mut(&mut state.history);
                        let mut deleted_any = false;

                        // Disable redrawing to prevent flickering during batch deletion
                        unsafe {
                            win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 0, 0);
                        }

                        for _ in 0..delete_count {
                            let len = state.current_full_paths.len();
                            if len == 0 {
                                break;
                            }
                            // Clamp target index to current list size
                            let idx = std::cmp::min(next_sel as usize, len - 1);
                            let target_text = state.current_full_paths[idx].clone();

                            // 1. Remove from state.history
                            if let Some(pos) = history.iter().position(|x| x == &target_text) {
                                history.remove(pos);
                                deleted_any = true;
                            }

                            // 2. Remove from state active results
                            if idx < state.current_results.len() {
                                state.current_results.remove(idx);
                            }
                            if idx < state.current_full_paths.len() {
                                state.current_full_paths.remove(idx);
                            }

                            // 3. Delete from the listbox directly (preserves scroll position)
                            unsafe {
                                win32::SendMessageW(
                                    *hwnd_listbox,
                                    win32::LB_DELETESTRING,
                                    idx,
                                    0,
                                );
                            }

                            // Determine next selection index
                            let new_len = state.current_full_paths.len();
                            if new_len > 0 {
                                next_sel = std::cmp::min(idx as isize, (new_len - 1) as isize);
                            } else {
                                next_sel = win32::LB_ERR;
                                break;
                            }
                        }

                        // Apply new selection and re-enable redrawing
                        unsafe {
                            if next_sel != win32::LB_ERR {
                                win32::SendMessageW(
                                    *hwnd_listbox,
                                    win32::LB_SETCURSEL,
                                    next_sel as usize,
                                    0,
                                );
                            }
                            win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 1, 0);
                        }

                        if deleted_any
                            && state::SAVE_HISTORY_TO_FILE
                                .load(std::sync::atomic::Ordering::Relaxed)
                        {
                            history_to_save = Some(std::sync::Arc::clone(&state.history));
                        }
                    }
                }
            }

            if let Some(history_arc) = history_to_save {
                if let Some(sender) = state::HISTORY_SAVE_SENDER.get() {
                    let _ = sender.send(history_arc);
                }
            }

            crate::wndproc::update_top_index();
            unsafe {
                win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
            }
            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                unsafe {
                    win32::InvalidateRect(*hwnd_main, std::ptr::null(), 1);
                }
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
        // 1. キャレット位置またはマウスカーソル位置を取得する
        let mut target_pt: Option<win32::POINT> = None;
        let mut caret_h = 0i32;

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
                caret_h = (gui.rcCaret.bottom - gui.rcCaret.top).max(0);
                target_pt = Some(pt);
            } else {
                // 2. MSAA IAccessible fallback (works for some apps that don't use Win32 caret)
                let focus_hwnd = if gui_ok && !gui.hwndFocus.is_null() {
                    gui.hwndFocus
                } else {
                    active_hwnd
                };
                if let Some((cx, cy, _cw, ch)) = win32::get_caret_rect_accessible(focus_hwnd) {
                    target_pt = Some(win32::POINT { x: cx, y: cy + ch });
                    caret_h = ch.max(0);
                }
            }
        }

        // キャレットが取れなければマウスカーソル位置
        let (pt, is_caret) = if let Some(p) = target_pt {
            (p, true)
        } else {
            let mut mouse_pt = win32::POINT { x: 0, y: 0 };
            unsafe { win32::GetCursorPos(&mut mouse_pt) };
            (mouse_pt, false)
        };

        // 2. カーソル/キャレット位置が属するモニターを特定する
        let h_monitor = unsafe {
            let mon = win32::MonitorFromPoint(pt, win32::MONITOR_DEFAULTTONEAREST);
            if mon.is_null() && !active_hwnd.is_null() {
                win32::MonitorFromWindow(active_hwnd, win32::MONITOR_DEFAULTTONEAREST)
            } else {
                mon
            }
        };

        let mut work_rect = win32::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut got_monitor = false;

        if !h_monitor.is_null() {
            unsafe {
                let mut mi: win32::MONITORINFO = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<win32::MONITORINFO>() as u32;
                if win32::GetMonitorInfoW(h_monitor, &mut mi) != 0 {
                    work_rect = mi.rcWork;
                    got_monitor = true;
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

        // 3. 対象モニターの DPI スケールを決定する
        let scale = unsafe {
            let mut dpi = 0u32;
            if !h_monitor.is_null() {
                // Try GetDpiForMonitor from shcore.dll
                type GetDpiForMonitorFn = unsafe extern "system" fn(
                    *mut std::ffi::c_void,
                    i32,
                    *mut u32,
                    *mut u32,
                ) -> i32;

                let mut shcore = win32::GetModuleHandleW(util::to_wstring("shcore.dll").as_ptr());
                if shcore.is_null() {
                    shcore = win32::LoadLibraryW(util::to_wstring("shcore.dll").as_ptr());
                }
                if !shcore.is_null() {
                    let proc_addr = win32::GetProcAddress(shcore, b"GetDpiForMonitor\0".as_ptr());
                    if !proc_addr.is_null() {
                        let func: GetDpiForMonitorFn = std::mem::transmute(proc_addr);
                        let mut dpi_x = 0u32;
                        let mut dpi_y = 0u32;
                        if func(h_monitor, 0 /* MDT_EFFECTIVE_DPI */, &mut dpi_x, &mut dpi_y) == 0 && dpi_x > 0 {
                            dpi = dpi_x;
                        }
                    }
                }
            }

            if dpi == 0 && !active_hwnd.is_null() {
                dpi = win32::GetDpiForWindow(active_hwnd);
            }
            if dpi == 0 {
                dpi = win32::GetDpiForWindow(*hwnd_main);
            }
            if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 }
        };

        // 4. ウィンドウサイズを計算
        let (w, h) = calculate_window_dimensions(scale);

        // 5. 初期配置位置を計算
        let gap_y = if is_caret {
            (4.0 * scale) as i32
        } else {
            (10.0 * scale) as i32
        };

        let mut x = pt.x;
        let mut y = pt.y + gap_y;

        // 6. work_rect 内にクランプ（はみ出し防止 & フリップ処理）
        // 横方向のはみ出し防止
        if x + w > work_rect.right {
            x = work_rect.right - w;
        }
        if x < work_rect.left {
            x = work_rect.left;
        }

        // 縦方向のはみ出し防止
        if y + h > work_rect.bottom {
            if is_caret && (pt.y - caret_h - h - gap_y >= work_rect.top) {
                // キャレットの上に反転配置
                y = pt.y - caret_h - h - gap_y;
            } else {
                // 下端にクランプ
                y = work_rect.bottom - h;
            }
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
            crate::wndproc::update_theme_resources_with_scale(*hwnd_main, is_dark, scale);

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

    // Restore focus to the last active window
    restore_focus(last_active);

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

    let ctrl_pressed =
        unsafe { (win32::GetKeyState(win32::VK_CONTROL as i32) & 0x8000u16 as i16) != 0 };

    if ctrl_pressed {
        // 物理的に Ctrl が押されている場合は、V の押し下げ・解放のみをシミュレートする
        let inputs = [
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_V,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
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
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
                    },
                },
            },
        ];
        unsafe {
            win32::SendInput(
                2,
                inputs.as_ptr(),
                std::mem::size_of::<win32::INPUT>() as i32,
            )
        };
    } else {
        // 物理的に Ctrl が押されていない場合は、Ctrl + V 全体をシミュレートする
        let inputs = [
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_CONTROL,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
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
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
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
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
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
                        dw_extra_info: state::CLIPPER_MAGIC_INFO,
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
}

pub fn update_tray_tip_and_icon(hwnd: win32::HWND) {
    let (mode, queue_len) = {
        let state_guard = lock_state();
        if let Some(state) = &*state_guard {
            (state.fifo_lifo_mode, state.fifo_lifo_queue.len())
        } else {
            (state::FifoLifoMode::None, 0)
        }
    };

    let version = env!("CARGO_PKG_VERSION");
    let tip_text = match mode {
        state::FifoLifoMode::None => format!("Clipper v{}", version),
        state::FifoLifoMode::Fifo => format!("Clipper v{} [FIFO: {} items]", version, queue_len),
        state::FifoLifoMode::Lifo => format!("Clipper v{} [LIFO: {} items]", version, queue_len),
    };

    let mut nid: win32::NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = win32::NIF_TIP;

    let tip_w = util::to_wstring(&tip_text);
    let tip_len = std::cmp::min(tip_w.len(), 127);
    nid.szTip[..tip_len].copy_from_slice(&tip_w[..tip_len]);

    unsafe {
        win32::Shell_NotifyIconW(win32::NIM_MODIFY, &nid);
    }
}

pub fn show_tray_menu(hwnd: win32::HWND) {
    let mut pt = win32::POINT { x: 0, y: 0 };
    unsafe { win32::GetCursorPos(&mut pt) };

    let menu = unsafe { win32::CreatePopupMenu() };
    unsafe {
        // 1. スニペット / 履歴操作
        win32::AppendMenuW(
            menu,
            0,
            1006,
            util::to_wstring("スニペットを再読み込み").as_ptr(),
        );
        win32::AppendMenuW(
            menu,
            0,
            1005,
            util::to_wstring("スニペットフォルダを開く").as_ptr(),
        );
        win32::AppendMenuW(menu, 0, 1009, util::to_wstring("履歴をクリア").as_ptr());

        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null()); // Separator

        // 2. FIFO / LIFO モード
        let current_mode = {
            let state_guard = lock_state();
            state_guard
                .as_ref()
                .map(|s| s.fifo_lifo_mode)
                .unwrap_or(state::FifoLifoMode::None)
        };
        let queue_len = {
            let state_guard = lock_state();
            state_guard
                .as_ref()
                .map(|s| s.fifo_lifo_queue.len())
                .unwrap_or(0)
        };

        let check_none = if current_mode == state::FifoLifoMode::None {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_none,
            1010,
            util::to_wstring("通常モード").as_ptr(),
        );

        let check_fifo = if current_mode == state::FifoLifoMode::Fifo {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_fifo,
            1011,
            util::to_wstring(&format!("FIFO モード ({})", queue_len)).as_ptr(),
        );

        let check_lifo = if current_mode == state::FifoLifoMode::Lifo {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_lifo,
            1012,
            util::to_wstring(&format!("LIFO モード ({})", queue_len)).as_ptr(),
        );

        if queue_len > 0 {
            win32::AppendMenuW(menu, 0, 1013, util::to_wstring("キューをクリア").as_ptr());
        }

        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null()); // Separator

        // 3. 設定 / トグル項目
        let is_show_tabs = state::SHOW_TABS.load(std::sync::atomic::Ordering::Relaxed);
        let check_tabs_flag = if is_show_tabs {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_tabs_flag,
            1014,
            util::to_wstring("タブを表示する").as_ptr(),
        );

        let is_save = state::SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed);
        let check_save_flag = if is_save {
            win32::MF_CHECKED
        } else {
            win32::MF_UNCHECKED
        };
        win32::AppendMenuW(
            menu,
            check_save_flag,
            1008,
            util::to_wstring("履歴をファイルに保存する").as_ptr(),
        );

        win32::AppendMenuW(
            menu,
            0,
            1004,
            util::to_wstring("設定ファイルを開く").as_ptr(),
        );

        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null()); // Separator

        // 4. アプリ情報
        win32::AppendMenuW(
            menu,
            0,
            1007,
            util::to_wstring("このアプリについて").as_ptr(),
        );

        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null()); // Separator

        // 5. 終了
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
        match std::fs::create_dir_all(&path) {
            Ok(_) => {
                let _ = std::process::Command::new("explorer").arg(path).spawn();
            }
            Err(e) => {
                show_notification(
                    "エラー",
                    &format!("フォルダを作成できませんでした: {}", e),
                    true,
                );
            }
        }
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
        show_notification(
            "Clipper",
            &format!("スニペットを再読み込みしました（{}件）", count),
            false,
        );
    } else if cmd == 1007 {
        let version = env!("CARGO_PKG_VERSION");
        let title_w = util::to_wstring(&format!("Clipper v{}", version));
        let message_w = util::to_wstring(
            "Clipper - Snippet & Clipboard Manager\n\n\
            【簡単な使い方】\n\
            ・Shiftキーを2回連打: スニペット検索ウィンドウを表示\n\
            ・Ctrlキーを2回連打: クリップボード履歴ウィンドウを表示\n\n\
            【FIFO/LIFO モード】\n\
            ・Ctrl+Shift+F: FIFOモードのON/OFF\n\
            ・Ctrl+Shift+L: LIFOモードのON/OFF\n\
            ・Ctrl+Shift+C: キューのクリアと終了\n\n\
            ※モード中にコピーすると自動的にキューに蓄積され、Ctrl+Vで順次ペーストできます。\n\n\
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
                if let Some(sender) = state::HISTORY_SAVE_SENDER.get() {
                    let _ = sender.send(history_arc);
                }
            }
        }
    } else if cmd == 1014 {
        let new_val = !state::SHOW_TABS.load(std::sync::atomic::Ordering::Relaxed);
        state::SHOW_TABS.store(new_val, std::sync::atomic::Ordering::Relaxed);

        // Update config.toml
        if let Some(config) = state::CONFIG.get() {
            let mut new_config = config.clone();
            new_config.show_tabs = new_val;
            new_config.save();
        }

        if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
            let visible = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.visible)
            };
            if visible {
                let scale = unsafe { win32::GetDpiForWindow(*hwnd_main) } as f32 / 96.0;
                let (w, h) = calculate_window_dimensions(scale);

                let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
                unsafe {
                    win32::GetWindowRect(*hwnd_main, &mut rc);
                    win32::SetWindowPos(
                        *hwnd_main,
                        std::ptr::null_mut(),
                        rc.left,
                        rc.top,
                        w,
                        h,
                        0x0004 /* SWP_NOZORDER */ | 0x0020 /* SWP_FRAMECHANGED */,
                    );
                    win32::InvalidateRect(*hwnd_main, std::ptr::null(), 1);
                }
            }
        }
    } else if cmd == 1009 {
        let title_w = util::to_wstring("Clipper");
        let question_w = util::to_wstring(
            "クリップボード履歴をすべてクリアしますか？\n（この操作は取り消せません）",
        );
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
            if let Some(sender) = state::HISTORY_SAVE_SENDER.get() {
                let _ = sender.send(std::sync::Arc::new(std::collections::VecDeque::new()));
            }

            // Update UI listbox if visible
            update_listbox_items();

            // Display balloon notification
            show_notification("Clipper", "履歴をクリアしました", false);
        }
    } else if cmd == 1010 {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.fifo_lifo_mode = state::FifoLifoMode::None;
            state.fifo_lifo_queue.clear();
            state::sync_fifo_lifo_state(state);
        }
        std::mem::drop(state_guard);
        update_tray_tip_and_icon(hwnd);
        show_notification_ex("通常モード", "通常モードに戻りました。", false, true);
    } else if cmd == 1011 {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.fifo_lifo_mode = state::FifoLifoMode::Fifo;
            state::sync_fifo_lifo_state(state);
        }
        std::mem::drop(state_guard);
        update_tray_tip_and_icon(hwnd);
        show_notification(
            "FIFO モード開始",
            "コピーしたデータが古い順にペーストされます。\n(Ctrl+Shift+F で解除)",
            false,
        );
    } else if cmd == 1012 {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.fifo_lifo_mode = state::FifoLifoMode::Lifo;
            state::sync_fifo_lifo_state(state);
        }
        std::mem::drop(state_guard);
        update_tray_tip_and_icon(hwnd);
        show_notification(
            "LIFO モード開始",
            "コピーしたデータが新しい順にペーストされます。\n(Ctrl+Shift+L で解除)",
            false,
        );
    } else if cmd == 1013 {
        let mut state_guard = lock_state();
        if let Some(state) = &mut *state_guard {
            state.fifo_lifo_queue.clear();
            state::sync_fifo_lifo_state(state);
        }
        std::mem::drop(state_guard);
        update_tray_tip_and_icon(hwnd);
        show_notification(
            "キューをクリア",
            "蓄積されたコピーデータをクリアしました。",
            false,
        );
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

use std::sync::Once;
use std::sync::OnceLock;

static REGISTER_APP_ID: Once = Once::new();
static TOAST_SEQUENCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CACHED_ICON_PATH_LIGHT: OnceLock<Option<String>> = OnceLock::new();
static CACHED_ICON_PATH_DARK: OnceLock<Option<String>> = OnceLock::new();

/// トースト通知に必要な AppUserModelId をレジストリに登録します。
/// Win32 API を直接使用して HKCU に書き込むため、PowerShell等の外部プロセスの起動がなく、
/// コンソール画面のポップアップやパフォーマンスオーバーヘッドは一切ありません。
pub fn register_app_id() {
    REGISTER_APP_ID.call_once(|| {
        unsafe {
            let subkey = util::to_wstring("Software\\Classes\\AppUserModelId\\clipper");
            let mut hkey = std::ptr::null_mut();

            let status = win32::RegCreateKeyExW(
                win32::HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                std::ptr::null(),
                0,
                win32::KEY_WRITE,
                std::ptr::null(),
                &mut hkey,
                std::ptr::null_mut(),
            );

            if status == 0 {
                // アプリ名を表示名として登録
                let value_name = util::to_wstring("DisplayName");
                let value_data = util::to_wstring("Clipper");
                let cb_data = (value_data.len() * 2) as u32;

                let _ = win32::RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    win32::REG_SZ,
                    value_data.as_ptr() as *const u8,
                    cb_data,
                );

                // タイトルバー用のアイコン絶対パスを登録
                let is_sys_dark = crate::darkmode::is_system_dark_mode();
                if let Some(icon_path) = get_icon_path(is_sys_dark) {
                    let win_path = icon_path.replace('/', "\\");
                    let icon_name = util::to_wstring("IconUri");
                    let icon_data = util::to_wstring(&win_path);
                    let cb_icon = (icon_data.len() * 2) as u32;

                    let _ = win32::RegSetValueExW(
                        hkey,
                        icon_name.as_ptr(),
                        0,
                        win32::REG_SZ,
                        icon_data.as_ptr() as *const u8,
                        cb_icon,
                    );
                }

                win32::RegCloseKey(hkey);
            }
        }
    });
}

/// トースト表示に使用するアプリのロゴ画像 (assets/app.png / assets/app_inverted.png) の絶対パスを取得します。
/// 重いファイルシステムアクセスを避けるため、初回取得時に結果を `OnceLock` へキャッシュします。
fn get_icon_path(is_dark: bool) -> Option<String> {
    let cache = if is_dark {
        &CACHED_ICON_PATH_DARK
    } else {
        &CACHED_ICON_PATH_LIGHT
    };

    cache
        .get_or_init(|| {
            if is_dark {
                crate::util::get_icon_inverted_path()
            } else {
                crate::util::get_icon_path()
            }
        })
        .clone()
}

/// Windows Runtime (WinRT) API を用いてトースト通知を送信します。
/// 送信前に古い Clipper の通知を `RemoveGroup` で即座に全削除し、
/// 新しい通知をアトミックなシーケンスID付きのユニークなタグで送信します。
/// 指定秒数経過後、自分のスレッドに対応するタグのみを消去することで競合を防ぎます。
fn show_toast_notification(
    title: &str,
    msg: &str,
    seconds: u64,
    sound: bool,
    _is_error: bool,
) -> windows::core::Result<()> {
    use windows::{Data::Xml::Dom::*, UI::Notifications::*, core::*};

    let app_id = HSTRING::from("clipper");
    let group = HSTRING::from("clipper_group");

    // 新しい通知を送信する前に、すでに表示されているClipperグループの古い通知をすべて即座に消去する
    if let Ok(history) = ToastNotificationManager::History() {
        let _ = history.RemoveGroup(&group);
    }

    // XML configuration
    let duration = if seconds <= 7 { "short" } else { "long" };
    let audio_xml = if sound {
        ""
    } else {
        "<audio silent=\"true\"/>"
    };

    let is_sys_dark = crate::darkmode::is_system_dark_mode();
    let mut image_xml = String::new();
    if let Some(clean_path) = get_icon_path(is_sys_dark) {
        image_xml = format!(
            r#"<image placement="appLogoOverride" src="file:///{}" />"#,
            clean_path
        );
    }

    let escaped_title = escape_xml(title);
    let escaped_msg = escape_xml(msg);

    let xml_string = format!(
        r#"<toast duration="{}">
            <visual>
                <binding template="ToastGeneric">
                    <text>{}</text>
                    <text>{}</text>
                    {}
                </binding>
            </visual>
            {}
        </toast>"#,
        duration, escaped_title, escaped_msg, image_xml, audio_xml
    );

    let xml_doc = XmlDocument::new()?;
    xml_doc.LoadXml(&HSTRING::from(xml_string))?;

    // 一意のシーケンス番号を含めたタグを生成
    let seq = TOAST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = HSTRING::from(format!("clipper_tag_{}_{}", std::process::id(), seq));

    let toast = ToastNotification::CreateToastNotification(&xml_doc)?;
    toast.SetTag(&tag)?;
    toast.SetGroup(&group)?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)?;
    notifier.Show(&toast)?;

    if seconds > 0 {
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        if let Ok(history) = ToastNotificationManager::History() {
            // 自分自身が送信した特定のタグのみを削除する
            let _ = history.RemoveGroupedTagWithId(&tag, &group, &app_id);
        }
    }

    Ok(())
}

/// XML文字列用の特殊文字エスケープ処理
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 設定に従ってトースト通知をバックグラウンドスレッドで送信します。
pub fn show_notification(title: &str, message: &str, is_error: bool) {
    show_notification_ex(title, message, is_error, false);
}

/// 強制的な通知音設定を伴う拡張版のトースト通知送信関数です。
pub fn show_notification_ex(title: &str, message: &str, is_error: bool, force_sound: bool) {
    let mut duration_secs = 5;
    let mut sound = false;

    if let Some(config) = state::CONFIG.get() {
        if !config.show_notifications {
            return;
        }
        duration_secs = config.notification_duration;
        sound = config.notification_sound;
    }

    if force_sound {
        sound = true;
    }

    register_app_id();

    let title_str = title.to_string();
    let message_str = message.to_string();

    std::thread::spawn(move || {
        let _ = show_toast_notification(&title_str, &message_str, duration_secs, sound, is_error);
    });
}
