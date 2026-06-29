use arboard::Clipboard;

use crate::darkmode;
use crate::state::{self, SafeHWND, SafeWndProc, SafeHBRUSH, SafeHFONT, MIGEMO_DICT, APP_STATE, BRUSH_BG, BRUSH_CTRL, BRUSH_EDIT, BRUSH_LISTBOX, EDIT_HWND, FONT_EDIT, FONT_LISTBOX, FONT_LISTBOX_BOLD, LISTBOX_HWND, OLD_EDIT_PROC, WM_CLIPBOARD_CHANGED, WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET};
use crate::state::Mode;
use crate::ui;
use crate::util;
use crate::win32;

// Win32 Edit margin constants
const EM_SETMARGINS: u32 = 0x00D3;
const EC_LEFTMARGIN: usize = 1;
const EC_RIGHTMARGIN: usize = 2;

// Additional Win32 constants for scrolling
const WM_VSCROLL: u32 = 0x0115;
const WM_MOUSEWHEEL: u32 = 0x020A;
const LBN_SELCHANGE: u16 = 1;

// IME Candidate Window theme colors (COLORREF = 0x00BBGGRR)
struct ThemeColors {
    window_bg: u32,
    edit_bg: u32,
    text_color: u32,
    sel_bg: u32,
    sel_text: u32,
    num_color: u32,
    border_color: u32,
    sep_color: u32,
}

const LIGHT_THEME: ThemeColors = ThemeColors {
    window_bg: 0x00F5F5F7,    // macOS/Win11 light gray (RGB: 245, 245, 247)
    edit_bg: 0x00FFFFFF,      // Pure white for edit search box
    text_color: 0x001C1C1E,   // Soft dark gray (RGB: 28, 28, 30)
    sel_bg: 0x00E67A00,       // Accent Blue (RGB: 0, 122, 230)
    sel_text: 0x00FFFFFF,     // White text for selection
    num_color: 0x009F9F9F,    // Soft gray for candidate numbers
    border_color: 0x00CCCCCC, // Clean gray border (RGB: 204, 204, 204)
    sep_color: 0x00E5E5EA,    // Elegant thin separator (RGB: 229, 229, 234)
};

const DARK_THEME: ThemeColors = ThemeColors {
    window_bg: 0x001C1C1E,    // macOS/Win11 dark gray (RGB: 28, 28, 30)
    edit_bg: 0x002C2C2E,      // Darker gray search box (RGB: 44, 44, 46)
    text_color: 0x00F2F2F7,   // Off-white text (RGB: 242, 242, 247)
    sel_bg: 0x00FF9F0A,       // Vibrant Blue (RGB: 10, 159, 255)
    sel_text: 0x00FFFFFF,     // White text for selection
    num_color: 0x008E8E93,    // Soft medium gray (RGB: 142, 142, 147)
    border_color: 0x00444446, // Dark clean border (RGB: 68, 68, 70)
    sep_color: 0x002C2C2E,    // Elegant dark separator
};

const IME_ITEM_HEIGHT: u32 = 28;  // Height for listbox candidates (generous padding)
const IME_NUM_COL_WIDTH: i32 = 28; // Candidate number column width

// Update theme colors, brushes, fonts and apply to controls
pub fn update_theme_resources(hwnd: win32::HWND, is_dark: bool) {
    let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

    unsafe {
        // Release old brushes
        if let Some(SafeHBRUSH(brush)) = BRUSH_BG.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_CTRL.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_EDIT.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }

        // Create new brushes
        let brush_bg = win32::CreateSolidBrush(colors.window_bg);
        let brush_ctrl = win32::CreateSolidBrush(colors.edit_bg);
        let brush_edit = win32::CreateSolidBrush(colors.edit_bg);
        let brush_listbox = win32::CreateSolidBrush(colors.window_bg);

        *BRUSH_BG.lock().unwrap() = Some(SafeHBRUSH(brush_bg));
        *BRUSH_CTRL.lock().unwrap() = Some(SafeHBRUSH(brush_ctrl));
        *BRUSH_EDIT.lock().unwrap() = Some(SafeHBRUSH(brush_edit));
        *BRUSH_LISTBOX.lock().unwrap() = Some(SafeHBRUSH(brush_listbox));

        // Initialize fonts if not done
        if FONT_EDIT.lock().unwrap().is_none() {
            let config = state::CONFIG.get().cloned().unwrap_or_default();
            let font_edit = util::create_ui_font(&config.font_name, -16, 400);
            let font_listbox = util::create_ui_font(&config.font_name, -14, 400);
            let font_listbox_bold = util::create_ui_font(&config.font_name, -14, 700);
            *FONT_EDIT.lock().unwrap() = Some(SafeHFONT(font_edit));
            *FONT_LISTBOX.lock().unwrap() = Some(SafeHFONT(font_listbox));
            *FONT_LISTBOX_BOLD.lock().unwrap() = Some(SafeHFONT(font_listbox_bold));
        }

        // Apply to main window and child controls
        darkmode::apply_to_window(hwnd, is_dark);

        if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
            darkmode::apply_to_control(*hwnd_edit, is_dark);
            darkmode::apply_to_control(*hwnd_listbox, is_dark);

            if let Some(SafeHFONT(font)) = FONT_EDIT.lock().unwrap().as_ref() {
                win32::SendMessageW(*hwnd_edit, win32::WM_SETFONT, *font as win32::WPARAM, 1);
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX.lock().unwrap().as_ref() {
                win32::SendMessageW(*hwnd_listbox, win32::WM_SETFONT, *font as win32::WPARAM, 1);
            }

            // Add padding to edit box (6px margin left and right)
            win32::SendMessageW(*hwnd_edit, EM_SETMARGINS, EC_LEFTMARGIN | EC_RIGHTMARGIN, (6 | (6 << 16)) as win32::LPARAM);

            // Set text placeholder (Cue Banner)
            let cue_text = util::to_wstring("検索 (Migemo)...");
            win32::SendMessageW(*hwnd_edit, win32::EM_SETCUEBANNER, 1, cue_text.as_ptr() as win32::LPARAM);

            win32::InvalidateRect(*hwnd_edit, std::ptr::null(), 1);
            win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
        }

        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

pub fn update_top_index() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        unsafe {
            let top_index = win32::SendMessageW(*hwnd_listbox, win32::LB_GETTOPINDEX, 0, 0) as usize;
            let mut state_guard = APP_STATE.lock().unwrap();
            if let Some(state) = &mut *state_guard {
                state.top_index = top_index;
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn edit_subclass_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    if msg == win32::WM_KEYDOWN {
        state::log_debug(&format!("Edit KeyDown: vk={}", wparam));
        match wparam {
            38 => { ui::move_listbox_selection(-1); return 0; }
            40 => { ui::move_listbox_selection(1); return 0; }
            13 => { ui::on_select(); return 0; }
            27 => { ui::hide_window(); return 0; }
            8 => { // Backspace
                let len = unsafe { win32::GetWindowTextLengthW(hwnd) };
                if len == 0 {
                    let has_parent = {
                        let state_guard = APP_STATE.lock().unwrap();
                        state_guard.as_ref().map_or(false, |s| !s.current_folder.is_empty())
                    };
                    if has_parent {
                        let mut state_guard = APP_STATE.lock().unwrap();
                        if let Some(state) = &mut *state_guard {
                            if let Some(pos) = state.current_folder.rfind('/') {
                                state.current_folder = state.current_folder[..pos].to_string();
                            } else {
                                state.current_folder.clear();
                            }
                        }
                        std::mem::drop(state_guard);
                        ui::update_listbox_items(MIGEMO_DICT.get());
                        return 0;
                    }
                }
            }
            _ => {}
        }

        let ctrl_pressed = (unsafe { win32::GetKeyState(0x11) } & 0x8000u16 as i16) != 0;
        if ctrl_pressed {
            match wparam {
                0x4E | 0x4A => { ui::move_listbox_selection(1); return 0; }
                0x50 | 0x4B => { ui::move_listbox_selection(-1); return 0; }
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
pub unsafe extern "system" fn window_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    match msg {
        win32::WM_CREATE => {
            state::log_debug("WM_CREATE message received.");
            let hinstance = unsafe { win32::GetModuleHandleW(std::ptr::null()) };

            // Search box without client edge (0 style)
            let hwnd_edit = unsafe {
                win32::CreateWindowExW(
                    0,
                    util::to_wstring("EDIT").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::ES_AUTOHSCROLL | win32::ES_LEFT | 0x0004,
                    0, 0, 0, 0,
                    hwnd,
                    101 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            state::log_debug(&format!("Edit control created: {:?}", hwnd_edit));

            unsafe { win32::ImmAssociateContext(hwnd_edit, std::ptr::null_mut()) };

            // ListBox with Owner Draw Fixed, no border
            let hwnd_listbox = unsafe {
                win32::CreateWindowExW(
                    0,
                    util::to_wstring("LISTBOX").as_ptr(),
                    std::ptr::null(),
                    win32::WS_CHILD | win32::WS_VISIBLE | win32::WS_VSCROLL
                        | win32::LBS_NOTIFY | win32::LBS_HASSTRINGS
                        | win32::LBS_NOINTEGRALHEIGHT | win32::LBS_OWNERDRAWFIXED,
                    0, 0, 0, 0,
                    hwnd,
                    102 as win32::HMENU,
                    hinstance,
                    std::ptr::null_mut(),
                )
            };
            state::log_debug(&format!("ListBox control created: {:?}", hwnd_listbox));

            let _ = EDIT_HWND.set(SafeHWND(hwnd_edit));
            let _ = LISTBOX_HWND.set(SafeHWND(hwnd_listbox));

            // Set up subclassing for keyboard control
            let old_proc = win32::SetWindowLongPtrW(hwnd_edit, win32::GWLP_WNDPROC, edit_subclass_proc as *const () as win32::LONG_PTR);
            let _ = OLD_EDIT_PROC.set(SafeWndProc(std::mem::transmute(old_proc)));
            state::log_debug(&format!("Edit subclass applied. Old proc: {:?}", old_proc));

            // Apply theme resources right after controls are initialized
            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            update_theme_resources(hwnd, is_dark);
        }
        win32::WM_SIZE => {
            if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
                let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
                unsafe { win32::GetClientRect(hwnd, &mut rc) };
                let cw = rc.right - rc.left;
                let ch = rc.bottom - rc.top;
                
                // Add inner margins for modern aesthetics
                let margin = 5;
                let edit_h = 24;
                let gap = 4;
                unsafe {
                    win32::MoveWindow(*hwnd_edit, margin, margin, cw - margin * 2, edit_h, 1);
                    win32::MoveWindow(*hwnd_listbox, margin, margin + edit_h + gap, cw - margin * 2, ch - margin * 2 - edit_h - gap, 1);
                }
            }
        }
        win32::WM_COMMAND => {
            let ctrl_id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if ctrl_id == 101 && code == win32::EN_CHANGE as usize {
                ui::update_listbox_items(MIGEMO_DICT.get());
            } else if ctrl_id == 102 {
                if code == 2 { // LBN_DBLCLK
                    ui::on_select();
                } else if code == LBN_SELCHANGE as usize {
                    update_top_index();
                }
            }
        }
        WM_VSCROLL | WM_MOUSEWHEEL => {
            update_top_index();
            return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        state::WM_FILTER_COMPLETE => {
            let generation = wparam as u32;
            let mut display_items = Vec::new();
            
            {
                let state_guard = APP_STATE.lock().unwrap();
                if let Some(state) = &*state_guard {
                    if state.filter_generation == generation {
                        display_items = state.current_results.clone();
                    }
                }
            }

            if !display_items.is_empty() || generation != 0 {
                if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                    unsafe {
                        win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                        for item in &display_items {
                            let item_w = util::to_wstring(item);
                            win32::SendMessageW(*hwnd_listbox, win32::LB_ADDSTRING, 0, item_w.as_ptr() as win32::LPARAM);
                        }
                        if !display_items.is_empty() {
                            win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, 0, 0);
                        }
                    }
                    update_top_index();
                }
            }
        }
        win32::WM_CTLCOLOREDIT => {
            let hdc = wparam as win32::HDC;
            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            unsafe {
                win32::SetTextColor(hdc, colors.text_color);
                win32::SetBkColor(hdc, colors.edit_bg);
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_EDIT.lock().unwrap().as_ref() {
                return *brush as win32::LRESULT;
            }
        }
        win32::WM_CTLCOLORLISTBOX => {
            let hdc = wparam as win32::HDC;
            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            unsafe {
                win32::SetTextColor(hdc, colors.text_color);
                win32::SetBkColor(hdc, colors.window_bg);
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX.lock().unwrap().as_ref() {
                return *brush as win32::LRESULT;
            }
        }
        win32::WM_MEASUREITEM => {
            let mis = lparam as *mut win32::MEASUREITEMSTRUCT;
            if !mis.is_null() {
                unsafe { (*mis).item_height = IME_ITEM_HEIGHT; }
            }
            return 1;
        }
        win32::WM_DRAWITEM => {
            let dis = lparam as *const win32::DRAWITEMSTRUCT;
            if dis.is_null() { return 0; }
            let dis = unsafe { *dis };

            if dis.item_id == u32::MAX { return 1; }

            let hdc = dis.hdc;
            let rc = dis.rc_item;
            let selected = (dis.item_state & win32::ODS_SELECTED) != 0;

            let (is_dark, mode, is_folder) = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or((false, Mode::Snippet, false), |s| {
                    let is_folder = if s.mode == Mode::Snippet && (dis.item_id as usize) < s.current_full_paths.len() {
                        let path = &s.current_full_paths[dis.item_id as usize];
                        path.starts_with("dir:") || path == ".."
                    } else {
                        false
                    };
                    (s.is_dark, s.mode, is_folder)
                })
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            // Fetch absolute list box item text
            let len = unsafe { win32::SendMessageW(dis.hwnd_item, win32::LB_GETTEXTLEN, dis.item_id as usize, 0) } as usize;
            let mut buf = vec![0u16; len + 1];
            unsafe { win32::SendMessageW(dis.hwnd_item, win32::LB_GETTEXT, dis.item_id as usize, buf.as_mut_ptr() as win32::LPARAM) };

            // Setup colors based on selection status
            let bg_color = if selected { colors.sel_bg } else { colors.window_bg };
            let text_color = if selected { colors.sel_text } else { colors.text_color };
            let num_color = if selected { colors.sel_text } else { colors.num_color };

            // Draw item background
            let bg_brush = unsafe { win32::CreateSolidBrush(bg_color) };
            if selected {
                // Pill-shaped rounded floating background: add 4px horizontal and 2px vertical margins
                let pill_rc = win32::RECT {
                    left: rc.left + 4,
                    top: rc.top + 2,
                    right: rc.right - 4,
                    bottom: rc.bottom - 2,
                };
                unsafe {
                    let old_brush = win32::SelectObject(hdc, bg_brush);
                    let old_pen = win32::SelectObject(hdc, win32::GetStockObject(8 /* NULL_PEN */));
                    
                    // Draw rounded rectangle with ellipse diameter of 6px
                    win32::RoundRect(hdc, pill_rc.left, pill_rc.top, pill_rc.right, pill_rc.bottom, 6, 6);
                    
                    win32::SelectObject(hdc, old_brush);
                    win32::SelectObject(hdc, old_pen);
                }
            } else {
                unsafe { win32::FillRect(hdc, &rc, bg_brush) };
            }
            unsafe { win32::DeleteObject(bg_brush) };

            unsafe { win32::SetBkMode(hdc, 1 /* TRANSPARENT */) };

            // Apply normal/bold font based on selection
            let font_to_use = if selected {
                FONT_LISTBOX_BOLD.lock().unwrap()
            } else {
                FONT_LISTBOX.lock().unwrap()
            };
            if let Some(SafeHFONT(font)) = font_to_use.as_ref() {
                unsafe { win32::SelectObject(hdc, *font as win32::HGDIOBJ) };
            }

            // Draw candidate text (adjust margin based on pill shape padding)
            let text_left_margin = 10;
            let text_right_margin = if is_folder {
                24
            } else {
                10
            };
            let mut text_rc = win32::RECT {
                left: rc.left + text_left_margin,
                top: rc.top,
                right: rc.right - text_right_margin,
                bottom: rc.bottom,
            };
            unsafe {
                win32::SetTextColor(hdc, text_color);
                win32::DrawTextW(
                    hdc, buf.as_ptr(), len as i32, &mut text_rc,
                    win32::DT_SINGLELINE | win32::DT_VCENTER | win32::DT_LEFT | win32::DT_END_ELLIPSIS | win32::DT_NOPREFIX,
                );
            }

            // Draw folder indicator `>` on the right edge
            if is_folder {
                let arrow_w = util::to_wstring(">");
                let mut arrow_rc = win32::RECT {
                    left: rc.right - 20,
                    top: rc.top,
                    right: rc.right - 6,
                    bottom: rc.bottom,
                };
                unsafe {
                    win32::SetTextColor(hdc, text_color);
                    win32::DrawTextW(
                        hdc, arrow_w.as_ptr(), -1, &mut arrow_rc,
                        win32::DT_SINGLELINE | win32::DT_VCENTER | win32::DT_RIGHT | win32::DT_NOPREFIX,
                    );
                }
            }

            return 1;
        }
        win32::WM_PAINT => {
            // Paint default window contents, then custom paint border
            let res = unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) };

            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            let hdc = unsafe { win32::GetDC(hwnd) };
            let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut rc) };

            // Draw 1px clean border
            let border_brush = unsafe { win32::CreateSolidBrush(colors.border_color) };
            unsafe { win32::FrameRect(hdc, &rc, border_brush) };
            unsafe { win32::DeleteObject(border_brush) };

            // Fill window background outside controls to ensure clean look
            let bg_brush = unsafe { win32::CreateSolidBrush(colors.window_bg) };
            // Draw a thin border fill or just draw inside client margins
            unsafe { win32::DeleteObject(bg_brush) };

            unsafe { win32::ReleaseDC(hwnd, hdc) };
            return res;
        }
        win32::WM_ERASEBKGND => {
            // Erase with the correct theme color
            let hdc = wparam as win32::HDC;
            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut rc) };

            let bg_brush = unsafe { win32::CreateSolidBrush(colors.window_bg) };
            unsafe { win32::FillRect(hdc, &rc, bg_brush) };
            unsafe { win32::DeleteObject(bg_brush) };
            return 1;
        }
        win32::WM_ACTIVATE => {
            let active_state = wparam & 0xFFFF;
            state::log_debug(&format!("WM_ACTIVATE: active_state={}", active_state));
            if active_state == win32::WA_INACTIVE {
                let is_visible = {
                    let state_guard = APP_STATE.lock().unwrap();
                    state_guard.as_ref().map_or(false, |s| s.visible)
                };
                if is_visible {
                    state::log_debug("Window inactive, hiding window...");
                    ui::hide_window();
                }
            } else {
                if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                    unsafe { win32::SetFocus(*hwnd_edit) };
                    state::log_debug("SetFocus called on Edit control.");
                }
            }
        }
        0x001C => { // WM_ACTIVATEAPP
            state::log_debug(&format!("WM_ACTIVATEAPP: wparam={}", wparam));
            if wparam == 0 { // Being deactivated
                let is_visible = {
                    let state_guard = APP_STATE.lock().unwrap();
                    state_guard.as_ref().map_or(false, |s| s.visible)
                };
                if is_visible {
                    state::log_debug("App inactive, hiding window...");
                    ui::hide_window();
                }
            }
        }
        win32::WM_TRAYICON => {
            if lparam == win32::WM_RBUTTONUP as win32::LPARAM {
                ui::show_tray_menu(hwnd);
            } else if lparam == win32::WM_LBUTTONDBLCLK as win32::LPARAM {
                ui::trigger_app(Mode::Snippet, std::ptr::null_mut());
            }
        }
        WM_TRIGGER_SNIPPET => {
            let active_hwnd = wparam as win32::HWND;
            ui::trigger_app(Mode::Snippet, active_hwnd);
        }
        WM_TRIGGER_HISTORY => {
            let active_hwnd = wparam as win32::HWND;
            ui::trigger_app(Mode::History, active_hwnd);
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
                    util::save_history(&state.history);
                }
            }
        }
        win32::WM_DESTROY => {
            // Clean up font objects
            if let Some(SafeHFONT(font)) = FONT_EDIT.lock().unwrap().take() {
                unsafe { win32::DeleteObject(font) };
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX.lock().unwrap().take() {
                unsafe { win32::DeleteObject(font) };
            }

            // Clean up brush objects
            if let Some(SafeHBRUSH(brush)) = BRUSH_BG.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_CTRL.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_EDIT.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            unsafe { win32::PostQuitMessage(0) };
        }
        _ => return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}
