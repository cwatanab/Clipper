use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use crate::win32;

pub fn to_wstring(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn create_ui_font(name: &str, size: i32, weight: i32) -> win32::HFONT {
    let name_w = to_wstring(name);
    unsafe {
        win32::CreateFontW(
            size, 0, 0, 0,
            weight,
            0, 0, 0,
            1,
            0, 0,
            5,
            0,
            name_w.as_ptr(),
        )
    }
}

pub fn get_app_dir() -> PathBuf {
    if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("clipper")
    } else {
        PathBuf::from(".")
    }
}

pub fn load_snippets() -> Vec<(String, String)> {
    let app_dir = get_app_dir();
    let snippets_dir = app_dir.join("snippets");
    if !snippets_dir.exists() {
        let _ = fs::create_dir_all(&snippets_dir);
        let _ = fs::write(snippets_dir.join("datetime.j2"), "現在日時: {{ datetime }}");
        let _ = fs::write(snippets_dir.join("link.j2"), "[{{ clipboard }}]({{ clipboard }})");
        let _ = fs::write(snippets_dir.join("code_block.j2"), "```\n{{ clipboard }}\n```");
    }

    let mut snippets = Vec::new();
    load_snippets_recursive(&snippets_dir, "", &mut snippets);
    snippets
}

fn parse_toml_snippets(value: &toml::Value, current_prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        toml::Value::String(s) => {
            if !current_prefix.is_empty() {
                out.push((current_prefix.to_string(), s.clone()));
            }
        }
        toml::Value::Table(table) => {
            for (k, v) in table {
                let next_prefix = if current_prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}/{}", current_prefix, k)
                };
                parse_toml_snippets(v, &next_prefix, out);
            }
        }
        toml::Value::Integer(i) => {
            if !current_prefix.is_empty() {
                out.push((current_prefix.to_string(), i.to_string()));
            }
        }
        toml::Value::Float(f) => {
            if !current_prefix.is_empty() {
                out.push((current_prefix.to_string(), f.to_string()));
            }
        }
        toml::Value::Boolean(b) => {
            if !current_prefix.is_empty() {
                out.push((current_prefix.to_string(), b.to_string()));
            }
        }
        _ => {}
    }
}

fn load_snippets_recursive(dir: &PathBuf, prefix: &str, out: &mut Vec<(String, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "j2" || ext == "txt" {
                        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                        let name = if prefix.is_empty() { stem } else { format!("{}/{}", prefix, stem) };
                        if let Ok(content) = fs::read_to_string(&path) {
                            out.push((name, content));
                        }
                    } else if ext == "toml" {
                        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                        let base_prefix = if stem == "snippets" {
                            prefix.to_string()
                        } else if prefix.is_empty() {
                            stem
                        } else {
                            format!("{}/{}", prefix, stem)
                        };
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                                parse_toml_snippets(&value, &base_prefix, out);
                            }
                        }
                    }
                }
            }
        }
        for d in dirs {
            let dir_name = d.file_name().unwrap().to_string_lossy().to_string();
            let child_prefix = if prefix.is_empty() { dir_name } else { format!("{}/{}", prefix, dir_name) };
            load_snippets_recursive(&d, &child_prefix, out);
        }
    }
}

fn encrypt_data(data: &[u8]) -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    unsafe {
        let data_in = win32::DATA_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = std::mem::zeroed::<win32::DATA_BLOB>();
        let ret = win32::CryptProtectData(
            &data_in,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut data_out,
        );
        if ret != 0 {
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let res = slice.to_vec();
            win32::LocalFree(data_out.pbData as *mut std::ffi::c_void);
            Some(res)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(data.to_vec())
    }
}

fn decrypt_data(data: &[u8]) -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    unsafe {
        let data_in = win32::DATA_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = std::mem::zeroed::<win32::DATA_BLOB>();
        let ret = win32::CryptUnprotectData(
            &data_in,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut data_out,
        );
        if ret != 0 {
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let res = slice.to_vec();
            win32::LocalFree(data_out.pbData as *mut std::ffi::c_void);
            Some(res)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(data.to_vec())
    }
}

pub fn load_history() -> VecDeque<String> {
    let app_dir = get_app_dir();
    let history_dat = app_dir.join("history.dat");
    let history_json = app_dir.join("history.json");

    // Migration logic: if history.json exists and history.dat doesn't, encrypt and migrate it
    if history_json.exists() && !history_dat.exists() {
        if let Ok(content) = fs::read_to_string(&history_json) {
            if let Ok(history) = serde_json::from_str::<VecDeque<String>>(&content) {
                save_history(&history);
                let _ = fs::remove_file(&history_json);
                return history;
            }
        }
    }

    if history_dat.exists() {
        if let Ok(encrypted_content) = fs::read(&history_dat) {
            if let Some(decrypted_content) = decrypt_data(&encrypted_content) {
                if let Ok(history) = serde_json::from_slice::<VecDeque<String>>(&decrypted_content) {
                    let mut seen = std::collections::HashSet::new();
                    let mut unique_history = VecDeque::new();
                    for item in history {
                        if seen.insert(item.clone()) {
                            unique_history.push_back(item);
                        }
                    }
                    return unique_history;
                }
            }
        }
    }
    VecDeque::new()
}

pub fn save_history(history: &VecDeque<String>) {
    let app_dir = get_app_dir();
    let history_dat = app_dir.join("history.dat");
    if let Ok(content) = serde_json::to_vec(history) {
        if let Some(encrypted) = encrypt_data(&content) {
            let _ = fs::create_dir_all(&app_dir);
            let _ = fs::write(history_dat, encrypted);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SYSTEMTIME {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
}

#[cfg(not(target_os = "windows"))]
unsafe fn GetLocalTime(_lpSystemTime: *mut SYSTEMTIME) {}

pub fn render_template(template_str: &str, clipboard_text: &str) -> String {
    let now = unsafe {
        let mut st = std::mem::zeroed::<SYSTEMTIME>();
        GetLocalTime(&mut st);
        format!(
            "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
            st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second
        )
    };
    template_str
        .replace("{{ datetime }}", &now)
        .replace("{{ clipboard }}", clipboard_text)
}

pub fn get_clipboard_text() -> Option<String> {
    #[cfg(target_os = "windows")]
    unsafe {
        if win32::OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h_data = win32::GetClipboardData(win32::CF_UNICODETEXT);
        if h_data.is_null() {
            win32::CloseClipboard();
            return None;
        }
        let p_data = win32::GlobalLock(h_data) as *const u16;
        if p_data.is_null() {
            win32::CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *p_data.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(p_data, len);
        let text = String::from_utf16_lossy(slice);
        win32::GlobalUnlock(h_data);
        win32::CloseClipboard();
        Some(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

pub fn set_clipboard_text(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        if win32::OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        if win32::EmptyClipboard() == 0 {
            win32::CloseClipboard();
            return false;
        }
        let text_w = to_wstring(text);
        let bytes_len = text_w.len() * std::mem::size_of::<u16>();
        let h_mem = win32::GlobalAlloc(win32::GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            win32::CloseClipboard();
            return false;
        }
        let p_mem = win32::GlobalLock(h_mem) as *mut u16;
        if p_mem.is_null() {
            win32::GlobalFree(h_mem);
            win32::CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(text_w.as_ptr(), p_mem, text_w.len());
        win32::GlobalUnlock(h_mem);
        if win32::SetClipboardData(win32::CF_UNICODETEXT, h_mem).is_null() {
            win32::GlobalFree(h_mem);
            win32::CloseClipboard();
            return false;
        }
        win32::CloseClipboard();
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

use std::sync::OnceLock;

macro_rules! define_wstring_cache {
    ($name:ident, $val:expr) => {
        pub fn $name() -> &'static [u16] {
            static CACHE: OnceLock<Vec<u16>> = OnceLock::new();
            CACHE.get_or_init(|| to_wstring($val))
        }
    };
}

define_wstring_cache!(wstr_edit, "EDIT");
define_wstring_cache!(wstr_listbox, "LISTBOX");
define_wstring_cache!(wstr_cue, "検索 (Migemo)...");
define_wstring_cache!(wstr_explorer_dark, "DarkMode_Explorer");
define_wstring_cache!(wstr_explorer, "Explorer");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_snippets() {
        let toml_content = r#"
            datetime = "現在日時: {{ datetime }}"
            number = 123
            boolean = true

            [git]
            commit = "feat: {{ clipboard }}"
            push = "git push"

            [dev.rust]
            struct = "struct MyStruct"
        "#;
        let value: toml::Value = toml::from_str(toml_content).unwrap();
        let mut out = Vec::new();
        parse_toml_snippets(&value, "", &mut out);

        out.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            out,
            vec![
                ("boolean".to_string(), "true".to_string()),
                ("datetime".to_string(), "現在日時: {{ datetime }}".to_string()),
                ("dev/rust/struct".to_string(), "struct MyStruct".to_string()),
                ("git/commit".to_string(), "feat: {{ clipboard }}".to_string()),
                ("git/push".to_string(), "git push".to_string()),
                ("number".to_string(), "123".to_string()),
            ]
        );
    }
}
