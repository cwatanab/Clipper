use std::sync::atomic::Ordering;

use crate::state::{
    LAST_KEY_TIME, LAST_KEY_VK, MAIN_HWND, MOUSE_HOOK, SafeHHOOK, SafeHWND, WM_HIDE_WINDOW,
    WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET, WM_FIFO_LIFO_PASTE, WM_TOGGLE_FIFO_LIFO,
};
use crate::win32;

#[cfg(target_os = "windows")]
fn matches_key(vk: u16, key_name: &str) -> bool {
    match key_name.to_lowercase().as_str() {
        "shift" => vk == win32::VK_SHIFT || vk == win32::VK_LSHIFT || vk == win32::VK_RSHIFT,
        "lshift" | "left_shift" => vk == win32::VK_LSHIFT,
        "rshift" | "right_shift" => vk == win32::VK_RSHIFT,
        "ctrl" | "control" => vk == win32::VK_CONTROL || vk == win32::VK_LCONTROL || vk == win32::VK_RCONTROL,
        "lctrl" | "left_ctrl" | "lcontrol" | "left_control" => vk == win32::VK_LCONTROL,
        "rctrl" | "right_ctrl" | "rcontrol" | "right_control" => vk == win32::VK_RCONTROL,
        "alt" | "menu" => vk == win32::VK_MENU || vk == win32::VK_LMENU || vk == win32::VK_RMENU,
        "lalt" | "left_alt" | "lmenu" | "left_menu" => vk == win32::VK_LMENU,
        "ralt" | "right_alt" | "rmenu" | "right_menu" => vk == win32::VK_RMENU,
        _ => false,
    }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    if code >= 0 {
        let kbd = unsafe { *(lparam as *const win32::KBDLLHOOKSTRUCT) };
        let vk = kbd.vk_code as u16;
        let now_time = if kbd.time == 0 {
            unsafe { win32::GetTickCount() }
        } else {
            kbd.time
        };

        let (snippet_key, history_key, double_tap_ms) = crate::state::CONFIG.get()
            .map(|c| (c.snippet_key.clone(), c.history_key.clone(), c.double_tap_ms))
            .unwrap_or_else(|| ("left_shift".to_string(), "left_ctrl".to_string(), 500));

        if wparam == win32::WM_KEYUP as win32::WPARAM
            || wparam == win32::WM_SYSKEYUP as win32::WPARAM
        {
            let is_snippet = matches_key(vk, &snippet_key);
            let is_history = matches_key(vk, &history_key);

            if is_snippet || is_history {
                let mapped_vk = if is_snippet { 1u32 } else { 2u32 };
                let prev_vk = LAST_KEY_VK.load(Ordering::Relaxed);
                let prev_time = LAST_KEY_TIME.load(Ordering::Relaxed);
                let other_pressed = crate::state::OTHER_KEY_PRESSED.load(Ordering::Relaxed);
                let keydown_time = crate::state::LAST_KEYDOWN_TIME.load(Ordering::Relaxed);
                let hold_duration = now_time.wrapping_sub(keydown_time);

                if other_pressed || hold_duration > 150 {
                    LAST_KEY_VK.store(0, Ordering::Relaxed);
                } else {
                    if prev_vk == mapped_vk && now_time.wrapping_sub(prev_time) < double_tap_ms {
                        let main_hwnd_val = MAIN_HWND.get();
                        if let Some(SafeHWND(main_hwnd)) = main_hwnd_val
                            && !(*main_hwnd).is_null()
                        {
                            let active_hwnd = unsafe { win32::GetForegroundWindow() };
                            let msg = if mapped_vk == 1 {
                                WM_TRIGGER_SNIPPET
                            } else {
                                WM_TRIGGER_HISTORY
                            };
                            unsafe {
                                win32::PostMessageW(*main_hwnd, msg, active_hwnd as win32::WPARAM, 0)
                            };
                        }
                        LAST_KEY_VK.store(0, Ordering::Relaxed);
                        return unsafe {
                            win32::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
                        };
                    }
                    LAST_KEY_VK.store(mapped_vk, Ordering::Relaxed);
                    LAST_KEY_TIME.store(now_time, Ordering::Relaxed);
                }
            } else {
                crate::state::OTHER_KEY_PRESSED.store(true, Ordering::Relaxed);
            }
        } else if wparam == win32::WM_KEYDOWN as win32::WPARAM
            || wparam == win32::WM_SYSKEYDOWN as win32::WPARAM
        {
            let ctrl_pressed = unsafe {
                (win32::GetAsyncKeyState(win32::VK_CONTROL as i32) & 0x8000u16 as i16) != 0
            };
            let shift_pressed = unsafe {
                (win32::GetKeyState(win32::VK_SHIFT as i32) & 0x8000u16 as i16) != 0
            };

            // Ctrl + V が押され、且つ FIFO/LIFO モードが有効でキューが空でない場合
            if vk == 0x56 /* VK_V */ && ctrl_pressed {
                let is_fifo_lifo_active = {
                    let state_guard = crate::state::lock_state();
                    state_guard.as_ref().map_or(false, |s| {
                        s.fifo_lifo_mode != crate::state::FifoLifoMode::None && !s.fifo_lifo_queue.is_empty()
                    })
                };

                if is_fifo_lifo_active && kbd.dw_extra_info != crate::state::CLIPPER_MAGIC_INFO {
                    if let Some(SafeHWND(main_hwnd)) = MAIN_HWND.get()
                        && !(*main_hwnd).is_null()
                    {
                        unsafe {
                            win32::PostMessageW(*main_hwnd, WM_FIFO_LIFO_PASTE, 0, 0);
                        }
                    }
                    return 1; // キー入力を消費
                }
            }

            // Ctrl + Shift + F / L / Q / C (ホットキーでのモード切り替え)
            if ctrl_pressed && shift_pressed {
                if vk == 0x46 /* 'F' */ {
                    if let Some(SafeHWND(main_hwnd)) = MAIN_HWND.get()
                        && !(*main_hwnd).is_null()
                    {
                        unsafe {
                            win32::PostMessageW(*main_hwnd, WM_TOGGLE_FIFO_LIFO, 1, 0);
                        }
                    }
                    return 1;
                } else if vk == 0x4C /* 'L' */ {
                    if let Some(SafeHWND(main_hwnd)) = MAIN_HWND.get()
                        && !(*main_hwnd).is_null()
                    {
                        unsafe {
                            win32::PostMessageW(*main_hwnd, WM_TOGGLE_FIFO_LIFO, 2, 0);
                        }
                    }
                    return 1;
                } else if vk == 0x51 /* 'Q' */ || vk == 0x43 /* 'C' */ {
                    if let Some(SafeHWND(main_hwnd)) = MAIN_HWND.get()
                        && !(*main_hwnd).is_null()
                    {
                        unsafe {
                            win32::PostMessageW(*main_hwnd, WM_TOGGLE_FIFO_LIFO, 0, 0);
                        }
                    }
                    return 1;
                }
            }

            let is_snippet = matches_key(vk, &snippet_key);
            let is_history = matches_key(vk, &history_key);
            if is_snippet || is_history {
                crate::state::LAST_KEYDOWN_TIME.store(now_time, Ordering::Relaxed);
                crate::state::OTHER_KEY_PRESSED.store(false, Ordering::Relaxed);
            } else {
                crate::state::OTHER_KEY_PRESSED.store(true, Ordering::Relaxed);
                LAST_KEY_VK.store(0, Ordering::Relaxed);
            }
        }
    }
    unsafe { win32::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    if code >= 0 {
        let wp = wparam as u32;
        if wp == win32::WM_LBUTTONDOWN
            || wp == win32::WM_RBUTTONDOWN
            || wp == win32::WM_MBUTTONDOWN
            || wp == win32::WM_NCLBUTTONDOWN
            || wp == win32::WM_NCRBUTTONDOWN
            || wp == win32::WM_NCMBUTTONDOWN
        {
            LAST_KEY_VK.store(0, Ordering::Relaxed);
            let ms = unsafe { *(lparam as *const win32::MSLLHOOKSTRUCT) };
            let pt = ms.pt;

            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get()
                && !(*hwnd_main).is_null()
            {
                let mut rect = win32::RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                unsafe { win32::GetWindowRect(*hwnd_main, &mut rect) };

                let inside = pt.x >= rect.left
                    && pt.x <= rect.right
                    && pt.y >= rect.top
                    && pt.y <= rect.bottom;
                if !inside {
                    unsafe { win32::PostMessageW(*hwnd_main, WM_HIDE_WINDOW, 0, 0) };
                }
            }
        }
    }
    unsafe { win32::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(target_os = "windows")]
pub fn install_mouse_hook() {
    let mut guard = MOUSE_HOOK.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        unsafe {
            let hinstance = win32::GetModuleHandleW(std::ptr::null());
            let hook =
                win32::SetWindowsHookExW(win32::WH_MOUSE_LL, Some(mouse_hook_proc), hinstance, 0);
            if !hook.is_null() {
                *guard = Some(SafeHHOOK(hook));
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn uninstall_mouse_hook() {
    let mut guard = MOUSE_HOOK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(SafeHHOOK(hook)) = guard.take() {
        unsafe {
            win32::UnhookWindowsHookEx(hook);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub unsafe extern "system" fn mouse_hook_proc(
    _code: i32,
    _wparam: win32::WPARAM,
    _lparam: win32::LPARAM,
) -> win32::LRESULT {
    0
}

#[cfg(not(target_os = "windows"))]
pub fn install_mouse_hook() {}

#[cfg(not(target_os = "windows"))]
pub fn uninstall_mouse_hook() {}
