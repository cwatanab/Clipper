#![windows_subsystem = "windows"]

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use chrono::Local;
use minijinja::{context, Environment};
use once_cell::sync::Lazy;
use regex::Regex;

use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use rustmigemo::migemo::romaji_processor::RomajiProcessor;

#[cfg(target_os = "windows")]
mod win32 {
    use std::ffi::c_void;

    pub type HWND = *mut c_void;
    pub type HMENU = *mut c_void;
    pub type HHOOK = *mut c_void;
    pub type HINSTANCE = *mut c_void;
    pub type HBRUSH = *mut c_void;
    pub type HIMC = *mut c_void;
    pub type HDC = *mut c_void;
    pub type HGDIOBJ = *mut c_void;
    pub type HFONT = *mut c_void;
    pub type HCURSOR = *mut c_void;
    pub type BOOL = i32;
    pub type LRESULT = isize;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LONG_PTR = isize;

    pub type HOOKPROC = Option<unsafe extern "system" fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT>;
    pub type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct WNDCLASSW {
        pub style: u32,
        pub lpfnWndProc: WNDPROC,
        pub cbClsExtra: i32,
        pub cbWndExtra: i32,
        pub hInstance: HINSTANCE,
        pub hIcon: *mut c_void,
        pub hCursor: HCURSOR,
        pub hbrBackground: HBRUSH,
        pub lpszMenuName: *const u16,
        pub lpszClassName: *const u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct POINT {
        pub x: i32,
        pub y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MSG {
        pub hwnd: HWND,
        pub message: u32,
        pub wparam: WPARAM,
        pub lparam: LPARAM,
        pub time: u32,
        pub pt: POINT,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct NOTIFYICONDATAW {
        pub cbSize: u32,
        pub hWnd: HWND,
        pub uID: u32,
        pub uFlags: u32,
        pub uCallbackMessage: u32,
        pub hIcon: *mut c_void,
        pub szTip: [u16; 128],
        pub dwState: u32,
        pub dwStateMask: u32,
        pub szInfo: [u16; 256],
        pub uTimeoutOrVersion: u32,
        pub szInfoTitle: [u16; 64],
        pub dwInfoFlags: u32,
        pub guidItem: [u8; 16],
        pub hBalloonIcon: *mut c_void,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct KBDLLHOOKSTRUCT {
        pub vk_code: u32,
        pub scan_code: u32,
        pub flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct GUITHREADINFO {
        pub cbSize: u32,
        pub flags: u32,
        pub hwndActive: HWND,
        pub hwndFocus: HWND,
        pub hwndCapture: HWND,
        pub hwndMenuOwner: HWND,
        pub hwndMoveSize: HWND,
        pub hwndCaret: HWND,
        pub rcCaret: RECT,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct KEYBDINPUT {
        pub w_vk: u16,
        pub w_scan: u16,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union INPUT_union {
        pub ki: KEYBDINPUT,
        pub align: [u8; 32],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub r#type: u32,
        pub u: INPUT_union,
    }

    pub const WH_KEYBOARD_LL: i32 = 13;
    pub const WM_KEYDOWN: u32 = 0x0100;
    pub const WM_KEYUP: u32 = 0x0101;
    pub const WM_SYSKEYDOWN: u32 = 0x0104;
    pub const WM_SYSKEYUP: u32 = 0x0105;

    pub const VK_SHIFT: u16 = 0x10;
    pub const VK_CONTROL: u16 = 0x11;
    pub const VK_LSHIFT: u16 = 0xA0;
    pub const VK_RSHIFT: u16 = 0xA1;
    pub const VK_LCONTROL: u16 = 0xA2;
    pub const VK_RCONTROL: u16 = 0xA3;
    pub const VK_V: u16 = 0x56;

    pub const INPUT_KEYBOARD: u32 = 1;
    pub const KEYEVENTF_KEYUP: u32 = 2;

    pub const CS_HREDRAW: u32 = 2;
    pub const CS_VREDRAW: u32 = 1;
    pub const IDC_ARROW: *const u16 = 32512 as *const u16;

    pub const WS_POPUP: u32 = 0x80000000;
    pub const WS_BORDER: u32 = 0x00800000;
    pub const WS_DLGFRAME: u32 = 0x00400000;
    pub const WS_CHILD: u32 = 0x40000000;
    pub const WS_VISIBLE: u32 = 0x10000000;
    pub const WS_VSCROLL: u32 = 0x00200000;
    pub const ES_AUTOHSCROLL: u32 = 0x0080;
    pub const ES_LEFT: u32 = 0x0000;
    pub const LBS_NOTIFY: u32 = 0x0001;
    pub const LBS_HASSTRINGS: u32 = 0x0040;
    pub const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;

    pub const WS_EX_TOPMOST: u32 = 0x00000008;
    pub const WS_EX_TOOLWINDOW: u32 = 0x00000080;
    pub const WS_EX_CLIENTEDGE: u32 = 0x00000200;

    pub const COLOR_3DFACE: u32 = 15;
    pub const COLOR_WINDOW: u32 = 5;

    pub const GWLP_WNDPROC: i32 = -4;

    pub const EN_CHANGE: u16 = 0x0300;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_CREATE: u32 = 0x0001;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_SETFONT: u32 = 0x0030;
    pub const WM_ACTIVATE: u32 = 0x0006;
    pub const WA_INACTIVE: usize = 0;

    pub const LB_ADDSTRING: u32 = 0x0180;
    pub const LB_RESETCONTENT: u32 = 0x0184;
    pub const LB_GETCURSEL: u32 = 0x0188;
    pub const LB_SETCURSEL: u32 = 0x0186;
    pub const LB_GETTEXT: u32 = 0x0189;
    pub const LB_GETTEXTLEN: u32 = 0x018A;
    pub const LB_ERR: isize = -1;

    pub const WM_CTLCOLOREDIT: u32 = 0x0133;
    pub const WM_CTLCOLORLISTBOX: u32 = 0x0134;

    pub const NIM_ADD: u32 = 0;
    pub const NIM_DELETE: u32 = 2;
    pub const NIF_MESSAGE: u32 = 1;
    pub const NIF_ICON: u32 = 2;
    pub const NIF_TIP: u32 = 4;
    pub const WM_TRAYICON: u32 = 0x8000 + 1;
    pub const WM_LBUTTONDBLCLK: usize = 0x0203;
    pub const WM_RBUTTONUP: usize = 0x0205;

    pub const TPM_RETURNCMD: u32 = 0x0100;
    pub const TPM_LEFTALIGN: u32 = 0x0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetForegroundWindow() -> HWND;
        pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
        pub fn IsWindow(hwnd: HWND) -> BOOL;
        
        pub fn SetWindowsHookExW(
            id_hook: i32,
            lpfn: HOOKPROC,
            hmod: HINSTANCE,
            dw_thread_id: u32,
        ) -> HHOOK;
        
        pub fn UnhookWindowsHookEx(hhk: HHOOK) -> BOOL;
        
        pub fn CallNextHookEx(
            hhk: HHOOK,
            n_code: i32,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> LRESULT;
        
        pub fn GetMessageW(
            lp_msg: *mut MSG,
            h_wnd: HWND,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
        ) -> BOOL;
        
        pub fn TranslateMessage(lp_msg: *const MSG) -> BOOL;
        pub fn DispatchMessageW(lp_msg: *const MSG) -> LRESULT;
        
        pub fn SendInput(
            c_inputs: u32,
            p_inputs: *const INPUT,
            cb_size: i32,
        ) -> u32;

        pub fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
        pub fn CreateWindowExW(
            dwExStyle: u32,
            lpClassName: *const u16,
            lpWindowName: *const u16,
            dwStyle: u32,
            x: i32,
            y: i32,
            nWidth: i32,
            nHeight: i32,
            hWndParent: HWND,
            hMenu: HMENU,
            hInstance: HINSTANCE,
            lpParam: *mut c_void,
        ) -> HWND;
        pub fn DefWindowProcW(
            hWnd: HWND,
            Msg: u32,
            wParam: WPARAM,
            lParam: LPARAM,
        ) -> LRESULT;
        pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: LONG_PTR) -> LONG_PTR;
        
        pub fn CreatePopupMenu() -> HMENU;
        pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
        pub fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> BOOL;
        pub fn TrackPopupMenu(
            hMenu: HMENU,
            uFlags: u32,
            x: i32,
            y: i32,
            nReserved: i32,
            hWnd: HWND,
            prcRect: *const c_void,
        ) -> BOOL;
        pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
        pub fn GetSystemMetrics(nIndex: i32) -> i32;
        pub fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        pub fn PostMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> BOOL;
        
        pub fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> BOOL;
        pub fn GetWindowTextLengthW(hWnd: HWND) -> i32;
        
        pub fn SetFocus(hWnd: HWND) -> HWND;
        pub fn GetSysColorBrush(nIndex: i32) -> HBRUSH;
        
        pub fn CreateFontW(
            cHeight: i32,
            cWidth: i32,
            cEscapement: i32,
            cOrientation: i32,
            cWeight: i32,
            bItalic: u32,
            bUnderline: u32,
            bStrikeOut: u32,
            iCharSet: u32,
            iOutPrecision: u32,
            iClipPrecision: u32,
            iQuality: u32,
            iPitchAndFamily: u32,
            pszFaceName: *const u16,
        ) -> HFONT;
        pub fn PostQuitMessage(nExitCode: i32);
        pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
        pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
        pub fn GetKeyState(nVirtKey: i32) -> i16;
        pub fn IsDialogMessageW(hDlg: HWND, lpMsg: *const MSG) -> BOOL;
        pub fn SetWindowPos(
            hWnd: HWND,
            hWndInsertAfter: HWND,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> BOOL;
        pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
        pub fn MoveWindow(hWnd: HWND, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: BOOL) -> BOOL;
        pub fn GetGUIThreadInfo(idThread: u32, pgui: *mut GUITHREADINFO) -> BOOL;
        pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
        pub fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateSolidBrush(color: u32) -> HBRUSH;
        pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
        pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
        pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        pub fn Shell_NotifyIconW(dwMessage: u32, lpData: *const NOTIFYICONDATAW) -> BOOL;
    }

    pub const IACE_DEFAULT: u32 = 0x0010;

    #[link(name = "imm32")]
    unsafe extern "system" {
        pub fn ImmAssociateContext(hwnd: HWND, himc: HIMC) -> HIMC;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CreateMutexW(lpMutexAttributes: *mut c_void, bInitialOwner: BOOL, lpName: *const u16) -> *mut c_void;
        pub fn GetLastError() -> u32;
        pub fn CloseHandle(hObject: *mut c_void) -> BOOL;
        pub fn GetCurrentThreadId() -> u32;
        pub fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: BOOL) -> BOOL;
    }

    pub const ERROR_ALREADY_EXISTS: u32 = 183;
}

#[cfg(not(target_os = "windows"))]
mod win32 {
    pub type HWND = usize;
    pub unsafe fn GetForegroundWindow() -> HWND { 0 }
    pub unsafe fn SetForegroundWindow(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn IsWindow(_hwnd: HWND) -> i32 { 0 }
    
    pub const VK_CONTROL: u16 = 0;
    pub const VK_V: u16 = 0;
    pub const KEYEVENTF_KEYUP: u32 = 0;
    pub const INPUT_KEYBOARD: u32 = 0;
    
    #[derive(Clone, Copy)]
    pub struct KEYBDINPUT {
        pub w_vk: u16,
        pub w_scan: u16,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }
    
    #[derive(Clone, Copy)]
    pub union INPUT_union {
        pub ki: KEYBDINPUT,
    }
    
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub r#type: u32,
        pub u: INPUT_union,
    }
    
    pub unsafe fn SendInput(_c_inputs: u32, _p_inputs: *const INPUT, _cb_size: i32) -> u32 { 0 }
    pub unsafe fn IsDialogMessageW(_hDlg: HWND, _lpMsg: *const std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn GetSysColorBrush(_n_index: i32) -> HBRUSH { std::ptr::null_mut() }
    pub const COLOR_3DFACE: u32 = 15;
    pub const COLOR_WINDOW: u32 = 5;
    pub const ERROR_ALREADY_EXISTS: u32 = 183;
    pub unsafe fn CreateMutexW(_a: *mut std::ffi::c_void, _b: i32, _c: *const u16) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn GetLastError() -> u32 { 0 }
    pub unsafe fn CloseHandle(_h: *mut std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn GetCurrentThreadId() -> u32 { 0 }
    pub unsafe fn AttachThreadInput(_a: u32, _b: u32, _c: i32) -> i32 { 0 }
}

fn load_migemo_dict() -> Option<CompactDictionary> {
    // 1. exeと同じディレクトリ（配布用）
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for candidate in &[
                exe_dir.join("migemo-compact-dict"),
                exe_dir.join("assets").join("migemo-compact-dict"),
            ] {
                if let Ok(bytes) = fs::read(candidate) {
                    log_debug(&format!("Loaded migemo dict (exe dir): {:?}", candidate));
                    return Some(CompactDictionary::new(&bytes));
                }
            }
        }
    }

    // 2. %APPDATA%\clipper\dict\（ユーザー追加用）
    let appdata_path = PathBuf::from(
        std::env::var("APPDATA").unwrap_or_default()
    ).join("clipper").join("dict").join("migemo-compact-dict");

    if let Ok(bytes) = fs::read(&appdata_path) {
        log_debug(&format!("Loaded migemo dict (appdata): {:?}", appdata_path));
        return Some(CompactDictionary::new(&bytes));
    }

    log_debug("migemo-compact-dict not found in exe dir or appdata");
    None
}

// カスタムメッセージ
const WM_TRIGGER_SNIPPET: u32 = 0x8000 + 2;
const WM_TRIGGER_HISTORY: u32 = 0x8000 + 3;
const WM_CLIPBOARD_CHANGED: u32 = 0x8000 + 4;



#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Snippet,
    History,
}

struct AppState {
    history: VecDeque<String>,
    snippets: Vec<(String, String)>,
    mode: Mode,
    visible: bool,
    current_results: Vec<String>,
    last_clipboard_value: String,
    last_active_window: Option<usize>,
}

#[derive(Clone, Copy, PartialEq)]
struct SafeHWND(win32::HWND);
unsafe impl Send for SafeHWND {}
unsafe impl Sync for SafeHWND {}

type EditWndProc = unsafe extern "system" fn(win32::HWND, u32, win32::WPARAM, win32::LPARAM) -> win32::LRESULT;
struct SafeWndProc(EditWndProc);
unsafe impl Send for SafeWndProc {}
unsafe impl Sync for SafeWndProc {}

struct SafeHBRUSH(win32::HBRUSH);
unsafe impl Send for SafeHBRUSH {}
unsafe impl Sync for SafeHBRUSH {}

struct SafeHFONT(win32::HFONT);
unsafe impl Send for SafeHFONT {}
unsafe impl Sync for SafeHFONT {}

// グローバル状態 (アトミックおよび OnceLock 構成で Mutex 競合によるフック強制解除を根絶)
static LAST_KEY_VK: AtomicU32 = AtomicU32::new(0);
static LAST_KEY_TIME: AtomicU32 = AtomicU32::new(0);
static APP_STATE: Lazy<Mutex<Option<AppState>>> = Lazy::new(|| Mutex::new(None));

static MAIN_HWND: OnceLock<SafeHWND> = OnceLock::new();
static EDIT_HWND: OnceLock<SafeHWND> = OnceLock::new();
static LISTBOX_HWND: OnceLock<SafeHWND> = OnceLock::new();
static OLD_EDIT_PROC: OnceLock<SafeWndProc> = OnceLock::new();
static MIGEMO_DICT: OnceLock<CompactDictionary> = OnceLock::new();

static BRUSH_BG: OnceLock<SafeHBRUSH> = OnceLock::new();
static BRUSH_CTRL: OnceLock<SafeHBRUSH> = OnceLock::new();
static FONT_EDIT: OnceLock<SafeHFONT> = OnceLock::new();
static FONT_LISTBOX: OnceLock<SafeHFONT> = OnceLock::new();

static LOG_QUEUE: Lazy<Mutex<VecDeque<String>>> = Lazy::new(|| Mutex::new(VecDeque::new()));

fn log_debug(_msg: &str) {
    // Disabled in production release.
    /*
    let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let formatted = format!("[{}] {}", now, msg);
    if let Ok(mut queue) = LOG_QUEUE.lock() {
        queue.push_back(formatted);
    }
    */
}

fn start_logging_thread() {
    // Disabled in production release.
    /*
    thread::spawn(|| {
        let app_dir = get_app_dir();
        let log_file = app_dir.join("debug.log");
        loop {
            thread::sleep(Duration::from_millis(100));
            if let Ok(mut queue) = LOG_QUEUE.lock() {
                if !queue.is_empty() {
                    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
                        use std::io::Write;
                        while let Some(msg) = queue.pop_front() {
                            let _ = writeln!(file, "{}", msg);
                        }
                    }
                }
            }
        }
    });
    */
}

fn get_app_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("clipper");
    path
}

fn load_snippets() -> Vec<(String, String)> {
    let app_dir = get_app_dir();
    let snippets_dir = app_dir.join("snippets");
    if !snippets_dir.exists() {
        let _ = fs::create_dir_all(&snippets_dir);
        let _ = fs::write(snippets_dir.join("datetime.j2"), "現在日時: {{ datetime }}");
        let _ = fs::write(snippets_dir.join("link.j2"), "[{{ clipboard }}]({{ clipboard }})");
        let _ = fs::write(snippets_dir.join("code_block.j2"), "```\n{{ clipboard }}\n```");
    }

    let mut snippets = Vec::new();
    if let Ok(entries) = fs::read_dir(snippets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "j2" || ext == "txt") {
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                if let Ok(content) = fs::read_to_string(&path) {
                    snippets.push((name, content));
                }
            }
        }
    }
    snippets
}

fn load_history() -> VecDeque<String> {
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

fn save_history(history: &VecDeque<String>) {
    let app_dir = get_app_dir();
    let history_file = app_dir.join("history.json");
    if let Ok(content) = serde_json::to_string_pretty(history) {
        let _ = fs::create_dir_all(&app_dir);
        let _ = fs::write(history_file, content);
    }
}

fn render_template(template_str: &str, clipboard_text: &str) -> String {
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

fn filter_items(query_text: &str, state: &AppState, dict_opt: Option<&CompactDictionary>) -> Vec<String> {
    if query_text.is_empty() {
        return match state.mode {
            Mode::Snippet => state.snippets.iter().map(|(name, _)| name.clone()).collect(),
            Mode::History => state.history.iter().cloned().collect(),
        };
    }

    let romaji_proc = RomajiProcessor::new();
    let hiragana = romaji_proc.romaji_to_hiragana(query_text);

    let regex_str = if let Some(dict) = dict_opt {
        query(query_text.to_string(), dict, &RegexOperator::Default)
    } else {
        String::new()
    };

    let re_opt = Regex::new(&regex_str).ok();

    let katakana: String = hiragana.chars().map(|c| {
        if ('ぁ'..='ん').contains(&c) {
            char::from_u32(c as u32 + 0x60).unwrap_or(c)
        } else {
            c
        }
    }).collect();

    let matches_text = |text: &str| -> bool {
        if let Some(ref re) = re_opt {
            if re.is_match(text) {
                return true;
            }
        }
        // migemo辞書に無い語のフォールバック: ひらがな/カタカナで直接検索
        if !hiragana.is_empty() && hiragana != query_text {
            if text.contains(&hiragana) || text.contains(&katakana) {
                return true;
            }
        }
        false
    };

    match state.mode {
        Mode::Snippet => {
            state.snippets.iter()
                .filter(|(name, content)| matches_text(name) || matches_text(content))
                .map(|(name, _)| name.clone())
                .collect()
        }
        Mode::History => {
            state.history.iter()
                .filter(|text| matches_text(text))
                .cloned()
                .collect()
        }
    }
}

fn to_wstring(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn create_ui_font(name: &str, size: i32) -> win32::HFONT {
    let name_w = to_wstring(name);
    unsafe {
        win32::CreateFontW(
            size, 0, 0, 0,
            400, // FW_NORMAL
            0, 0, 0,
            1, // DEFAULT_CHARSET
            0, 0,
            5, // CLEARTYPE_QUALITY
            0,
            name_w.as_ptr(),
        )
    }
}

fn move_listbox_selection(dir: i32) {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        let count = unsafe { win32::SendMessageW(*hwnd_listbox, 0x018B /* LB_GETCOUNT */, 0, 0) } as isize;
        if count > 0 {
            let dir_isize = dir as isize;
            let next = if cur == win32::LB_ERR {
                if dir_isize > 0 { 0 } else { count - 1 }
            } else {
                (cur + dir_isize + count) % count
            };
            unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, next as usize, 0) };
        }
    }
}

fn on_select() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
        log_debug(&format!("on_select: cur={}", cur));
        if cur != win32::LB_ERR {
            let len = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETTEXTLEN, cur as usize, 0) } as usize;
            let mut buf = vec![0u16; len + 1];
            unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETTEXT, cur as usize, buf.as_mut_ptr() as win32::LPARAM) };
            
            let selected_text = String::from_utf16_lossy(&buf[..len]);
            log_debug(&format!("on_select selected text: {}", selected_text));
            
            let mut final_text = selected_text.clone();
            let mut last_active = None;
            {
                let mut state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &mut *state_guard {
                    if state.mode == Mode::Snippet {
                        if let Some((_, template)) = state.snippets.iter().find(|(name, _)| name == &selected_text) {
                            final_text = render_template(template, &state.last_clipboard_value);
                        }
                    }
                    last_active = state.last_active_window;
                }
            }

            log_debug(&format!("on_select final text: {}", final_text));

            let mut success = false;
            for _ in 0..10 {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.set_text(final_text.clone()).is_ok() {
                        success = true;
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
            log_debug(&format!("Clipboard write success: {}", success));

            restore_focus(last_active);
            hide_window();
            simulate_paste();
        }
    }
}

fn trigger_app(mode: Mode, active_hwnd: win32::HWND) {
    log_debug(&format!("trigger_app called. Mode={:?}, active_hwnd={:?}", mode, active_hwnd));
    
    // 1. Scope state updates and release the lock immediately
    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.mode = mode;
            if mode == Mode::Snippet {
                state.snippets = load_snippets();
            }
            state.visible = true;

            if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                if !active_hwnd.is_null() && active_hwnd != *hwnd_main {
                    state.last_active_window = Some(active_hwnd as usize);
                } else {
                    let cur_active = unsafe { win32::GetForegroundWindow() };
                    if cur_active != *hwnd_main {
                        state.last_active_window = Some(cur_active as usize);
                    }
                }
            }
        }
    }

    // 2. Perform Win32 actions with lock released
    if let (Some(SafeHWND(hwnd_main)), Some(SafeHWND(hwnd_edit))) = (MAIN_HWND.get(), EDIT_HWND.get()) {
        let monitor_w = unsafe { win32::GetSystemMetrics(0) }; // SM_CXSCREEN
        let monitor_h = unsafe { win32::GetSystemMetrics(1) }; // SM_CYSCREEN
        let w = 400;
        let h = 300;

        // キャレット位置を取得してウィンドウを配置 (fallback: 画面中央)
        let (mut x, mut y) = ((monitor_w - w) / 2, (monitor_h - h) / 2);
        if !active_hwnd.is_null() {
            let tid = unsafe { win32::GetWindowThreadProcessId(active_hwnd, std::ptr::null_mut()) };
            let mut gui: win32::GUITHREADINFO = unsafe { std::mem::zeroed() };
            gui.cbSize = std::mem::size_of::<win32::GUITHREADINFO>() as u32;
            if unsafe { win32::GetGUIThreadInfo(tid, &mut gui) } != 0 && !gui.hwndCaret.is_null() {
                let mut pt = win32::POINT { x: gui.rcCaret.left, y: gui.rcCaret.bottom };
                unsafe { win32::ClientToScreen(gui.hwndCaret, &mut pt) };
                x = pt.x;
                y = pt.y + 4;
            }
        }

        // 画面外にはみ出さないようにクランプ
        if x + w > monitor_w { x = monitor_w - w; }
        if y + h > monitor_h { y = monitor_h - h; }
        if x < 0 { x = 0; }
        if y < 0 { y = 0; }
        
        unsafe {
            win32::SetWindowPos(*hwnd_main, std::ptr::null_mut(), x, y, w, h, 0x0040 /* SWP_SHOWWINDOW */);

            // AttachThreadInput で確実にフォアグラウンド化
            let foreground = win32::GetForegroundWindow();
            if !foreground.is_null() && foreground != *hwnd_main {
                let cur_thread = win32::GetCurrentThreadId();
                let fg_thread = win32::GetWindowThreadProcessId(foreground, std::ptr::null_mut());
                win32::AttachThreadInput(cur_thread, fg_thread, 1);
                win32::SetForegroundWindow(*hwnd_main);
                win32::AttachThreadInput(cur_thread, fg_thread, 0);
            } else {
                win32::SetForegroundWindow(*hwnd_main);
            }

            win32::SetWindowTextW(*hwnd_edit, to_wstring("").as_ptr());
            win32::SetFocus(*hwnd_edit);
            win32::ImmAssociateContext(*hwnd_edit, std::ptr::null_mut());
        }
        update_listbox_items(MIGEMO_DICT.get());
        }

    }

fn hide_window() {
    {
        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.visible = false;
        }
    }
    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
        unsafe { win32::ShowWindow(*hwnd_main, 0 /* SW_HIDE */) };
    }
}

fn restore_focus(last_active_window: Option<usize>) {
    if let Some(hwnd_usize) = last_active_window {
        let hwnd = hwnd_usize as win32::HWND;
        if unsafe { win32::IsWindow(hwnd) } != 0 {
            unsafe { win32::SetForegroundWindow(hwnd) };
        }
    }
}

fn simulate_paste() {
    thread::sleep(Duration::from_millis(150));
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
                }
            }
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V,
                    w_scan: 0,
                    dw_flags: 0,
                    time: 0,
                    dw_extra_info: 0,
                }
            }
        },
        win32::INPUT {
            r#type: win32::INPUT_KEYBOARD,
            u: win32::INPUT_union {
                ki: win32::KEYBDINPUT {
                    w_vk: win32::VK_V,
                    w_scan: 0,
                    dw_flags: win32::KEYEVENTF_KEYUP,
                    time: 0,
                    dw_extra_info: 0,
                }
            }
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
                }
            }
        }
    ];
    unsafe { win32::SendInput(4, inputs.as_ptr(), std::mem::size_of::<win32::INPUT>() as i32) };
}

fn update_listbox_items(dict_opt: Option<&CompactDictionary>) {
    if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
        let len = unsafe { win32::GetWindowTextLengthW(*hwnd_edit) } as usize;
        let mut buf = vec![0u16; len + 1];
        unsafe { win32::GetWindowTextW(*hwnd_edit, buf.as_mut_ptr(), (len + 1) as i32) };
        let query_text = String::from_utf16_lossy(&buf[..len]);

        let mut state_guard = APP_STATE.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            let filtered = filter_items(&query_text, state, dict_opt);
            state.current_results = filtered.clone();
            std::mem::drop(state_guard);

            unsafe {
                win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                for item in &filtered {
                    let item_w = to_wstring(item);
                    win32::SendMessageW(*hwnd_listbox, win32::LB_ADDSTRING, 0, item_w.as_ptr() as win32::LPARAM);
                }
                if !filtered.is_empty() {
                    win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, 0, 0);
                }
            }
        }
    }
}

fn show_tray_menu(hwnd: win32::HWND) {
    let mut pt = win32::POINT { x: 0, y: 0 };
    unsafe { win32::GetCursorPos(&mut pt) };

    let menu = unsafe { win32::CreatePopupMenu() };
    unsafe {
        win32::AppendMenuW(menu, 0, 1001, to_wstring("Show Snippets (Shift x2)").as_ptr());
        win32::AppendMenuW(menu, 0, 1002, to_wstring("Show History (Ctrl x2)").as_ptr());
        win32::AppendMenuW(menu, 0x0800, 0, std::ptr::null());
        win32::AppendMenuW(menu, 0, 1003, to_wstring("Exit").as_ptr());

        win32::SetForegroundWindow(hwnd);
    };

    let cmd = unsafe {
        win32::TrackPopupMenu(
            menu,
            win32::TPM_RETURNCMD | win32::TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        )
    };
    unsafe {
        win32::DestroyMenu(menu);
    };

    if cmd == 1001 {
        trigger_app(Mode::Snippet, std::ptr::null_mut());
    } else if cmd == 1002 {
        trigger_app(Mode::History, std::ptr::null_mut());
    } else if cmd == 1003 {
        unsafe { win32::PostQuitMessage(0) };
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn edit_subclass_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    if msg == win32::WM_KEYDOWN {
        log_debug(&format!("Edit KeyDown: vk={}", wparam));
        match wparam {
            38 => { // UP
                move_listbox_selection(-1);
                return 0;
            }
            40 => { // DOWN
                move_listbox_selection(1);
                return 0;
            }
            13 => { // ENTER
                on_select();
                return 0;
            }
            27 => { // ESC
                hide_window();
                return 0;
            }
            _ => {}
        }

        let ctrl_pressed = (unsafe { win32::GetKeyState(0x11 /* VK_CONTROL */) } & 0x8000u16 as i16) != 0;
        if ctrl_pressed {
            match wparam {
                0x4E | 0x4A => { // N, J -> Down
                    move_listbox_selection(1);
                    return 0;
                }
                0x50 | 0x4B => { // P, K -> Up
                    move_listbox_selection(-1);
                    return 0;
                }
                _ => {}
            }
        }
    }

    let old_proc_opt = OLD_EDIT_PROC.get();
    if let Some(SafeWndProc(old_proc)) = old_proc_opt {
        unsafe { old_proc(hwnd, msg, wparam, lparam) }
    } else {
        unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
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
                    log_debug(&format!("Double press detected: vk={}", mapped_vk));
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
unsafe extern "system" fn window_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    match msg {
        win32::WM_CREATE => {
            log_debug("WM_CREATE message received.");
            let hinstance = unsafe { win32::GetModuleHandleW(std::ptr::null()) };
            
            let hwnd_edit = unsafe {
                win32::CreateWindowExW(
                    win32::WS_EX_CLIENTEDGE,
                    to_wstring("EDIT").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::ES_AUTOHSCROLL | win32::ES_LEFT | 0x0004,
                    0, 0, 0, 0,
                    hwnd,
                    101 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            log_debug(&format!("Edit control created: {:?}", hwnd_edit));

            // IME を無効化してローマ字入力を Migemo に直接渡す
            unsafe { win32::ImmAssociateContext(hwnd_edit, std::ptr::null_mut()) };

            let hwnd_listbox = unsafe {
                win32::CreateWindowExW(
                    win32::WS_EX_CLIENTEDGE,
                    to_wstring("LISTBOX").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::WS_VSCROLL | win32::LBS_NOTIFY | win32::LBS_HASSTRINGS | win32::LBS_NOINTEGRALHEIGHT,
                    0, 0, 0, 0,
                    hwnd,
                    102 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            log_debug(&format!("ListBox control created: {:?}", hwnd_listbox));

            let font_edit = create_ui_font("Segoe UI", -18);
            let font_listbox = create_ui_font("Segoe UI", -16);

            // GetClientRect でクライアント領域に合わせてコントロールを配置
            let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut rc) };
            let cw = rc.right - rc.left;
            let ch = rc.bottom - rc.top;
            let margin = 4;
            let edit_h = 28;
            let gap = 4;

            unsafe {
                win32::MoveWindow(hwnd_edit, margin, margin, cw - margin * 2, edit_h, 1);
                win32::MoveWindow(hwnd_listbox, margin, margin + edit_h + gap, cw - margin * 2, ch - margin * 2 - edit_h - gap, 1);

                win32::SendMessageW(hwnd_edit, win32::WM_SETFONT, font_edit as win32::WPARAM, 1);
                win32::SendMessageW(hwnd_listbox, win32::WM_SETFONT, font_listbox as win32::WPARAM, 1);

                let old_proc = win32::SetWindowLongPtrW(hwnd_edit, win32::GWLP_WNDPROC, edit_subclass_proc as *const () as win32::LONG_PTR);
                let _ = OLD_EDIT_PROC.set(SafeWndProc(std::mem::transmute(old_proc)));
                log_debug(&format!("Edit subclass applied. Old proc: {:?}", old_proc));

                let brush_bg = win32::GetSysColorBrush(win32::COLOR_3DFACE as i32);
                let brush_ctrl = win32::GetSysColorBrush(win32::COLOR_WINDOW as i32);

                let _ = EDIT_HWND.set(SafeHWND(hwnd_edit));
                let _ = LISTBOX_HWND.set(SafeHWND(hwnd_listbox));
                let _ = BRUSH_BG.set(SafeHBRUSH(brush_bg));
                let _ = BRUSH_CTRL.set(SafeHBRUSH(brush_ctrl));
                let _ = FONT_EDIT.set(SafeHFONT(font_edit));
                let _ = FONT_LISTBOX.set(SafeHFONT(font_listbox));

                log_debug("CONTROLS successfully stored inside OnceLocks.");
            }
        }
        // Removed custom WM_CTLCOLOR overrides to inherit native Windows theme colors.
        win32::WM_COMMAND => {
            let ctrl_id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if ctrl_id == 101 && code == win32::EN_CHANGE as usize {
                update_listbox_items(MIGEMO_DICT.get());
            } else if ctrl_id == 102 {
                if code == 2 { // LBN_DBLCLK
                    on_select();
                }
            }
        }
        win32::WM_ACTIVATE => {
            log_debug(&format!("WM_ACTIVATE: wparam={}", wparam));
            if wparam == win32::WA_INACTIVE {
                let is_visible = {
                    let state_guard = APP_STATE.lock().unwrap();
                    state_guard.as_ref().map_or(false, |s| s.visible)
                };
                if is_visible {
                    log_debug("Window inactive, hiding window...");
                    hide_window();
                }
            } else {
                if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                    unsafe { win32::SetFocus(*hwnd_edit) };
                    log_debug("SetFocus called on Edit control.");
                }
            }
        }
        win32::WM_TRAYICON => {
            if lparam == win32::WM_RBUTTONUP as win32::LPARAM {
                show_tray_menu(hwnd);
            } else if lparam == win32::WM_LBUTTONDBLCLK as win32::LPARAM {
                trigger_app(Mode::Snippet, std::ptr::null_mut());
            }
        }
        WM_TRIGGER_SNIPPET => {
            let active_hwnd = wparam as win32::HWND;
            trigger_app(Mode::Snippet, active_hwnd);
        }
        WM_TRIGGER_HISTORY => {
            let active_hwnd = wparam as win32::HWND;
            trigger_app(Mode::History, active_hwnd);
        }
        WM_CLIPBOARD_CHANGED => {
            let mut clipboard = Clipboard::new().unwrap();
            if let Ok(text) = clipboard.get_text() {
                let mut state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &mut *state_guard {
                    state.last_clipboard_value = text.clone();
                    if let Some(pos) = state.history.iter().position(|x| x == &text) {
                        state.history.remove(pos);
                    }
                    state.history.push_front(text);
                    if state.history.len() > 100 {
                        state.history.pop_back();
                    }
                    save_history(&state.history);
                }
            }
        }
        win32::WM_DESTROY => {
            if let Some(SafeHFONT(font)) = FONT_EDIT.get() {
                unsafe { win32::DeleteObject(*font) };
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX.get() {
                unsafe { win32::DeleteObject(*font) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_BG.get() {
                unsafe { win32::DeleteObject(*brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_CTRL.get() {
                unsafe { win32::DeleteObject(*brush) };
            }
            unsafe { win32::PostQuitMessage(0) };
        }
        _ => return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 二重起動防止: 名前付きミューテックスで既存インスタンスの有無をチェック
    {
        let name = to_wstring("Global\\ClipperAppMutex");
        unsafe { win32::CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if unsafe { win32::GetLastError() } == win32::ERROR_ALREADY_EXISTS {
            return Ok(());
        }
    }

    start_logging_thread();
    if let Some(dict) = load_migemo_dict() {
        let _ = MIGEMO_DICT.set(dict);
    }

    let mut app_state = AppState {
        history: load_history(),
        snippets: load_snippets(),
        mode: Mode::Snippet,
        visible: false,
        current_results: Vec::new(),
        last_clipboard_value: String::new(),
        last_active_window: None,
    };

    if let Ok(mut cb) = Clipboard::new() {
        if let Ok(text) = cb.get_text() {
            app_state.last_clipboard_value = text;
        }
    }

    *APP_STATE.lock().unwrap() = Some(app_state);

    unsafe {
        let hinstance = win32::GetModuleHandleW(std::ptr::null());
        let class_name = to_wstring("ClipperWindowClass");
        
        let wnd_class = win32::WNDCLASSW {
            style: win32::CS_HREDRAW | win32::CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: win32::LoadCursorW(std::ptr::null_mut(), win32::IDC_ARROW),
            hbrBackground: (win32::COLOR_3DFACE + 1) as win32::HBRUSH, // システムデフォルト背景色 (COLOR_3DFACE)
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        win32::RegisterClassW(&wnd_class);

        let hwnd = win32::CreateWindowExW(
            win32::WS_EX_TOPMOST | win32::WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            to_wstring("Clipper").as_ptr(),
            win32::WS_POPUP | win32::WS_BORDER | win32::WS_DLGFRAME,
            0, 0, 400, 300,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create Win32 Window".into());
        }

        let _ = MAIN_HWND.set(SafeHWND(hwnd));

        let mut nid: win32::NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = win32::NIF_MESSAGE | win32::NIF_ICON | win32::NIF_TIP;
        nid.uCallbackMessage = win32::WM_TRAYICON;
        nid.hIcon = win32::LoadCursorW(std::ptr::null_mut(), win32::IDC_ARROW);

        let tip_w = to_wstring("Clipper - Snippet & Clipboard Manager");
        let tip_len = std::cmp::min(tip_w.len(), 127);
        nid.szTip[..tip_len].copy_from_slice(&tip_w[..tip_len]);

        win32::Shell_NotifyIconW(win32::NIM_ADD, &nid);

        let hinstance_hook = win32::GetModuleHandleW(std::ptr::null());
        let hook = win32::SetWindowsHookExW(
            win32::WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            hinstance_hook,
            0,
        );
        if hook.is_null() {
            log_debug("SetWindowsHookExW failed to register on main thread!");
        } else {
            log_debug("SetWindowsHookExW registered successfully on main thread.");
        }

        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(_) => return,
            };
            let mut last_text = String::new();
            loop {
                if let Ok(text) = clipboard.get_text() {
                    if !text.is_empty() && text != last_text {
                        last_text = text.clone();
                        if let Some(SafeHWND(main_hwnd_val)) = MAIN_HWND.get() {
                            win32::PostMessageW(*main_hwnd_val, WM_CLIPBOARD_CHANGED, 0, 0);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(500));
            }
        });

        let mut msg = std::mem::zeroed();
        while win32::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let is_visible = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.visible)
            };

            if is_visible && msg.message == win32::WM_KEYDOWN {
                if msg.wparam == 13 {
                    log_debug("Enter key intercepted in message loop.");
                    on_select();
                    continue;
                } else if msg.wparam == 27 {
                    log_debug("Esc key intercepted in message loop.");
                    hide_window();
                    continue;
                }
            }

            if win32::IsDialogMessageW(hwnd, &msg) == 0 {
                win32::TranslateMessage(&msg);
                win32::DispatchMessageW(&msg);
            }
        }

        if !hook.is_null() {
            win32::UnhookWindowsHookEx(hook);
        }
        win32::Shell_NotifyIconW(win32::NIM_DELETE, &nid);
    }

    Ok(())
}
