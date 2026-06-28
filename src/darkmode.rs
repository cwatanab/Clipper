use std::ffi::c_void;

use crate::util;
use crate::win32;

type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;
type AllowDarkModeForWindowFn = unsafe extern "system" fn(win32::HWND, i32) -> i32;

fn get_uxtheme_ordinal(ordinal: u16) -> *mut c_void {
    unsafe {
        let hmod = win32::GetModuleHandleW(util::to_wstring("uxtheme.dll").as_ptr());
        if hmod.is_null() {
            return std::ptr::null_mut();
        }
        win32::GetProcAddress(hmod, ordinal as *const u8)
    }
}

pub fn apply() {
    let ptr = get_uxtheme_ordinal(135);
    if !ptr.is_null() {
        unsafe {
            let f: SetPreferredAppModeFn = std::mem::transmute(ptr);
            f(1);
        }
    }
}

pub fn apply_to_window(hwnd: win32::HWND) {
    unsafe {
        let use_dark: i32 = 1;
        win32::DwmSetWindowAttribute(
            hwnd,
            win32::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark as *const _ as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let ptr = get_uxtheme_ordinal(133);
        if !ptr.is_null() {
            let f: AllowDarkModeForWindowFn = std::mem::transmute(ptr);
            f(hwnd, 1);
        }

        let theme = util::to_wstring("DarkMode_Explorer");
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}

pub fn apply_to_control(hwnd: win32::HWND) {
    unsafe {
        let theme = util::to_wstring("DarkMode_Explorer");
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}
