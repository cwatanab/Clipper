use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use chrono::Local;
use minijinja::{context, Environment};

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
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("clipper");
    path
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

fn load_snippets_recursive(dir: &PathBuf, prefix: &str, out: &mut Vec<(String, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() && path.extension().map_or(false, |ext| ext == "j2" || ext == "txt") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let name = if prefix.is_empty() { stem } else { format!("{}/{}", prefix, stem) };
                if let Ok(content) = fs::read_to_string(&path) {
                    out.push((name, content));
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

pub fn render_template(template_str: &str, clipboard_text: &str) -> String {
    let mut env = Environment::new();
    if let Err(_) = env.add_template("temp", template_str) {
        return template_str.to_string();
    }
    if let Ok(tmpl) = env.get_template("temp") {
        let now = Local::now().format("%Y/%m/%d %H:%M:%S").to_string();
        tmpl.render(context! {
            datetime => now,
            clipboard => clipboard_text,
        }).unwrap_or_else(|_| template_str.to_string())
    } else {
        template_str.to_string()
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
