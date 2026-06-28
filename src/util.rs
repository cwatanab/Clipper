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

pub fn create_ui_font(name: &str, size: i32) -> win32::HFONT {
    let name_w = to_wstring(name);
    unsafe {
        win32::CreateFontW(
            size, 0, 0, 0,
            400,
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

pub fn load_history() -> VecDeque<String> {
    let app_dir = get_app_dir();
    let history_file = app_dir.join("history.json");
    if history_file.exists() {
        if let Ok(content) = fs::read_to_string(history_file) {
            if let Ok(history) = serde_json::from_str::<VecDeque<String>>(&content) {
                return history;
            }
        }
    }
    VecDeque::new()
}

pub fn save_history(history: &VecDeque<String>) {
    let app_dir = get_app_dir();
    let history_file = app_dir.join("history.json");
    if let Ok(content) = serde_json::to_string_pretty(history) {
        let _ = fs::create_dir_all(&app_dir);
        let _ = fs::write(history_file, content);
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
