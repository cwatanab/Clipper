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
use crate::state::{Mode, SafeHWND, lock_state, MAIN_HWND};
use crate::wndproc::window_proc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set Per-Monitor DPI Aware V2 context
    unsafe {
        win32::SetProcessDpiAwarenessContext(-4isize as *mut std::ffi::c_void);
    }

    let config = config::Config::load();
    let _ = state::CONFIG.set(config);

    darkmode::apply();

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
        last_active_window: None,
        is_dark: darkmode::is_dark_mode(),
        current_folder: String::new(),
        top_index: 0,
        filter_generation: 0,
    };

    if let Some(text) = util::get_clipboard_text() {
        app_state.last_clipboard_value = text;
    }

    *lock_state() = Some(app_state);

    unsafe {
        let hinstance = win32::GetModuleHandleW(std::ptr::null());
        let class_name = util::to_wstring("ClipperWindowClass");
        let app_icon = win32::LoadIconW(hinstance, 1 as *const u16);

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

        let hwnd = win32::CreateWindowExW(
            win32::WS_EX_TOPMOST | win32::WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            util::to_wstring("Clipper").as_ptr(),
            win32::WS_POPUP, // Borderless, we will draw the border in WM_PAINT
            0, 0, 380, initial_h,
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
            let is_visible = {
                let state_guard = lock_state();
                state_guard.as_ref().map_or(false, |s| s.visible)
            };

            if is_visible && msg.message == win32::WM_KEYDOWN {
                if msg.wparam == 13 {
                    ui::on_select();
                    continue;
                } else if msg.wparam == 27 {
                    ui::hide_window();
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
