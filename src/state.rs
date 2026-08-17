use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32};

use crate::win32;

// カスタムメッセージ
pub const WM_TRIGGER_SNIPPET: u32 = 0x8000 + 2;
pub const WM_TRIGGER_HISTORY: u32 = 0x8000 + 3;
pub const WM_FILTER_COMPLETE: u32 = 0x8000 + 5;
pub const WM_HIDE_WINDOW: u32 = 0x8000 + 6;
pub const WM_FIFO_LIFO_PASTE: u32 = 0x8000 + 7;
pub const WM_TOGGLE_FIFO_LIFO: u32 = 0x8000 + 8;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Snippet,
    History,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FifoLifoMode {
    None,
    Fifo,
    Lifo,
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
    pub current_selection: String,
    pub last_active_window: Option<usize>,
    pub is_dark: bool,
    pub current_folder: String,
    pub top_index: usize,
    pub filter_generation: u32,
    pub fifo_lifo_mode: FifoLifoMode,
    pub fifo_lifo_queue: VecDeque<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct SafeHWND(pub win32::HWND);
unsafe impl Send for SafeHWND {}
unsafe impl Sync for SafeHWND {}

#[derive(Clone, Copy, PartialEq)]
pub struct SafeHHOOK(pub win32::HHOOK);
unsafe impl Send for SafeHHOOK {}
unsafe impl Sync for SafeHHOOK {}

pub type EditWndProc =
    unsafe extern "system" fn(win32::HWND, u32, win32::WPARAM, win32::LPARAM) -> win32::LRESULT;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum KeyTriggerKind {
    None = 0,
    Shift = 1,
    LShift = 2,
    RShift = 3,
    Ctrl = 4,
    LCtrl = 5,
    RCtrl = 6,
    Alt = 7,
    LAlt = 8,
    RAlt = 9,
}

impl KeyTriggerKind {
    pub fn parse(key_name: &str) -> Self {
        match key_name.to_lowercase().as_str() {
            "shift" => KeyTriggerKind::Shift,
            "lshift" | "left_shift" => KeyTriggerKind::LShift,
            "rshift" | "right_shift" => KeyTriggerKind::RShift,
            "ctrl" | "control" => KeyTriggerKind::Ctrl,
            "lctrl" | "left_ctrl" | "lcontrol" | "left_control" => KeyTriggerKind::LCtrl,
            "rctrl" | "right_ctrl" | "rcontrol" | "right_control" => KeyTriggerKind::RCtrl,
            "alt" | "menu" => KeyTriggerKind::Alt,
            "lalt" | "left_alt" | "lmenu" | "left_menu" => KeyTriggerKind::LAlt,
            "ralt" | "right_alt" | "rmenu" | "right_menu" => KeyTriggerKind::RAlt,
            _ => KeyTriggerKind::None,
        }
    }

    #[inline(always)]
    pub fn matches(&self, vk: u16) -> bool {
        match self {
            KeyTriggerKind::Shift => {
                vk == win32::VK_SHIFT || vk == win32::VK_LSHIFT || vk == win32::VK_RSHIFT
            }
            KeyTriggerKind::LShift => vk == win32::VK_LSHIFT,
            KeyTriggerKind::RShift => vk == win32::VK_RSHIFT,
            KeyTriggerKind::Ctrl => {
                vk == win32::VK_CONTROL || vk == win32::VK_LCONTROL || vk == win32::VK_RCONTROL
            }
            KeyTriggerKind::LCtrl => vk == win32::VK_LCONTROL,
            KeyTriggerKind::RCtrl => vk == win32::VK_RCONTROL,
            KeyTriggerKind::Alt => {
                vk == win32::VK_MENU || vk == win32::VK_LMENU || vk == win32::VK_RMENU
            }
            KeyTriggerKind::LAlt => vk == win32::VK_LMENU,
            KeyTriggerKind::RAlt => vk == win32::VK_RMENU,
            KeyTriggerKind::None => false,
        }
    }

    #[inline(always)]
    pub fn from_u8(val: u8) -> Self {
        if val <= 9 { unsafe { std::mem::transmute(val) } } else { KeyTriggerKind::None }
    }
}

pub static LAST_KEY_VK: AtomicU32 = AtomicU32::new(0);
pub static LAST_KEY_TIME: AtomicU32 = AtomicU32::new(0);
pub static LAST_KEYDOWN_TIME: AtomicU32 = AtomicU32::new(0);
pub static OTHER_KEY_PRESSED: AtomicBool = AtomicBool::new(false);
pub static SAVE_HISTORY_TO_FILE: AtomicBool = AtomicBool::new(true);
pub static SHOW_TABS: AtomicBool = AtomicBool::new(true);
pub static LAST_SHOW_TIME: AtomicU32 = AtomicU32::new(0);
pub static FIFO_LIFO_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static FIFO_LIFO_HAS_ITEMS: AtomicBool = AtomicBool::new(false);
pub static SNIPPET_KEY_KIND: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(KeyTriggerKind::LShift as u8);
pub static HISTORY_KEY_KIND: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(KeyTriggerKind::LCtrl as u8);
pub static DOUBLE_TAP_MS: AtomicU32 = AtomicU32::new(300);
pub const CLIPPER_MAGIC_INFO: usize = 0x12345678;
pub static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

pub fn sync_fifo_lifo_state(state: &AppState) {
    FIFO_LIFO_ACTIVE.store(
        state.fifo_lifo_mode != FifoLifoMode::None,
        std::sync::atomic::Ordering::Release,
    );
    FIFO_LIFO_HAS_ITEMS.store(
        !state.fifo_lifo_queue.is_empty(),
        std::sync::atomic::Ordering::Release,
    );
}

pub fn update_hook_config(config: &Config) {
    let snippet_kind = KeyTriggerKind::parse(&config.snippet_key);
    let history_kind = KeyTriggerKind::parse(&config.history_key);
    SNIPPET_KEY_KIND.store(snippet_kind as u8, std::sync::atomic::Ordering::Release);
    HISTORY_KEY_KIND.store(history_kind as u8, std::sync::atomic::Ordering::Release);
    DOUBLE_TAP_MS.store(config.double_tap_ms, std::sync::atomic::Ordering::Release);
}

/// Poison-safe lock helper for APP_STATE.
/// Recovers from poisoned mutex instead of panicking.
pub fn lock_state() -> std::sync::MutexGuard<'static, Option<AppState>> {
    APP_STATE.lock().unwrap_or_else(|e| e.into_inner())
}

use crate::config::Config;
pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub static MAIN_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static EDIT_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static LISTBOX_HWND: OnceLock<SafeHWND> = OnceLock::new();
pub static OLD_EDIT_PROC: OnceLock<SafeWndProc> = OnceLock::new();
pub static OLD_LISTBOX_PROC: OnceLock<SafeWndProc> = OnceLock::new();
pub static MOUSE_HOOK: Mutex<Option<SafeHHOOK>> = Mutex::new(None);
use rustmigemo::migemo::compact_dictionary::CompactDictionary;

pub fn get_migemo_dict() -> Option<&'static CompactDictionary> {
    crate::dict::get_dictionary()
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
pub static FONT_ICONS_16: Mutex<Option<SafeHFONT>> = Mutex::new(None);
pub static FONT_ICONS_18: Mutex<Option<SafeHFONT>> = Mutex::new(None);

pub fn log_debug(_msg: &str) {}

pub fn start_logging_thread() {}

pub static HISTORY_SAVE_SENDER: OnceLock<std::sync::mpsc::Sender<Arc<VecDeque<String>>>> = OnceLock::new();

pub fn init_history_saver() {
    let (tx, rx) = std::sync::mpsc::channel::<Arc<VecDeque<String>>>();
    if HISTORY_SAVE_SENDER.set(tx).is_ok() {
        std::thread::spawn(move || {
            let mut pending_data: Option<Arc<VecDeque<String>>> = None;
            loop {
                let timeout = if pending_data.is_some() {
                    std::time::Duration::from_millis(200)
                } else {
                    std::time::Duration::MAX
                };

                match rx.recv_timeout(timeout) {
                    Ok(data) => {
                        pending_data = Some(data);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(data) = pending_data.take() {
                            crate::util::save_history(&data);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        if let Some(data) = pending_data.take() {
                            crate::util::save_history(&data);
                        }
                        break;
                    }
                }
            }
        });
    }
}

pub fn flush_history_saver() {
    if !SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let history_opt = {
        let state_guard = lock_state();
        state_guard.as_ref().map(|s| Arc::clone(&s.history))
    };
    if let Some(history) = history_opt {
        crate::util::save_history(&history);
    }
}

