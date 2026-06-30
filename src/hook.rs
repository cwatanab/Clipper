use std::sync::atomic::Ordering;

use crate::state::{SafeHWND, SafeHHOOK, LAST_KEY_VK, LAST_KEY_TIME, MAIN_HWND, MOUSE_HOOK, WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET, WM_HIDE_WINDOW};
use crate::win32;

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    if code >= 0 {
        let kbd = unsafe { *(lparam as *const win32::KBDLLHOOKSTRUCT) };
        let vk = kbd.vk_code as u16;

        if wparam == win32::WM_KEYUP as win32::WPARAM || wparam == win32::WM_SYSKEYUP as win32::WPARAM {
            let is_shift = vk == win32::VK_SHIFT || vk == win32::VK_LSHIFT || vk == win32::VK_RSHIFT;
            let is_ctrl = vk == win32::VK_CONTROL || vk == win32::VK_LCONTROL || vk == win32::VK_RCONTROL;

            if is_shift || is_ctrl {
                let mapped_vk = if is_shift { win32::VK_SHIFT as u32 } else { win32::VK_CONTROL as u32 };
                let prev_vk = LAST_KEY_VK.load(Ordering::Relaxed);
                let prev_time = LAST_KEY_TIME.load(Ordering::Relaxed);
                let now_time = kbd.time;

                if prev_vk == mapped_vk && now_time.wrapping_sub(prev_time) < 500 {
                    let main_hwnd_val = MAIN_HWND.get();
                    if let Some(SafeHWND(main_hwnd)) = main_hwnd_val {
                        if *main_hwnd != std::ptr::null_mut() {
                            let active_hwnd = unsafe { win32::GetForegroundWindow() };
                            let msg = if mapped_vk == win32::VK_SHIFT as u32 { WM_TRIGGER_SNIPPET } else { WM_TRIGGER_HISTORY };
                            unsafe { win32::PostMessageW(*main_hwnd, msg, active_hwnd as win32::WPARAM, 0) };
                        }
                    }
                    LAST_KEY_VK.store(0, Ordering::Relaxed);
                    return unsafe { win32::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
                }
                LAST_KEY_VK.store(mapped_vk, Ordering::Relaxed);
                LAST_KEY_TIME.store(now_time, Ordering::Relaxed);
            }
        } else if wparam == win32::WM_KEYDOWN as win32::WPARAM || wparam == win32::WM_SYSKEYDOWN as win32::WPARAM {
            let is_modifier = vk == win32::VK_SHIFT || vk == win32::VK_LSHIFT || vk == win32::VK_RSHIFT
                || vk == win32::VK_CONTROL || vk == win32::VK_LCONTROL || vk == win32::VK_RCONTROL;
            if !is_modifier {
                LAST_KEY_VK.store(0, Ordering::Relaxed);
            }
        }
    }
    unsafe { win32::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    if code >= 0 {
        let wp = wparam as u32;
        if wp == win32::WM_LBUTTONDOWN 
            || wp == win32::WM_RBUTTONDOWN 
            || wp == win32::WM_MBUTTONDOWN 
            || wp == win32::WM_NCLBUTTONDOWN 
            || wp == win32::WM_NCRBUTTONDOWN 
            || wp == win32::WM_NCMBUTTONDOWN 
        {
            let ms = unsafe { *(lparam as *const win32::MSLLHOOKSTRUCT) };
            let pt = ms.pt;
            
            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                if *hwnd_main != std::ptr::null_mut() {
                    let mut rect = win32::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    unsafe { win32::GetWindowRect(*hwnd_main, &mut rect) };
                    
                    let inside = pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom;
                    if !inside {
                        unsafe { win32::PostMessageW(*hwnd_main, WM_HIDE_WINDOW, 0, 0) };
                    }
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
            let hook = win32::SetWindowsHookExW(
                win32::WH_MOUSE_LL,
                Some(mouse_hook_proc),
                hinstance,
                0,
            );
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
pub unsafe extern "system" fn mouse_hook_proc(_code: i32, _wparam: win32::WPARAM, _lparam: win32::LPARAM) -> win32::LRESULT {
    0
}

#[cfg(not(target_os = "windows"))]
pub fn install_mouse_hook() {}

#[cfg(not(target_os = "windows"))]
pub fn uninstall_mouse_hook() {}
