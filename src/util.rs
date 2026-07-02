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
            size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1,
            0,
            0,
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
        let _ = fs::write(snippets_dir.join("datetime.j2"), "現在日時: {{ now }}");
        let _ = fs::write(snippets_dir.join("link.j2"), "[{{ input }}]({{ input }})");
        let _ = fs::write(snippets_dir.join("code_block.j2"), "```\n{{ input }}\n```");
    }

    let mut snippets = Vec::new();
    load_snippets_recursive(&snippets_dir, "", &mut snippets);
    snippets
}

fn parse_toml_snippets(value: &toml::Value, current_prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        toml::Value::String(s) if !current_prefix.is_empty() => {
            out.push((current_prefix.to_string(), s.clone()));
        }
        toml::Value::Table(table) => {
            for (k, v) in table {
                let escaped_k = k.replace('/', "\\/");
                let next_prefix = if current_prefix.is_empty() {
                    escaped_k
                } else {
                    format!("{}/{}", current_prefix, escaped_k)
                };
                parse_toml_snippets(v, &next_prefix, out);
            }
        }
        toml::Value::Integer(i) if !current_prefix.is_empty() => {
            out.push((current_prefix.to_string(), i.to_string()));
        }
        toml::Value::Float(f) if !current_prefix.is_empty() => {
            out.push((current_prefix.to_string(), f.to_string()));
        }
        toml::Value::Boolean(b) if !current_prefix.is_empty() => {
            out.push((current_prefix.to_string(), b.to_string()));
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
            } else if path.is_file()
                && let Some(ext) = path.extension()
            {
                if ext == "j2" || ext == "txt" {
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    let name = if prefix.is_empty() {
                        stem
                    } else {
                        format!("{}/{}", prefix, stem)
                    };
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
                    match fs::read_to_string(&path) {
                        Ok(content) => match toml::from_str::<toml::Value>(&content) {
                            Ok(value) => {
                                parse_toml_snippets(&value, &base_prefix, out);
                            }
                            Err(e) => {
                                let filename =
                                    path.file_name().unwrap_or_default().to_string_lossy();
                                let err_msg =
                                    format!("{} のパースに失敗しました:\n{}", filename, e);
                                crate::ui::show_notification(
                                    "スニペット読み込みエラー",
                                    &err_msg,
                                    true,
                                );
                            }
                        },
                        Err(e) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            let err_msg = format!("{} を読み込めませんでした: {}", filename, e);
                            crate::ui::show_notification(
                                "スニペットファイル読み込みエラー",
                                &err_msg,
                                true,
                            );
                        }
                    }
                }
            }
        }
        for d in dirs {
            let dir_name = d.file_name().unwrap().to_string_lossy().to_string();
            let child_prefix = if prefix.is_empty() {
                dir_name
            } else {
                format!("{}/{}", prefix, dir_name)
            };
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
    if history_json.exists()
        && !history_dat.exists()
        && let Ok(content) = fs::read_to_string(&history_json)
        && let Ok(history) = serde_json::from_str::<VecDeque<String>>(&content)
    {
        save_history(&history);
        let _ = fs::remove_file(&history_json);
        return history;
    }

    if history_dat.exists()
        && let Ok(encrypted_content) = fs::read(&history_dat)
        && let Some(decrypted_content) = decrypt_data(&encrypted_content)
        && let Ok(history) = serde_json::from_slice::<VecDeque<String>>(&decrypted_content)
    {
        let mut seen = std::collections::HashSet::new();
        let mut unique_history = VecDeque::new();
        for item in history {
            if seen.insert(item.clone()) {
                unique_history.push_back(item);
            }
        }
        return unique_history;
    }
    VecDeque::new()
}

pub fn save_history(history: &VecDeque<String>) {
    let app_dir = get_app_dir();
    let history_dat = app_dir.join("history.dat");
    if let Ok(content) = serde_json::to_vec(history)
        && let Some(encrypted) = encrypt_data(&content)
    {
        let _ = fs::create_dir_all(&app_dir);
        let _ = fs::write(history_dat, encrypted);
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

static JINJA_ENV: OnceLock<minijinja::Environment<'static>> = OnceLock::new();

pub fn render_template(template_str: &str, clipboard_text: &str) -> String {
    let now = unsafe {
        let mut st = std::mem::zeroed::<SYSTEMTIME>();
        GetLocalTime(&mut st);
        format!(
            "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
            st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second
        )
    };

    let env = JINJA_ENV.get_or_init(|| {
        let mut env = minijinja::Environment::new();

        // Register text transformation filters
        env.add_filter(
            "single_str",
            |val: String| -> Result<String, minijinja::Error> { Ok(to_halfwidth(&val)) },
        );
        env.add_filter(
            "double_str",
            |val: String| -> Result<String, minijinja::Error> { Ok(to_fullwidth(&val)) },
        );
        env.add_filter(
            "hira_str",
            |val: String| -> Result<String, minijinja::Error> { Ok(to_hiragana(&val)) },
        );
        env.add_filter(
            "kata_str",
            |val: String| -> Result<String, minijinja::Error> { Ok(to_katakana(&val)) },
        );
        env.add_filter(
            "urlencode",
            |val: String| -> Result<String, minijinja::Error> { Ok(url_encode(&val)) },
        );
        env.add_filter(
            "urldecode",
            |val: String| -> Result<String, minijinja::Error> { Ok(url_decode(&val)) },
        );
        env.add_filter(
            "base64encode",
            |val: String| -> Result<String, minijinja::Error> { Ok(base64_encode(&val)) },
        );
        env.add_filter(
            "base64decode",
            |val: String| -> Result<String, minijinja::Error> { Ok(base64_decode(&val)) },
        );
        env.add_filter(
            "unicode_escape",
            |val: String| -> Result<String, minijinja::Error> { Ok(unicode_escape(&val)) },
        );
        env.add_filter(
            "unicode_unescape",
            |val: String| -> Result<String, minijinja::Error> { Ok(unicode_unescape(&val)) },
        );
        env.add_filter(
            "html_escape",
            |val: String| -> Result<String, minijinja::Error> { Ok(html_escape(&val)) },
        );
        env.add_filter(
            "html_unescape",
            |val: String| -> Result<String, minijinja::Error> { Ok(html_unescape(&val)) },
        );
        env.add_filter("nfc", |val: String| -> Result<String, minijinja::Error> {
            Ok(to_nfc(&val))
        });
        env.add_filter(
            "to_hex",
            |val: String, prefix: Option<bool>| -> Result<String, minijinja::Error> {
                let clean = val.trim();
                if let Ok(num) = clean.parse::<i64>() {
                    if prefix.unwrap_or(true) {
                        Ok(format!("0x{:X}", num))
                    } else {
                        Ok(format!("{:X}", num))
                    }
                } else {
                    Ok(val)
                }
            },
        );
        env.add_filter(
            "from_hex",
            |val: String| -> Result<String, minijinja::Error> {
                let clean = val.trim().trim_start_matches("0x").trim_start_matches("0X");
                if let Ok(num) = i64::from_str_radix(clean, 16) {
                    Ok(num.to_string())
                } else {
                    Ok(val)
                }
            },
        );

        // Register a custom datetimeformat filter
        env.add_filter(
            "datetimeformat",
            |value: String, fmt: Option<String>| -> Result<String, minijinja::Error> {
                if value.len() < 19 {
                    return Ok(value);
                }
                let year = &value[0..4];
                let month = &value[5..7];
                let day = &value[8..10];
                let hour = &value[11..13];
                let minute = &value[14..16];
                let second = &value[17..19];

                let fmt_str = fmt.as_deref().unwrap_or("long");
                let formatted = match fmt_str {
                    "short" => format!("{}/{}/{}", year, month, day),
                    "time" => format!("{}:{}:{}", hour, minute, second),
                    "year" => year.to_string(),
                    _ => {
                        // Check if it's a format pattern like yyyyMMdd or standard strftime symbols
                        if fmt_str.contains('%')
                            || fmt_str.contains('Y')
                            || fmt_str.contains('y')
                            || fmt_str.contains('m')
                            || fmt_str.contains('M')
                            || fmt_str.contains('d')
                            || fmt_str.contains('D')
                            || fmt_str.contains('H')
                            || fmt_str.contains('h')
                            || fmt_str.contains('s')
                            || fmt_str.contains('S')
                        {
                            fmt_str
                                .replace("%Y", year)
                                .replace("yyyy", year)
                                .replace("YYYY", year)
                                .replace("YY", &year[2..4])
                                .replace("yy", &year[2..4])
                                .replace("%m", month)
                                .replace("MM", month)
                                .replace("%d", day)
                                .replace("dd", day)
                                .replace("DD", day)
                                .replace("%H", hour)
                                .replace("HH", hour)
                                .replace("hh", hour)
                                .replace("%M", minute)
                                .replace("mm", minute)
                                .replace("%S", second)
                                .replace("ss", second)
                                .replace("SS", second)
                        } else {
                            value
                        }
                    }
                };
                Ok(formatted)
            },
        );

        env
    });

    let render_res = env.render_str(
        template_str,
        minijinja::context! {
            now => now,
            input => clipboard_text,
        },
    );

    match render_res {
        Ok(res) => res,
        Err(e) => {
            crate::state::log_debug(&format!(
                "Failed to render template with minijinja: {:?}",
                e
            ));
            template_str.to_string()
        }
    }
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

pub fn get_selection_or_clipboard(active_hwnd: win32::HWND) -> String {
    #[cfg(target_os = "windows")]
    {
        let original_text = get_clipboard_text().unwrap_or_default();

        if active_hwnd.is_null() {
            return original_text;
        }

        let sentinel = "CLIPPER_SENTINEL_987654321";
        if !set_clipboard_text(sentinel) {
            return original_text;
        }

        let inputs = [
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_CONTROL,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_C,
                        w_scan: 0,
                        dw_flags: 0,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_C,
                        w_scan: 0,
                        dw_flags: win32::KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
            win32::INPUT {
                r#type: win32::INPUT_KEYBOARD,
                u: win32::INPUT_union {
                    ki: win32::KEYBDINPUT {
                        w_vk: win32::VK_CONTROL,
                        w_scan: 0,
                        dw_flags: win32::KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            },
        ];
        unsafe {
            win32::SendInput(
                4,
                inputs.as_ptr(),
                std::mem::size_of::<win32::INPUT>() as i32,
            );
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        let new_text = get_clipboard_text().unwrap_or_default();

        if !original_text.is_empty() {
            set_clipboard_text(&original_text);
        } else {
            unsafe {
                if win32::OpenClipboard(std::ptr::null_mut()) != 0 {
                    win32::EmptyClipboard();
                    win32::CloseClipboard();
                }
            }
        }

        if new_text.is_empty() || new_text == sentinel {
            original_text
        } else {
            new_text
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        String::new()
    }
}

pub fn to_halfwidth(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if val == 0x3000 {
                ' '
            } else if (0xFF01..=0xFF5E).contains(&val) {
                char::from_u32(val - 0xFEE0).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

pub fn to_fullwidth(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if val == 0x0020 {
                '\u{3000}'
            } else if (0x0021..=0x007E).contains(&val) {
                char::from_u32(val + 0xFEE0).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

pub fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if (0x30A1..=0x30F6).contains(&val) {
                char::from_u32(val - 0x0060).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

pub fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if (0x3041..=0x3096).contains(&val) {
                char::from_u32(val + 0x0060).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

pub fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
}

pub fn url_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.as_bytes().iter().copied();
    while let Some(b) = chars.next() {
        if b == b'%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let Ok(hex_str) = std::str::from_utf8(&[h1, h2]) {
                    if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                        bytes.push(val);
                        continue;
                    }
                }
            }
            bytes.push(b'%');
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

pub fn base64_encode(s: &str) -> String {
    const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b1 = bytes[i];
        let b2 = if i + 1 < bytes.len() {
            Some(bytes[i + 1])
        } else {
            None
        };
        let b3 = if i + 2 < bytes.len() {
            Some(bytes[i + 2])
        } else {
            None
        };

        let val = ((b1 as u32) << 16) | ((b2.unwrap_or(0) as u32) << 8) | (b3.unwrap_or(0) as u32);

        result.push(BASE64_CHARS[((val >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((val >> 12) & 0x3F) as usize] as char);
        if b2.is_some() {
            result.push(BASE64_CHARS[((val >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if b3.is_some() {
            result.push(BASE64_CHARS[(val & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

pub fn base64_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in s.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => continue,
            _ => continue,
        };
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

pub fn unicode_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if val <= 0x7F && !c.is_ascii_control() {
                c.to_string()
            } else if val <= 0xFFFF {
                format!("\\u{:04x}", val)
            } else {
                format!("\\u{{{:x}}}", val)
            }
        })
        .collect()
}

pub fn unicode_unescape(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            if chars[i + 1] == 'u' {
                if i + 2 < chars.len() && chars[i + 2] == '{' {
                    let mut j = i + 3;
                    while j < chars.len() && chars[j] != '}' {
                        j += 1;
                    }
                    if j < chars.len() {
                        let hex_str: String = chars[i + 3..j].iter().collect();
                        if let Ok(val) = u32::from_str_radix(&hex_str, 16) {
                            if let Some(c) = char::from_u32(val) {
                                result.push(c);
                                i = j + 1;
                                continue;
                            }
                        }
                    }
                } else if i + 5 < chars.len() {
                    let hex_str: String = chars[i + 2..=i + 5].iter().collect();
                    if let Ok(val) = u32::from_str_radix(&hex_str, 16) {
                        if let Some(c) = char::from_u32(val) {
                            result.push(c);
                            i += 6;
                            continue;
                        }
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

pub fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            let val = c as u32;
            if val <= 0x7F
                && !c.is_ascii_control()
                && c != '&'
                && c != '<'
                && c != '>'
                && c != '"'
                && c != '\''
            {
                c.to_string()
            } else {
                format!("&#x{:X};", val)
            }
        })
        .collect()
}

pub fn html_unescape(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' && i + 1 < chars.len() && chars[i + 1] == '#' {
            let mut j = i + 2;
            let mut is_hex = false;
            if j < chars.len() && (chars[j] == 'x' || chars[j] == 'X') {
                is_hex = true;
                j += 1;
            }
            let start = j;
            while j < chars.len() && chars[j] != ';' {
                j += 1;
            }
            if j < chars.len() {
                let num_str: String = chars[start..j].iter().collect();
                let val = if is_hex {
                    u32::from_str_radix(&num_str, 16).ok()
                } else {
                    num_str.parse::<u32>().ok()
                };
                if let Some(val) = val {
                    if let Some(c) = char::from_u32(val) {
                        result.push(c);
                        i = j + 1;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

pub fn to_nfc(s: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        let src_w = to_wstring(s);
        unsafe {
            let len = win32::NormalizeString(
                1,
                src_w.as_ptr(),
                src_w.len() as i32,
                std::ptr::null_mut(),
                0,
            );
            if len > 0 {
                let mut dst = vec![0u16; len as usize];
                let res = win32::NormalizeString(
                    1,
                    src_w.as_ptr(),
                    src_w.len() as i32,
                    dst.as_mut_ptr(),
                    len,
                );
                if res > 0 {
                    let slice = &dst[..res as usize];
                    return String::from_utf16_lossy(slice);
                }
            }
        }
        s.to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        s.to_string()
    }
}

pub fn split_path(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '/' {
            current.push('/');
            i += 2;
        } else if chars[i] == '/' {
            parts.push(current.clone());
            current.clear();
            i += 1;
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }
    parts.push(current);
    parts
}

pub fn join_path(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| p.replace('/', "\\/"))
        .collect::<Vec<String>>()
        .join("/")
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
            datetime = "現在日時: {{ now }}"
            number = 123
            boolean = true

            [git]
            commit = "feat: {{ input }}"
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
                ("datetime".to_string(), "現在日時: {{ now }}".to_string()),
                ("dev/rust/struct".to_string(), "struct MyStruct".to_string()),
                ("git/commit".to_string(), "feat: {{ input }}".to_string()),
                ("git/push".to_string(), "git push".to_string()),
                ("number".to_string(), "123".to_string()),
            ]
        );
    }

    #[test]
    fn test_render_template() {
        let template = "Clipboard: {{ input }}, Date: {{ now|datetimeformat('yyyy-MM-DD') }}, CustomDate: {{ now|datetimeformat('yyyyMMDD') }}";
        let rendered = render_template(template, "hello");

        let parts: Vec<&str> = rendered.split(", ").collect();
        assert_eq!(parts[0], "Clipboard: hello");

        let date_part = parts[1].strip_prefix("Date: ").unwrap();
        assert_eq!(date_part.len(), 10);
        assert_eq!(&date_part[4..5], "-");
        assert_eq!(&date_part[7..8], "-");

        let custom_date_part = parts[2].strip_prefix("CustomDate: ").unwrap();
        assert_eq!(custom_date_part.len(), 8);
        assert!(custom_date_part.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_text_transformation_filters() {
        // Halfwidth / Fullwidth
        assert_eq!(
            render_template("{{ input|single_str }}", "ＡＢＣ １２３"),
            "ABC 123"
        );
        assert_eq!(
            render_template("{{ input|double_str }}", "ABC 123"),
            "ＡＢＣ\u{3000}１２３"
        );

        // Hiragana / Katakana
        assert_eq!(
            render_template("{{ input|hira_str }}", "アイウエオ"),
            "あいうえお"
        );
        assert_eq!(
            render_template("{{ input|kata_str }}", "あいうえお"),
            "アイウエオ"
        );

        // URL encode / decode
        assert_eq!(
            render_template("{{ input|urlencode }}", "hello world!"),
            "hello%20world%21"
        );
        assert_eq!(
            render_template("{{ input|urldecode }}", "hello%20world%21"),
            "hello world!"
        );

        // Base64 encode / decode
        assert_eq!(
            render_template("{{ input|base64encode }}", "Rust"),
            "UnVzdA=="
        );
        assert_eq!(
            render_template("{{ input|base64decode }}", "UnVzdA=="),
            "Rust"
        );

        // Unicode escape / unescape
        assert_eq!(
            render_template("{{ input|unicode_escape }}", "あ"),
            "\\u3042"
        );
        assert_eq!(
            render_template("{{ input|unicode_unescape }}", "\\u3042"),
            "あ"
        );

        // HTML escape / unescape
        assert_eq!(render_template("{{ input|html_escape }}", "あ"), "&#x3042;");
        assert_eq!(
            render_template("{{ input|html_unescape }}", "&#x3042;"),
            "あ"
        );

        // Hex / Dec conversions
        assert_eq!(render_template("{{ input|to_hex }}", "255"), "0xFF");
        assert_eq!(render_template("{{ input|to_hex(false) }}", "255"), "FF");
        assert_eq!(render_template("{{ input|from_hex }}", "0xFF"), "255");
        assert_eq!(render_template("{{ input|from_hex }}", "FF"), "255");
    }

    #[test]
    fn test_escaped_path() {
        // Path splitting and joining with escaped slashes
        assert_eq!(split_path("git/commit"), vec!["git", "commit"]);
        assert_eq!(split_path("yyyy\\/mm\\/dd"), vec!["yyyy/mm/dd"]);
        assert_eq!(split_path("date\\/time/now"), vec!["date/time", "now"]);

        assert_eq!(
            join_path(&["git".to_string(), "commit".to_string()]),
            "git/commit"
        );
        assert_eq!(join_path(&["yyyy/mm/dd".to_string()]), "yyyy\\/mm\\/dd");

        // TOML parsing with slashes in key
        let toml_content = r#"
            "yyyy/mm/dd" = "{{ now|datetimeformat('YYYY/MM/DD') }}"
        "#;
        let value: toml::Value = toml::from_str(toml_content).unwrap();
        let mut out = Vec::new();
        parse_toml_snippets(&value, "", &mut out);
        assert_eq!(
            out,
            vec![(
                "yyyy\\/mm\\/dd".to_string(),
                "{{ now|datetimeformat('YYYY/MM/DD') }}".to_string()
            )]
        );
    }
}
