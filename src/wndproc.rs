use std::sync::atomic::{AtomicBool, Ordering};

pub static EDIT_FOCUSED: AtomicBool = AtomicBool::new(false);

use crate::darkmode;
use crate::state::{self, SafeHWND, SafeWndProc, SafeHBRUSH, SafeHFONT, APP_STATE, BRUSH_BG, BRUSH_CTRL, BRUSH_EDIT, BRUSH_LISTBOX, BRUSH_BORDER, BRUSH_SEL_BG, EDIT_HWND, FONT_EDIT, FONT_LISTBOX, FONT_LISTBOX_BOLD, LISTBOX_HWND, OLD_EDIT_PROC, WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET};
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
    border_color: u32,
}

const LIGHT_THEME: ThemeColors = ThemeColors {
    window_bg: 0x00F5F5F7,    // macOS/Win11 light gray (RGB: 245, 245, 247)
    edit_bg: 0x00FFFFFF,      // Pure white for edit search box
    text_color: 0x00000000,   // Pure black text (RGB: 0, 0, 0)
    sel_bg: 0x00E67A00,       // Accent Blue (RGB: 0, 122, 230)
    sel_text: 0x00FFFFFF,     // White text for selection
    border_color: 0x00CCCCCC, // Clean gray border (RGB: 204, 204, 204)
};

const DARK_THEME: ThemeColors = ThemeColors {
    window_bg: 0x001C1C1E,    // macOS/Win11 dark gray (RGB: 28, 28, 30)
    edit_bg: 0x002C2C2E,      // Darker gray search box (RGB: 44, 44, 46)
    text_color: 0x00FFFFFF,   // Pure white text (RGB: 255, 255, 255) - slightly brightened
    sel_bg: 0x00FF9F0A,       // Vibrant Blue (RGB: 10, 159, 255)
    sel_text: 0x00FFFFFF,     // White text for selection
    border_color: 0x00444446, // Dark clean border (RGB: 68, 68, 70)
};

const IME_ITEM_HEIGHT: u32 = 30;  // Height for listbox candidates (generous padding)

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
        if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG.lock().unwrap().take() {
            win32::DeleteObject(brush);
        }

        // Create new brushes
        let brush_bg = win32::CreateSolidBrush(colors.window_bg);
        let brush_ctrl = win32::CreateSolidBrush(colors.edit_bg);
        let brush_edit = win32::CreateSolidBrush(colors.edit_bg);
        let brush_listbox = win32::CreateSolidBrush(colors.window_bg);
        let brush_border = win32::CreateSolidBrush(colors.border_color);
        let brush_sel_bg = win32::CreateSolidBrush(colors.sel_bg);

        *BRUSH_BG.lock().unwrap() = Some(SafeHBRUSH(brush_bg));
        *BRUSH_CTRL.lock().unwrap() = Some(SafeHBRUSH(brush_ctrl));
        *BRUSH_EDIT.lock().unwrap() = Some(SafeHBRUSH(brush_edit));
        *BRUSH_LISTBOX.lock().unwrap() = Some(SafeHBRUSH(brush_listbox));
        *BRUSH_BORDER.lock().unwrap() = Some(SafeHBRUSH(brush_border));
        *BRUSH_SEL_BG.lock().unwrap() = Some(SafeHBRUSH(brush_sel_bg));

        // Initialize fonts if not done
        if FONT_EDIT.lock().unwrap().is_none() {
            let config = state::CONFIG.get().cloned().unwrap_or_default();
            let font_edit = util::create_ui_font(&config.font_name, -16, 400);
            let font_listbox = util::create_ui_font(&config.font_name, -14, 400);
            let font_listbox_bold = util::create_ui_font(&config.font_name, -14, 700);
            *FONT_EDIT.lock().unwrap() = Some(SafeHFONT(font_edit));
            *FONT_LISTBOX.lock().unwrap() = Some(SafeHFONT(font_listbox));
            *FONT_LISTBOX_BOLD.lock().unwrap() = Some(SafeHFONT(font_listbox_bold));

            let font_name = util::to_wstring("Segoe MDL2 Assets");
            let font_icons_16 = win32::CreateFontW(
                -16, 0, 0, 0,
                400,
                0, 0, 0,
                1, // DEFAULT_CHARSET (Required for Segoe MDL2 Assets)
                0, 0,
                5, // CLEARTYPE_QUALITY
                0,
                font_name.as_ptr(),
            );
            let font_icons_18 = win32::CreateFontW(
                -18, 0, 0, 0,
                400,
                0, 0, 0,
                1, // DEFAULT_CHARSET (Required for Segoe MDL2 Assets)
                0, 0,
                5, // CLEARTYPE_QUALITY
                0,
                font_name.as_ptr(),
            );
            let _ = state::FONT_ICONS_16.set(SafeHFONT(font_icons_16));
            let _ = state::FONT_ICONS_18.set(SafeHFONT(font_icons_18));
        }



        // Apply to main window and child controls
        darkmode::apply_to_window(hwnd, is_dark);

        if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) = (EDIT_HWND.get(), LISTBOX_HWND.get()) {
            let empty_str = [0u16];
            win32::SetWindowTheme(*hwnd_edit, empty_str.as_ptr(), empty_str.as_ptr());
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
            win32::SendMessageW(*hwnd_edit, win32::EM_SETCUEBANNER, 1, util::wstr_cue().as_ptr() as win32::LPARAM);

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
            46 => { ui::delete_selected_item(); return 0; }
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
                        ui::update_listbox_items();
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
pub unsafe extern "system" fn listbox_subclass_proc(
    hwnd: win32::HWND,
    msg: u32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    if msg == win32::WM_LBUTTONUP {
        let old_proc_opt = state::OLD_LISTBOX_PROC.get();
        let res = unsafe {
            if let Some(SafeWndProc(old_proc)) = old_proc_opt {
                old_proc(hwnd, msg, wparam, lparam)
            } else {
                win32::DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        };

        // Trigger selection immediately on single mouse click (WM_LBUTTONUP)
        let cur = unsafe { win32::SendMessageW(hwnd, win32::LB_GETCURSEL, 0, 0) } as isize;
        if cur != win32::LB_ERR {
            ui::on_select();
        }
        return res;
    }

    let old_proc_opt = state::OLD_LISTBOX_PROC.get();
    unsafe {
        if let Some(SafeWndProc(old_proc)) = old_proc_opt {
            old_proc(hwnd, msg, wparam, lparam)
        } else {
            win32::DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn window_proc(hwnd: win32::HWND, msg: u32, wparam: win32::WPARAM, lparam: win32::LPARAM) -> win32::LRESULT {
    match msg {
        win32::WM_CREATE => {
            state::log_debug("WM_CREATE message received.");
            
            // Set rounded corners for Windows 11
            let corner_preference = win32::DWMWCP_ROUND;
            unsafe {
                win32::DwmSetWindowAttribute(
                    hwnd,
                    win32::DWMWA_WINDOW_CORNER_PREFERENCE,
                    &corner_preference as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
            }

            let hinstance = unsafe { win32::GetModuleHandleW(std::ptr::null()) };

            // Search box without client edge (0 style)
            let hwnd_edit = unsafe {
                win32::CreateWindowExW(
                    0,
                    util::wstr_edit().as_ptr(),
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
                    util::wstr_listbox().as_ptr(),
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
            let old_proc = unsafe {
                win32::SetWindowLongPtrW(hwnd_edit, win32::GWLP_WNDPROC, edit_subclass_proc as *const () as win32::LONG_PTR)
            };
            let _ = OLD_EDIT_PROC.set(SafeWndProc(unsafe { std::mem::transmute(old_proc) }));
            state::log_debug(&format!("Edit subclass applied. Old proc: {:?}", old_proc));

            let old_listbox_proc = unsafe {
                win32::SetWindowLongPtrW(hwnd_listbox, win32::GWLP_WNDPROC, listbox_subclass_proc as *const () as win32::LONG_PTR)
            };
            let _ = state::OLD_LISTBOX_PROC.set(SafeWndProc(unsafe { std::mem::transmute(old_listbox_proc) }));
            state::log_debug(&format!("ListBox subclass applied. Old proc: {:?}", old_listbox_proc));

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
                
                let margin = 4;
                let edit_container_h = 30;
                let edit_h = 24;
                let gap = 5;
                
                let listbox_y = margin + edit_container_h + gap;
                let listbox_w = cw - margin * 2;
                let listbox_h = ch - listbox_y - margin;
                
                unsafe {
                    win32::MoveWindow(*hwnd_listbox, margin, listbox_y, listbox_w, listbox_h, 1);
                }

                // Get listbox client width to align the edit control exactly to the listbox items area
                let mut list_client_rc: win32::RECT = unsafe { std::mem::zeroed() };
                unsafe { win32::GetClientRect(*hwnd_listbox, &mut list_client_rc) };
                let list_client_w = list_client_rc.right - list_client_rc.left;

                let edit_y = margin + (edit_container_h - edit_h) / 2;
                let edit_x = margin + 26; // 4 (margin) + 26 = 30
                let edit_w = list_client_w - 34; // Align right edge to the listbox client area
                
                unsafe {
                    win32::MoveWindow(*hwnd_edit, edit_x, edit_y, edit_w, edit_h, 1);
                }
            }
        }
        win32::WM_COMMAND => {
            let ctrl_id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if ctrl_id == 101 {
                if code == win32::EN_CHANGE as usize {
                    ui::update_listbox_items();
                } else if code == 0x0100 /* EN_SETFOCUS */ {
                    EDIT_FOCUSED.store(true, Ordering::SeqCst);
                    unsafe { win32::InvalidateRect(hwnd, std::ptr::null(), 1); }
                 } else if code == 0x0200 /* EN_KILLFOCUS */ {
                    EDIT_FOCUSED.store(false, Ordering::SeqCst);
                    unsafe { win32::InvalidateRect(hwnd, std::ptr::null(), 1); }
                    
                    let focus_hwnd = unsafe { win32::GetFocus() };
                    let is_child = if focus_hwnd.is_null() {
                        false
                    } else {
                        focus_hwnd == hwnd ||
                        EDIT_HWND.get().map_or(false, |h| focus_hwnd == h.0) ||
                        LISTBOX_HWND.get().map_or(false, |h| focus_hwnd == h.0)
                    };
                    if !is_child {
                        ui::hide_window();
                    }
                }
            } else if ctrl_id == 102 {
                if code == 2 { // LBN_DBLCLK
                    ui::on_select();
                } else if code == LBN_SELCHANGE as usize {
                    update_top_index();
                    
                    // Navigate folders immediately on a single click
                    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                        let cur = unsafe { win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0) } as isize;
                        if cur != win32::LB_ERR {
                            let mut is_folder = false;
                            {
                                let state_guard = APP_STATE.lock().unwrap();
                                if let Some(state) = &*state_guard {
                                    if state.mode == Mode::Snippet && (cur as usize) < state.current_full_paths.len() {
                                        let target = &state.current_full_paths[cur as usize];
                                        is_folder = target == ".." || target.starts_with("dir:");
                                    }
                                }
                            }
                            if is_folder {
                                ui::on_select();
                            }
                        }
                    }
                } else if code == 5 { // LBN_KILLFOCUS
                    let focus_hwnd = unsafe { win32::GetFocus() };
                    let is_child = if focus_hwnd.is_null() {
                        false
                    } else {
                        focus_hwnd == hwnd ||
                        EDIT_HWND.get().map_or(false, |h| focus_hwnd == h.0) ||
                        LISTBOX_HWND.get().map_or(false, |h| focus_hwnd == h.0)
                    };
                    if !is_child {
                        ui::hide_window();
                    }
                }
            }
        }
        WM_VSCROLL | WM_MOUSEWHEEL => {
            update_top_index();
            return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        state::WM_FILTER_COMPLETE => {
            let generation = wparam as u32;
            let display_items = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().and_then(|state| {
                    if state.filter_generation == generation {
                        Some(state.current_results.clone())
                    } else {
                        None
                    }
                })
            };

            if let Some(items) = display_items {
                if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                    unsafe {
                        win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                        for item in &items {
                            let item_w = util::to_wstring(item);
                            win32::SendMessageW(*hwnd_listbox, win32::LB_ADDSTRING, 0, item_w.as_ptr() as win32::LPARAM);
                        }
                        if !items.is_empty() {
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

            let (is_dark, _mode, is_folder) = {
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

            // Draw item background using cached brushes
            let (bg_brush, delete_brush) = if selected {
                if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG.lock().unwrap().as_ref() {
                    (*brush, false)
                } else {
                    (unsafe { win32::CreateSolidBrush(bg_color) }, true)
                }
            } else {
                if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX.lock().unwrap().as_ref() {
                    (*brush, false)
                } else {
                    (unsafe { win32::CreateSolidBrush(bg_color) }, true)
                }
            };

            if selected {
                // Pill-shaped rounded floating background: add 2px horizontal and 1px vertical gap
                let pill_rc = win32::RECT {
                    left: rc.left + 2,
                    top: rc.top,
                    right: rc.right - 2,
                    bottom: rc.bottom - 1,
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
            if delete_brush {
                unsafe { win32::DeleteObject(bg_brush) };
            }

            unsafe { win32::SetBkMode(hdc, 1 /* TRANSPARENT */) };

            // Parse text and tags directly on UTF-16 slice to prevent heap allocations
            let wide_text = &buf[..len];
            let (icon_type_opt, clean_text_w) = if wide_text.starts_with(&[91, 68, 73, 82, 93, 32]) { // "[DIR] "
                let contains_dots = wide_text.windows(2).any(|w| w == [46, 46]);
                let icon = if contains_dots { IconType::ParentFolder } else { IconType::Folder };
                (Some(icon), &wide_text[6..])
            } else if wide_text.starts_with(&[91, 83, 78, 73, 80, 93, 32]) { // "[SNIP] "
                (Some(IconType::Snippet), &wide_text[7..])
            } else if wide_text.starts_with(&[91, 72, 73, 83, 84, 93, 32]) { // "[HIST] "
                (Some(IconType::History), &wide_text[7..])
            } else {
                (None, wide_text)
            };

            // Draw icon if present
            let has_icon = icon_type_opt.is_some();
            if let Some(icon_type) = icon_type_opt {
                let icon_x = rc.left + 10;
                let icon_y = rc.top + (rc.bottom - rc.top - 16) / 2;
                draw_vector_icon(hdc, icon_type, icon_x, icon_y, 16, text_color);
            }

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
            let text_left_margin = if has_icon { 32 } else { 10 };
            let text_right_margin = if is_folder { 24 } else { 10 };
            let mut text_rc = win32::RECT {
                left: rc.left + text_left_margin,
                top: rc.top,
                right: rc.right - text_right_margin,
                bottom: rc.bottom,
            };
            
            unsafe {
                win32::SetTextColor(hdc, text_color);
                win32::DrawTextW(
                    hdc, clean_text_w.as_ptr(), clean_text_w.len() as i32, &mut text_rc,
                    win32::DT_SINGLELINE | win32::DT_VCENTER | win32::DT_LEFT | win32::DT_END_ELLIPSIS | win32::DT_NOPREFIX,
                );
            }

            // Draw folder indicator (chevron-right) on the right edge using native vector drawing
            if is_folder {
                let arrow_x = rc.right - 18;
                let arrow_y = rc.top + (rc.bottom - rc.top - 14) / 2;
                draw_vector_icon(hdc, IconType::ChevronRight, arrow_x, arrow_y, 14, text_color);
            }

            return 1;
        }
        win32::WM_PAINT => {
            let mut ps = unsafe { std::mem::zeroed::<win32::PAINTSTRUCT>() };
            let hdc = unsafe { win32::BeginPaint(hwnd, &mut ps) };

            let is_dark = {
                let state_guard = APP_STATE.lock().unwrap();
                state_guard.as_ref().map_or(false, |s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            let mut client_rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut client_rc) };
            let cw = client_rc.right - client_rc.left;

            // Draw a rounded input container border/background
            let margin = 4;
            let edit_container_h = 30;
            
            // Align to listbox client width
            let mut list_client_w = cw - margin * 2;
            if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                let mut list_client_rc: win32::RECT = unsafe { std::mem::zeroed() };
                unsafe {
                    win32::GetClientRect(*hwnd_listbox, &mut list_client_rc);
                }
                list_client_w = list_client_rc.right - list_client_rc.left;
            }
            
            let container_rc = win32::RECT {
                left: margin + 2,
                top: margin,
                right: margin + list_client_w - 2,
                bottom: margin + edit_container_h,
            };
            
            // Choose border color based on focus
            let focus = EDIT_FOCUSED.load(Ordering::SeqCst);
            let border_color = if focus { colors.sel_bg } else { colors.border_color };
            
            // Draw container background and border using RoundRect
            unsafe {
                let border_pen = win32::CreatePen(win32::PS_SOLID, 1, border_color);
                let old_pen = win32::SelectObject(hdc, border_pen);
                
                let bg_brush = win32::CreateSolidBrush(colors.edit_bg);
                let old_brush = win32::SelectObject(hdc, bg_brush);
                
                // Draw rounded rect with 6px corner radius
                win32::RoundRect(hdc, container_rc.left, container_rc.top, container_rc.right, container_rc.bottom, 6, 6);
                
                win32::SelectObject(hdc, old_brush);
                win32::DeleteObject(bg_brush);
                
                win32::SelectObject(hdc, old_pen);
                win32::DeleteObject(border_pen);
            }
            
            // Draw the 🔍 search icon inside the container using native vector drawing
            let icon_color = if is_dark { 0x00888888 } else { 0x00888888 }; // subtle gray
            draw_vector_icon(hdc, IconType::Search, margin + 6, margin + (edit_container_h - 18) / 2, 18, icon_color);

            // Draw 1px clean border of the main window itself
            let mut delete_border = false;
            let border_brush = if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER.lock().unwrap().as_ref() {
                *brush
            } else {
                delete_border = true;
                unsafe { win32::CreateSolidBrush(colors.border_color) }
            };
            unsafe { win32::FrameRect(hdc, &client_rc, border_brush) };
            if delete_border {
                unsafe { win32::DeleteObject(border_brush) };
            }

            unsafe { win32::EndPaint(hwnd, &ps) };
            return 0;
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

            let mut delete_bg = false;
            let bg_brush = if let Some(SafeHBRUSH(brush)) = BRUSH_BG.lock().unwrap().as_ref() {
                *brush
            } else {
                delete_bg = true;
                unsafe { win32::CreateSolidBrush(colors.window_bg) }
            };
            unsafe { win32::FillRect(hdc, &rc, bg_brush) };
            if delete_bg {
                unsafe { win32::DeleteObject(bg_brush) };
            }
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
        win32::WM_CLIPBOARDUPDATE => {
            if let Some(text) = util::get_clipboard_text() {
                if !text.is_empty() {
                    let mut state_guard = APP_STATE.lock().unwrap();
                    if let Some(state) = &mut *state_guard {
                        if text != state.last_clipboard_value {
                            state.last_clipboard_value = text.clone();
                            let history = std::sync::Arc::make_mut(&mut state.history);
                            if let Some(pos) = history.iter().position(|x| x == &text) {
                                history.remove(pos);
                            }
                            history.push_front(text);
                            if history.len() > 1000 {
                                history.pop_back();
                            }
                            let history_arc = std::sync::Arc::clone(&state.history);
                            std::thread::spawn(move || {
                                util::save_history(&history_arc);
                            });
                        }
                    }
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
            if let Some(SafeHFONT(font)) = state::FONT_ICONS_16.get() {
                unsafe { win32::DeleteObject(*font) };
            }
            if let Some(SafeHFONT(font)) = state::FONT_ICONS_18.get() {
                unsafe { win32::DeleteObject(*font) };
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
            if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG.lock().unwrap().take() {
                unsafe { win32::DeleteObject(brush) };
            }
            unsafe { win32::PostQuitMessage(0) };
        }
        _ => return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}

#[derive(Clone, Copy, PartialEq)]
pub enum IconType {
    Search,
    Folder,
    ParentFolder,
    Snippet,
    History,
    ChevronRight,
}

pub fn draw_vector_icon(hdc: win32::HDC, icon_type: IconType, x: i32, y: i32, size: i32, color: u32) {
    let mut drawn_with_font = false;

    unsafe {
        let font_icons = if size == 18 {
            state::FONT_ICONS_18.get()
        } else {
            state::FONT_ICONS_16.get()
        };

        if let Some(SafeHFONT(font)) = font_icons {
            if !font.is_null() {
                let old_font = win32::SelectObject(hdc, *font as win32::HGDIOBJ);
                if !old_font.is_null() {
                    let old_color = win32::SetTextColor(hdc, color);
                    let old_mode = win32::SetBkMode(hdc, 1 /* TRANSPARENT */);

                    let glyph_char = match icon_type {
                        IconType::Search => 0xE721u16,         // Search (🔍)
                        IconType::Folder => 0xE8B7u16,         // Folder (📁)
                        IconType::ParentFolder => 0xE10Eu16,   // FolderParent / Up (↩/⬆)
                        IconType::Snippet => 0xE70Au16,        // Document (📄)
                        IconType::History => 0xE12Fu16,        // Clipboard (📋)
                        IconType::ChevronRight => 0xE974u16,   // ChevronRight (＞)
                    };

                    let glyph_w = [glyph_char, 0];
                    let mut rc = win32::RECT {
                        left: x,
                        top: y + 1,
                        right: x + size,
                        bottom: y + size + 1,
                    };

                    win32::DrawTextW(
                        hdc,
                        glyph_w.as_ptr(),
                        1,
                        &mut rc,
                        win32::DT_SINGLELINE | win32::DT_CENTER | win32::DT_VCENTER | win32::DT_NOPREFIX,
                    );

                    win32::SelectObject(hdc, old_font);
                    win32::SetTextColor(hdc, old_color);
                    win32::SetBkMode(hdc, old_mode);
                    drawn_with_font = true;
                }
            }
        }
    }

    if !drawn_with_font {
        let s = |v: f32| (v * size as f32 / 24.0).round() as i32;

        unsafe {
            let pen = win32::CreatePen(win32::PS_SOLID, 2, color);
            let old_pen = win32::SelectObject(hdc, pen);
            let null_brush = win32::GetStockObject(5 /* NULL_BRUSH */);
            let old_brush = win32::SelectObject(hdc, null_brush);

            match icon_type {
                IconType::Search => {
                    win32::Ellipse(hdc, x + s(6.0), y + s(4.0), x + s(16.0), y + s(14.0));
                    win32::MoveToEx(hdc, x + s(13.0), y + s(11.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(19.0), y + s(17.0));
                }
                IconType::Folder => {
                    win32::MoveToEx(hdc, x + s(4.0), y + s(9.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(4.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(9.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(11.0), y + s(9.0));
                    win32::RoundRect(hdc, x + s(4.0), y + s(9.0), x + s(20.0), y + s(19.0), s(2.0), s(2.0));
                }
                IconType::ParentFolder => {
                    win32::MoveToEx(hdc, x + s(4.0), y + s(9.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(4.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(9.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(11.0), y + s(9.0));
                    win32::RoundRect(hdc, x + s(4.0), y + s(9.0), x + s(20.0), y + s(19.0), s(2.0), s(2.0));
                    win32::MoveToEx(hdc, x + s(12.0), y + s(11.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(12.0), y + s(17.0));
                    win32::MoveToEx(hdc, x + s(12.0), y + s(11.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(9.0), y + s(14.0));
                    win32::MoveToEx(hdc, x + s(12.0), y + s(11.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(15.0), y + s(14.0));
                }
                IconType::Snippet => {
                    win32::MoveToEx(hdc, x + s(6.0), y + s(2.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(14.0), y + s(2.0));
                    win32::LineTo(hdc, x + s(20.0), y + s(8.0));
                    win32::LineTo(hdc, x + s(20.0), y + s(22.0));
                    win32::LineTo(hdc, x + s(6.0), y + s(22.0));
                    win32::LineTo(hdc, x + s(6.0), y + s(2.0));
                    win32::MoveToEx(hdc, x + s(14.0), y + s(2.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(14.0), y + s(8.0));
                    win32::LineTo(hdc, x + s(20.0), y + s(8.0));
                    win32::MoveToEx(hdc, x + s(9.0), y + s(12.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(17.0), y + s(12.0));
                    win32::MoveToEx(hdc, x + s(9.0), y + s(16.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(17.0), y + s(16.0));
                }
                IconType::History => {
                    win32::RoundRect(hdc, x + s(5.0), y + s(5.0), x + s(19.0), y + s(22.0), s(3.0), s(3.0));
                    win32::RoundRect(hdc, x + s(8.0), y + s(2.0), x + s(16.0), y + s(6.0), s(2.0), s(2.0));
                    win32::MoveToEx(hdc, x + s(8.0), y + s(10.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(16.0), y + s(10.0));
                    win32::MoveToEx(hdc, x + s(8.0), y + s(14.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(16.0), y + s(14.0));
                }
                IconType::ChevronRight => {
                    win32::MoveToEx(hdc, x + s(9.0), y + s(6.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(15.0), y + s(12.0));
                    win32::LineTo(hdc, x + s(9.0), y + s(18.0));
                }
            }

            win32::SelectObject(hdc, old_pen);
            win32::SelectObject(hdc, old_brush);
            win32::DeleteObject(pen);
        }
    }
}

