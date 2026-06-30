use std::collections::VecDeque;
use std::sync::atomic::AtomicU32;
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::win32;

// カスタムメッセージ
pub const WM_TRIGGER_SNIPPET: u32 = 0x8000 + 2;
pub const WM_TRIGGER_HISTORY: u32 = 0x8000 + 3;
pub const WM_FILTER_COMPLETE: u32 = 0x8000 + 5;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Snippet,
    History,
}

use std::sync::Arc;

pub struct AppState {
    pub history: Arc<VecDeque<String>>,
    pub snippets: Arc<Vec<(String, String)>>,
    pub mode: Mode,
    pub visible: bool,
    pub current_results: Vec<String>,
    pub current_full_paths: Vec<String>,
    pub last_clipboard_value: String,
    pub last_active_window: Option<usize>,
    pub is_dark: bool,
    pub current_folder: String,
    pub top_index: usize,
    pub filter_generation: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub struct SafeHWND(pub win32::HWND);
unsafe impl Send for SafeHWND {}
unsafe impl Sync for SafeHWND {}

pub type EditWndProc = unsafe extern "system" fn(win32::HWND, u32, win32::WPARAM, win32::LPARAM) -> win32::LRESULT;
pub struct SafeWndProc(pub EditWndProc);
unsafe impl Send for SafeWndProc {}
unsafe impl Sync for SafeWndProc {}

#[derive(Clone, Copy, Debug)]
pub struct SafeHBRUSH(pub win32::HBRUSH);
unsafe impl Send for SafeHBRUSH {}
unsafe impl Sync for SafeHBRUSH {}

#[derive(Clone, Copy, Debug)]
pub struct SafeHFONT(pub win32::HFONT);
unsafe impl Send for SafeHFONT {}
unsafe impl Sync for SafeHFONT {}

pub static LAST_KEY_VK: AtomicU32 = AtomicU32::new(0);
pub static LAST_KEY_TIME: AtomicU32 = AtomicU32::new(0);
pub static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

use crate::config::Config;
pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub static MAIN_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static EDIT_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static LISTBOX_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static OLD_EDIT_PROC: OnceLock<SafeWndProc> = OnceLock::new();
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
pub static MIGEMO_DICT: Mutex<Option<Arc<CompactDictionary>>> = Mutex::new(None);

pub fn get_migemo_dict() -> Option<Arc<CompactDictionary>> {
    let mut guard = MIGEMO_DICT.lock().unwrap();
    if guard.is_none() {
        if let Some(dict) = crate::dict::load() {
            *guard = Some(Arc::new(dict));
        }
    }
    guard.clone()
}

pub fn clear_migemo_dict() {
    let mut guard = MIGEMO_DICT.lock().unwrap();
    *guard = None;
}

pub static BRUSH_BG: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static BRUSH_CTRL: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static BRUSH_EDIT: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static BRUSH_LISTBOX: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static BRUSH_BORDER: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static BRUSH_SEL_BG: Mutex<Option<SafeHBRUSH>> = Mutex::new(None);
pub static FONT_EDIT: Mutex<Option<SafeHFONT>> = Mutex::new(None);
pub static FONT_LISTBOX: Mutex<Option<SafeHFONT>> = Mutex::new(None);
pub static FONT_LISTBOX_BOLD: Mutex<Option<SafeHFONT>> = Mutex::new(None);

pub fn log_debug(_msg: &str) {}

pub fn start_logging_thread() {}
