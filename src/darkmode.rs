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
            f(1); // 1 = AllowDark, 2 = ForceDark
        }
    }
}

pub fn is_dark_mode() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        let subkey = util::to_wstring("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let valname = util::to_wstring("AppsUseLightTheme");
        let mut hkey: win32::HKEY = std::ptr::null_mut();
        
        let status = win32::RegOpenKeyExW(
            win32::HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            win32::KEY_READ,
            &mut hkey,
        );
        
        if status == 0 {
            let mut type_val: u32 = 0;
            let mut data_val: [u8; 4] = [0; 4];
            let mut size_val: u32 = 4;
            
            let query_status = win32::RegQueryValueExW(
                hkey,
                valname.as_ptr(),
                std::ptr::null_mut(),
                &mut type_val,
                data_val.as_mut_ptr(),
                &mut size_val,
            );
            
            win32::RegCloseKey(hkey);
            
            if query_status == 0 && size_val == 4 {
                let val = u32::from_le_bytes(data_val);
                return val == 0; // 0 means Dark, 1 means Light
            }
        }
    }
    false // Default to light mode
}

pub fn apply_to_window(hwnd: win32::HWND, dark: bool) {
    unsafe {
        let use_dark: i32 = if dark { 1 } else { 0 };
        win32::DwmSetWindowAttribute(
            hwnd,
            win32::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark as *const _ as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let ptr = get_uxtheme_ordinal(133);
        if !ptr.is_null() {
            let f: AllowDarkModeForWindowFn = std::mem::transmute(ptr);
            f(hwnd, use_dark);
        }

        let theme = if dark {
            util::to_wstring("DarkMode_Explorer")
        } else {
            util::to_wstring("Explorer")
        };
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}

pub fn apply_to_control(hwnd: win32::HWND, dark: bool) {
    unsafe {
        let theme = if dark {
            util::to_wstring("DarkMode_Explorer")
        } else {
            util::to_wstring("Explorer")
        };
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}
