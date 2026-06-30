#[cfg(target_os = "windows")]
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod windows {
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
    pub struct PAINTSTRUCT {
        pub hdc: HDC,
        pub fErase: BOOL,
        pub rcPaint: RECT,
        pub fRestore: BOOL,
        pub fIncUpdate: BOOL,
        pub rgbReserved: [u8; 32],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DATA_BLOB {
        pub cbData: u32,
        pub pbData: *mut u8,
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
    pub const WM_CLIPBOARDUPDATE: u32 = 0x031D;
    pub const CF_UNICODETEXT: u32 = 13;
    pub const GMEM_MOVEABLE: u32 = 0x0002;

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
    pub const LBS_OWNERDRAWFIXED: u32 = 0x0010;

    pub const WS_EX_TOPMOST: u32 = 0x00000008;
    pub const WS_EX_TOOLWINDOW: u32 = 0x00000080;
    pub const WS_EX_CLIENTEDGE: u32 = 0x00000200;

    pub const COLOR_3DFACE: u32 = 15;
    pub const COLOR_WINDOW: u32 = 5;

    pub const GWLP_WNDPROC: i32 = -4;
    pub const EM_SETCUEBANNER: u32 = 0x1501;
    pub const EN_CHANGE: u16 = 0x0300;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_CREATE: u32 = 0x0001;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_SETFONT: u32 = 0x0030;
    pub const WM_ACTIVATE: u32 = 0x0006;
    pub const WM_PAINT: u32 = 0x000F;
    pub const WM_ERASEBKGND: u32 = 0x0014;
    pub const WA_INACTIVE: usize = 0;

    pub const LB_ADDSTRING: u32 = 0x0180;
    pub const LB_RESETCONTENT: u32 = 0x0184;
    pub const LB_GETCURSEL: u32 = 0x0188;
    pub const LB_SETCURSEL: u32 = 0x0186;
    pub const LB_GETTEXT: u32 = 0x0189;
    pub const LB_GETTEXTLEN: u32 = 0x018A;
    pub const LB_GETTOPINDEX: u32 = 0x018E;
    pub const LB_ERR: isize = -1;

    pub type HKEY = *mut c_void;
    pub const HKEY_CURRENT_USER: HKEY = 0x80000001 as HKEY;
    pub const KEY_READ: u32 = 0x20019;

    pub const WM_CTLCOLOREDIT: u32 = 0x0133;
    pub const WM_CTLCOLORLISTBOX: u32 = 0x0134;
    pub const WM_DRAWITEM: u32 = 0x002B;
    pub const WM_MEASUREITEM: u32 = 0x002C;
    pub const WM_SIZE: u32 = 0x0005;

    pub const ODA_DRAWENTIRE: u32 = 0x0001;
    pub const ODA_SELECT: u32 = 0x0002;
    pub const ODA_FOCUS: u32 = 0x0004;
    pub const ODS_SELECTED: u32 = 0x0001;

    pub const DT_LEFT: u32 = 0x00000000;
    pub const DT_CENTER: u32 = 0x00000001;
    pub const DT_RIGHT: u32 = 0x00000002;
    pub const DT_VCENTER: u32 = 0x00000004;
    pub const DT_SINGLELINE: u32 = 0x00000020;
    pub const DT_END_ELLIPSIS: u32 = 0x00008000;
    pub const DT_NOPREFIX: u32 = 0x00000800;

    pub const PS_SOLID: i32 = 0;
    pub const NULL_PEN: i32 = 8;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DRAWITEMSTRUCT {
        pub ctl_type: u32,
        pub ctl_id: u32,
        pub item_id: u32,
        pub item_action: u32,
        pub item_state: u32,
        pub hwnd_item: HWND,
        pub hdc: HDC,
        pub rc_item: RECT,
        pub item_data: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MEASUREITEMSTRUCT {
        pub ctl_type: u32,
        pub ctl_id: u32,
        pub item_id: u32,
        pub item_width: u32,
        pub item_height: u32,
        pub item_data: usize,
    }

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
        pub fn GetFocus() -> HWND;
        pub fn IsWindow(hwnd: HWND) -> BOOL;
        pub fn SetWindowsHookExW(id_hook: i32, lpfn: HOOKPROC, hmod: HINSTANCE, dw_thread_id: u32) -> HHOOK;
        pub fn UnhookWindowsHookEx(hhk: HHOOK) -> BOOL;
        pub fn CallNextHookEx(hhk: HHOOK, n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
        pub fn GetMessageW(lp_msg: *mut MSG, h_wnd: HWND, w_msg_filter_min: u32, w_msg_filter_max: u32) -> BOOL;
        pub fn TranslateMessage(lp_msg: *const MSG) -> BOOL;
        pub fn DispatchMessageW(lp_msg: *const MSG) -> LRESULT;
        pub fn SendInput(c_inputs: u32, p_inputs: *const INPUT, cb_size: i32) -> u32;
        pub fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
        pub fn CreateWindowExW(dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16, dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32, hWndParent: HWND, hMenu: HMENU, hInstance: HINSTANCE, lpParam: *mut c_void) -> HWND;
        pub fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: LONG_PTR) -> LONG_PTR;
        pub fn CreatePopupMenu() -> HMENU;
        pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
        pub fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> BOOL;
        pub fn TrackPopupMenu(hMenu: HMENU, uFlags: u32, x: i32, y: i32, nReserved: i32, hWnd: HWND, prcRect: *const c_void) -> BOOL;
        pub fn GetCursorPos(lpPoint: *mut POINT);
        pub fn GetSystemMetrics(nIndex: i32) -> i32;
        pub fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        pub fn PostMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> BOOL;
        pub fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> BOOL;
        pub fn GetWindowTextLengthW(hWnd: HWND) -> i32;
        pub fn SetFocus(hWnd: HWND) -> HWND;
        pub fn GetSysColorBrush(nIndex: i32) -> HBRUSH;
        pub fn CreateFontW(cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32, cWeight: i32, bItalic: u32, bUnderline: u32, bStrikeOut: u32, iCharSet: u32, iOutPrecision: u32, iClipPrecision: u32, iQuality: u32, iPitchAndFamily: u32, pszFaceName: *const u16) -> HFONT;
        pub fn PostQuitMessage(nExitCode: i32);
        pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
        pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
        pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: *const u16) -> *mut c_void;
        pub fn GetKeyState(nVirtKey: i32) -> i16;
        pub fn IsDialogMessageW(hDlg: HWND, lpMsg: *const MSG) -> BOOL;
        pub fn SetWindowPos(hWnd: HWND, hWndInsertAfter: HWND, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> BOOL;
        pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
        pub fn MoveWindow(hWnd: HWND, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: BOOL) -> BOOL;
        pub fn GetGUIThreadInfo(idThread: u32, pgui: *mut GUITHREADINFO) -> BOOL;
        pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
        pub fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
        pub fn DrawTextW(hdc: HDC, lpchText: *const u16, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
        pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
        pub fn FrameRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
        pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
        pub fn GetDC(hWnd: HWND) -> HDC;
        pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
        pub fn GetDpiForWindow(hwnd: HWND) -> u32;
        pub fn MonitorFromWindow(hwnd: HWND, dwFlags: u32) -> *mut c_void;
        pub fn AddClipboardFormatListener(hwnd: HWND) -> BOOL;
        pub fn RemoveClipboardFormatListener(hwnd: HWND) -> BOOL;
        pub fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
        pub fn CloseClipboard() -> BOOL;
        pub fn GetClipboardData(uFormat: u32) -> *mut c_void;
        pub fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
        pub fn EmptyClipboard() -> BOOL;
        pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
        pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateSolidBrush(color: u32) -> HBRUSH;
        pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
        pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
        pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
        pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
        pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
        pub fn CreatePen(iStyle: i32, cWidth: i32, color: u32) -> HGDIOBJ;
        pub fn GetStockObject(i: i32) -> HGDIOBJ;
        pub fn Rectangle(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
        pub fn RoundRect(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, width: i32, height: i32) -> BOOL;
        pub fn MoveToEx(hdc: HDC, x: i32, y: i32, lppt: *mut POINT) -> BOOL;
        pub fn LineTo(hdc: HDC, x: i32, y: i32) -> BOOL;
        pub fn AddFontMemResourceEx(pFileView: *const std::ffi::c_void, cjSize: u32, pvReserved: *mut std::ffi::c_void, pNumFonts: *mut u32) -> *mut std::ffi::c_void;
        pub fn RemoveFontMemResourceEx(h: *mut std::ffi::c_void) -> BOOL;
        pub fn Polygon(hdc: HDC, apt: *const POINT, cpt: i32) -> BOOL;
        pub fn Ellipse(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
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
        pub fn GetProcAddress(hModule: HINSTANCE, lpProcName: *const u8) -> *mut c_void;
        pub fn LocalFree(hMem: *mut c_void) -> *mut c_void;
        pub fn GetCurrentProcess() -> *mut c_void;
        pub fn SetProcessWorkingSetSize(
            hProcess: *mut c_void,
            dwMinimumWorkingSetSize: usize,
            dwMaximumWorkingSetSize: usize,
        ) -> BOOL;
        pub fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
        pub fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
        pub fn GlobalUnlock(hMem: *mut c_void) -> BOOL;
        pub fn GlobalFree(hMem: *mut c_void) -> *mut c_void;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        pub fn DwmSetWindowAttribute(hwnd: HWND, dwAttribute: u32, pvAttribute: *const c_void, cbAttribute: u32) -> i32;
    }

    pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    pub const DWMWCP_ROUND: u32 = 2;

    #[link(name = "uxtheme")]
    unsafe extern "system" {
        pub fn SetWindowTheme(hwnd: HWND, pszSubAppName: *const u16, pszSubIdList: *const u16) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn RegOpenKeyExW(hKey: HKEY, lpSubKey: *const u16, ulOptions: u32, samDesired: u32, phkResult: *mut HKEY) -> i32;
        pub fn RegQueryValueExW(hKey: HKEY, lpValueName: *const u16, lpReserved: *mut u32, lpType: *mut u32, lpData: *mut u8, lpcbData: *mut u32) -> i32;
        pub fn RegCloseKey(hKey: HKEY) -> i32;
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        pub fn CryptProtectData(
            pDataIn: *const DATA_BLOB,
            szDataDescr: *const u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut c_void,
            pPromptStruct: *mut c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> BOOL;

        pub fn CryptUnprotectData(
            pDataIn: *const DATA_BLOB,
            ppszDataDescr: *mut *mut u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut c_void,
            pPromptStruct: *mut c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> BOOL;
    }

    pub const ERROR_ALREADY_EXISTS: u32 = 183;
}

#[cfg(not(target_os = "windows"))]
mod windows {
    pub type HWND = usize;
    pub type HBRUSH = *mut std::ffi::c_void;
    pub type BOOL = i32;
    pub type HFONT = *mut std::ffi::c_void;

    pub const VK_CONTROL: u16 = 0;
    pub const VK_V: u16 = 0;
    pub const KEYEVENTF_KEYUP: u32 = 0;
    pub const INPUT_KEYBOARD: u32 = 0;

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
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub r#type: u32,
        pub u: INPUT_union,
    }

    pub const COLOR_3DFACE: u32 = 15;
    pub const COLOR_WINDOW: u32 = 5;
    pub const ERROR_ALREADY_EXISTS: u32 = 183;

    pub type HKEY = usize;
    pub const HKEY_CURRENT_USER: HKEY = 0;
    pub const KEY_READ: u32 = 0;
    pub const LB_GETTOPINDEX: u32 = 0;
    pub const WM_PAINT: u32 = 0;
    pub const WM_ERASEBKGND: u32 = 0;

    pub unsafe fn GetForegroundWindow() -> HWND { 0 }
    pub unsafe fn SetForegroundWindow(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn IsWindow(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn SendInput(_c_inputs: u32, _p_inputs: *const INPUT, _cb_size: i32) -> u32 { 0 }
    pub unsafe fn IsDialogMessageW(_hDlg: HWND, _lpMsg: *const std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn GetSysColorBrush(_n_index: i32) -> HBRUSH { std::ptr::null_mut() }
    pub unsafe fn CreateMutexW(_a: *mut std::ffi::c_void, _b: i32, _c: *const u16) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn GetLastError() -> u32 { 0 }
    pub unsafe fn CloseHandle(_h: *mut std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn GetCurrentThreadId() -> u32 { 0 }
    pub unsafe fn AttachThreadInput(_a: u32, _b: u32, _c: i32) -> i32 { 0 }
    pub unsafe fn GetProcAddress(_m: HWND, _n: *const u8) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn DwmSetWindowAttribute(_h: HWND, _a: u32, _p: *const std::ffi::c_void, _c: u32) -> i32 { 0 }
    pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    pub const DWMWCP_ROUND: u32 = 2;
    pub const CF_UNICODETEXT: u32 = 13;
    pub const GMEM_MOVEABLE: u32 = 0x0002;
    pub const WM_CLIPBOARDUPDATE: u32 = 0x031D;
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct PAINTSTRUCT {
        pub hdc: usize,
        pub fErase: i32,
        pub rcPaint: RECT,
        pub fRestore: i32,
        pub fIncUpdate: i32,
        pub rgbReserved: [u8; 32],
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DATA_BLOB {
        pub cbData: u32,
        pub pbData: *mut std::ffi::c_void,
    }
    pub unsafe fn CryptProtectData(_a: *const DATA_BLOB, _b: *const u16, _c: *const DATA_BLOB, _d: *mut std::ffi::c_void, _e: *mut std::ffi::c_void, _f: u32, _g: *mut DATA_BLOB) -> i32 { 0 }
    pub unsafe fn CryptUnprotectData(_a: *const DATA_BLOB, _b: *mut *mut u16, _c: *const DATA_BLOB, _d: *mut std::ffi::c_void, _e: *mut std::ffi::c_void, _f: u32, _g: *mut DATA_BLOB) -> i32 { 0 }
    pub unsafe fn LocalFree(_h: *mut std::ffi::c_void) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn AddFontMemResourceEx(_a: *const std::ffi::c_void, _b: u32, _c: *mut std::ffi::c_void, _d: *mut u32) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn RemoveFontMemResourceEx(_h: *mut std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn Polygon(_hdc: usize, _apt: *const std::ffi::c_void, _cpt: i32) -> i32 { 0 }
    pub unsafe fn Ellipse(_hdc: usize, _left: i32, _top: i32, _right: i32, _bottom: i32) -> i32 { 0 }
    pub unsafe fn GetFocus() -> HWND { std::ptr::null_mut() }
    pub unsafe fn GetCurrentProcess() -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn SetProcessWorkingSetSize(_h: *mut std::ffi::c_void, _min: usize, _max: usize) -> i32 { 0 }
    pub unsafe fn AddClipboardFormatListener(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn RemoveClipboardFormatListener(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn OpenClipboard(_hwnd: HWND) -> i32 { 0 }
    pub unsafe fn CloseClipboard() -> i32 { 0 }
    pub unsafe fn GetClipboardData(_format: u32) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn SetClipboardData(_format: u32, _hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn EmptyClipboard() -> i32 { 0 }
    pub unsafe fn GlobalAlloc(_flags: u32, _bytes: usize) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn GlobalLock(_h: *mut std::ffi::c_void) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn GlobalUnlock(_h: *mut std::ffi::c_void) -> i32 { 0 }
    pub unsafe fn GlobalFree(_h: *mut std::ffi::c_void) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    pub unsafe fn BeginPaint(_hwnd: HWND, _lpPaint: *mut PAINTSTRUCT) -> usize { 0 }
    pub unsafe fn EndPaint(_hwnd: HWND, _lpPaint: *const PAINTSTRUCT) -> i32 { 0 }
}

pub use windows::*;
