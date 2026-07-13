#![windows_subsystem = "windows"]

mod config;
mod darkmode;
mod dict;
mod filter;
mod hook;
mod state;
mod ui;
mod util;
mod win32;
mod wndproc;

use crate::hook::keyboard_hook_proc;
use crate::state::{MAIN_HWND, Mode, SafeHWND, lock_state, LISTBOX_HWND};
use crate::wndproc::window_proc;

fn get_shortcut_index(wparam: usize) -> Option<usize> {
    if (0x31..=0x39).contains(&wparam) {
        // '1'..'9'
        Some(wparam - 0x31)
    } else if wparam == 0x30 {
        // '0'
        Some(9)
    } else if (0x41..=0x5A).contains(&wparam) {
        // 'A'..'Z'
        Some(10 + (wparam - 0x41))
    } else {
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set Per-Monitor DPI Aware V2 context
    unsafe {
        win32::SetProcessDpiAwarenessContext(-4isize as *mut std::ffi::c_void);
    }

    let config = config::Config::load();
    state::SAVE_HISTORY_TO_FILE.store(config.save_history, std::sync::atomic::Ordering::Relaxed);
    let _ = state::CONFIG.set(config);

    darkmode::apply();

    // トースト通知用の AppUserModelId / IconUri を起動時に即座にレジストリ登録
    ui::register_app_id();

    // Initialize COM for MSAA (IAccessible) caret position detection
    unsafe { win32::CoInitializeEx(std::ptr::null_mut(), win32::COINIT_APARTMENTTHREADED) };

    let _mutex_handle;
    {
        let name = util::to_wstring("Global\\ClipperAppMutex");
        _mutex_handle = unsafe { win32::CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if unsafe { win32::GetLastError() } == win32::ERROR_ALREADY_EXISTS {
            return Ok(());
        }
    }

    state::start_logging_thread();

    let mut app_state = state::AppState {
        history: std::sync::Arc::new(util::load_history()),
        snippets: std::sync::Arc::new(Vec::new()),
        mode: Mode::Snippet,
        visible: false,
        current_results: Vec::new(),
        current_full_paths: Vec::new(),
        last_clipboard_value: String::new(),
        current_selection: String::new(),
        last_active_window: None,
        is_dark: darkmode::is_dark_active(),
        current_folder: String::new(),
        top_index: 0,
        filter_generation: 0,
        fifo_lifo_mode: state::FifoLifoMode::None,
        fifo_lifo_queue: std::collections::VecDeque::new(),
    };

    if let Some(text) = util::get_clipboard_text() {
        app_state.last_clipboard_value = text;
    }

    *lock_state() = Some(app_state);

    unsafe {
        let hinstance = win32::GetModuleHandleW(std::ptr::null());
        let class_name = util::to_wstring("ClipperWindowClass");
        let is_sys_dark = darkmode::is_system_dark_mode();
        let icon_id = if is_sys_dark { 2 } else { 1 };
        let app_icon = win32::LoadIconW(hinstance, icon_id as *const u16);

        let wnd_class = win32::WNDCLASSW {
            style: win32::CS_HREDRAW | win32::CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: app_icon,
            hCursor: win32::LoadCursorW(std::ptr::null_mut(), win32::IDC_ARROW),
            hbrBackground: std::ptr::null_mut(), // Handle background erasing manually
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        win32::RegisterClassW(&wnd_class);

        let max_rows = state::CONFIG.get().map_or(15, |c| c.max_rows);
        let initial_h = (max_rows as i32) * 26 + 52;
        let base_width = state::CONFIG.get().map_or(380.0, |c| c.width);

        let hwnd = win32::CreateWindowExW(
            win32::WS_EX_TOPMOST | win32::WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            util::to_wstring("Clipper").as_ptr(),
            win32::WS_POPUP, // Borderless, we will draw the border in WM_PAINT
            0,
            0,
            base_width as i32,
            initial_h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create Win32 Window".into());
        }

        let _ = MAIN_HWND.set(SafeHWND(hwnd));

        let mut nid: win32::NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = win32::NIF_MESSAGE | win32::NIF_ICON | win32::NIF_TIP;
        nid.uCallbackMessage = win32::WM_TRAYICON;
        nid.hIcon = app_icon;

        let version = env!("CARGO_PKG_VERSION");
        let tip_w = util::to_wstring(&format!("Clipper v{}", version));
        let tip_len = std::cmp::min(tip_w.len(), 127);
        nid.szTip[..tip_len].copy_from_slice(&tip_w[..tip_len]);

        win32::Shell_NotifyIconW(win32::NIM_ADD, &nid);
        win32::AddClipboardFormatListener(hwnd);

        let hinstance_hook = win32::GetModuleHandleW(std::ptr::null());
        let hook = win32::SetWindowsHookExW(
            win32::WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            hinstance_hook,
            0,
        );
        if hook.is_null() {
            state::log_debug("SetWindowsHookExW failed to register on main thread!");
        } else {
            state::log_debug("SetWindowsHookExW registered successfully on main thread.");
        }

        let mut msg = std::mem::zeroed();
        while win32::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let is_key_msg = msg.message == win32::WM_KEYDOWN || msg.message == win32::WM_SYSKEYDOWN;
            let is_visible = is_key_msg && {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.visible)
            };

            if is_visible && msg.message == win32::WM_KEYDOWN {
                if msg.wparam == 13 {
                    ui::on_select();
                    continue;
                } else if msg.wparam == 27 {
                    ui::hide_window();
                    continue;
                }
            } else if is_visible && msg.message == win32::WM_SYSKEYDOWN {
                if let Some(shortcut_idx) = get_shortcut_index(msg.wparam) {
                    let (top_index, item_count) = {
                        let top = if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                            win32::SendMessageW(*hwnd_listbox, win32::LB_GETTOPINDEX, 0, 0) as usize
                        } else {
                            0
                        };
                        let state_guard = lock_state();
                        let count = state_guard.as_ref().map_or(0, |s| s.current_results.len());
                        (top, count)
                    };
                    let target_idx = top_index + shortcut_idx;

                    if target_idx < item_count {
                        if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                            win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, target_idx, 0);
                            ui::on_select();
                        }
                    }
                    continue;
                }
            }

            if win32::IsDialogMessageW(hwnd, &msg) == 0 {
                win32::TranslateMessage(&msg);
                win32::DispatchMessageW(&msg);
            }
        }

        if !hook.is_null() {
            win32::UnhookWindowsHookEx(hook);
        }
        win32::RemoveClipboardFormatListener(hwnd);
        win32::Shell_NotifyIconW(win32::NIM_DELETE, &nid);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shortcut_index() {
        assert_eq!(get_shortcut_index(0x31), Some(0)); // '1'
        assert_eq!(get_shortcut_index(0x39), Some(8)); // '9'
        assert_eq!(get_shortcut_index(0x30), Some(9)); // '0'
        assert_eq!(get_shortcut_index(0x41), Some(10)); // 'A'
        assert_eq!(get_shortcut_index(0x5A), Some(35)); // 'Z'
        assert_eq!(get_shortcut_index(b' ' as usize), None);
        assert_eq!(get_shortcut_index(0x00), None);
    }
}
