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

pub fn is_dark_active() -> bool {
    let mode = if let Some(config) = crate::state::CONFIG.get() {
        config.theme_mode.clone()
    } else {
        crate::config::Config::load().theme_mode
    };

    match mode {
        crate::config::ThemeMode::Dark => true,
        crate::config::ThemeMode::Light => false,
        crate::config::ThemeMode::Auto => is_dark_mode(),
    }
}

#[cfg(target_os = "windows")]
pub fn read_registry_dword(hkey_root: win32::HKEY, subkey: &str, value_name: &str) -> Option<u32> {
    unsafe {
        let subkey_w = util::to_wstring(subkey);
        let valname_w = util::to_wstring(value_name);
        let mut hkey: win32::HKEY = std::ptr::null_mut();

        let status = win32::RegOpenKeyExW(
            hkey_root,
            subkey_w.as_ptr(),
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
                valname_w.as_ptr(),
                std::ptr::null_mut(),
                &mut type_val,
                data_val.as_mut_ptr(),
                &mut size_val,
            );

            win32::RegCloseKey(hkey);

            if query_status == 0 && size_val == 4 {
                return Some(u32::from_le_bytes(data_val));
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn read_registry_dword(_hkey_root: win32::HKEY, _subkey: &str, _value_name: &str) -> Option<u32> {
    None
}

const PERSONALIZE_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

pub fn is_dark_mode() -> bool {
    read_registry_dword(
        win32::HKEY_CURRENT_USER,
        PERSONALIZE_SUBKEY,
        "AppsUseLightTheme",
    ) == Some(0) // 0 means Dark, 1 means Light
}

pub fn is_system_dark_mode() -> bool {
    read_registry_dword(
        win32::HKEY_CURRENT_USER,
        PERSONALIZE_SUBKEY,
        "SystemUsesLightTheme",
    ) == Some(0) // 0 means Dark, 1 means Light
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
            util::wstr_explorer_dark()
        } else {
            util::wstr_explorer()
        };
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}

pub fn apply_to_control(hwnd: win32::HWND, dark: bool) {
    unsafe {
        let theme = if dark {
            util::wstr_explorer_dark()
        } else {
            util::wstr_explorer()
        };
        win32::SetWindowTheme(hwnd, theme.as_ptr(), std::ptr::null());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_mode_helpers() {
        // Verify default fallback evaluation doesn't panic
        let _ = is_dark_mode();
        let _ = is_system_dark_mode();
    }
}
