use std::sync::atomic::{AtomicBool, Ordering};

pub static EDIT_FOCUSED: AtomicBool = AtomicBool::new(false);

use crate::darkmode;
use crate::state::Mode;
use crate::state::{
    self, BRUSH_BG, BRUSH_BORDER, BRUSH_CTRL, BRUSH_EDIT, BRUSH_LISTBOX, BRUSH_SEL_BG, EDIT_HWND,
    FONT_EDIT, FONT_LISTBOX, FONT_LISTBOX_BOLD, FifoLifoMode, LISTBOX_HWND, MAIN_HWND,
    OLD_EDIT_PROC, SafeHBRUSH, SafeHFONT, SafeHWND, SafeWndProc, WM_FIFO_LIFO_PASTE,
    WM_HIDE_WINDOW, WM_TOGGLE_FIFO_LIFO, WM_TRIGGER_HISTORY, WM_TRIGGER_SNIPPET, lock_state,
};
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
    dim_text_color: u32, // 二次テキスト（アイコン等の補助色）
    sel_bg: u32,
    sel_text: u32,
    border_color: u32,
}

const LIGHT_THEME: ThemeColors = ThemeColors {
    window_bg: 0x00F7F5F5,      // Warm light gray (RGB: 245, 245, 247)
    edit_bg: 0x00FFFFFF,        // Pure white for edit search box
    text_color: 0x001F1D1D,     // Apple-style soft black (RGB: 29, 29, 31)
    dim_text_color: 0x008B8686, // Secondary gray (RGB: 134, 134, 139)
    sel_bg: 0x00E3710,          // Apple Blue (RGB: 0, 113, 227)
    sel_text: 0x00FFFFFF,       // White text for selection
    border_color: 0x00D7D2D2,   // Soft border (RGB: 210, 210, 215)
};

const DARK_THEME: ThemeColors = ThemeColors {
    window_bg: 0x001E1C1C,      // macOS/Win11 dark (RGB: 28, 28, 30)
    edit_bg: 0x002E2C2C,        // Elevated dark (RGB: 44, 44, 46)
    text_color: 0x00F7F5F5,     // Soft white (RGB: 247, 245, 245)
    dim_text_color: 0x009D9898, // Secondary muted (RGB: 152, 152, 157)
    sel_bg: 0x00FF840A,         // iOS Blue (RGB: 10, 132, 255)
    sel_text: 0x00FFFFFF,       // White text for selection
    border_color: 0x003A3838,   // Subtle dark border (RGB: 56, 56, 58)
};

const SHORTCUT_CHARS: &[char] = &[
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

// Update theme colors, brushes, fonts and apply to controls
pub fn update_theme_resources(hwnd: win32::HWND, is_dark: bool) {
    let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

    unsafe {
        // Release old brushes
        if let Some(SafeHBRUSH(brush)) = BRUSH_BG.lock().unwrap_or_else(|e| e.into_inner()).take() {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_CTRL.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_EDIT.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            win32::DeleteObject(brush);
        }
        if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            win32::DeleteObject(brush);
        }

        // Create new brushes
        let brush_bg = win32::CreateSolidBrush(colors.window_bg);
        let brush_ctrl = win32::CreateSolidBrush(colors.edit_bg);
        let brush_edit = win32::CreateSolidBrush(colors.edit_bg);
        let brush_listbox = win32::CreateSolidBrush(colors.window_bg);
        let brush_border = win32::CreateSolidBrush(colors.border_color);
        let brush_sel_bg = win32::CreateSolidBrush(colors.sel_bg);

        *BRUSH_BG.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_bg));
        *BRUSH_CTRL.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_ctrl));
        *BRUSH_EDIT.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_edit));
        *BRUSH_LISTBOX.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_listbox));
        *BRUSH_BORDER.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_border));
        *BRUSH_SEL_BG.lock().unwrap_or_else(|e| e.into_inner()) = Some(SafeHBRUSH(brush_sel_bg));

        // Scale sizes by DPI scale factor
        let scale = win32::GetDpiForWindow(hwnd) as f32 / 96.0;
        let config = state::CONFIG.get().cloned().unwrap_or_default();
        let font_edit_size = (-16.0 * scale) as i32;
        let font_listbox_size = (-14.0 * scale) as i32;
        let icon_16_size = (-16.0 * scale) as i32;
        let icon_18_size = (-18.0 * scale) as i32;

        // Recreate FONT_EDIT
        {
            let mut font_guard = FONT_EDIT.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(SafeHFONT(old_font)) = font_guard.take() {
                win32::DeleteObject(old_font);
            }
            let font_edit = util::create_ui_font(&config.font_name, font_edit_size, 400);
            *font_guard = Some(SafeHFONT(font_edit));
        }

        // Recreate FONT_LISTBOX
        {
            let mut font_guard = FONT_LISTBOX.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(SafeHFONT(old_font)) = font_guard.take() {
                win32::DeleteObject(old_font);
            }
            let font_listbox = util::create_ui_font(&config.font_name, font_listbox_size, 400);
            *font_guard = Some(SafeHFONT(font_listbox));
        }

        // Recreate FONT_LISTBOX_BOLD
        {
            let mut font_guard = FONT_LISTBOX_BOLD.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(SafeHFONT(old_font)) = font_guard.take() {
                win32::DeleteObject(old_font);
            }
            let font_listbox_bold = util::create_ui_font(&config.font_name, font_listbox_size, 700);
            *font_guard = Some(SafeHFONT(font_listbox_bold));
        }

        // Recreate FONT_ICONS_16
        {
            let mut font_guard = state::FONT_ICONS_16
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(SafeHFONT(old_font)) = font_guard.take() {
                win32::DeleteObject(old_font);
            }
            let font_name = util::to_wstring("Segoe MDL2 Assets");
            let font_icons_16 = win32::CreateFontW(
                icon_16_size,
                0,
                0,
                0,
                700, // Bold to thicken icon lines
                0,
                0,
                0,
                1, // DEFAULT_CHARSET (Required for Segoe MDL2 Assets)
                0,
                0,
                5, // CLEARTYPE_QUALITY
                0,
                font_name.as_ptr(),
            );
            *font_guard = Some(SafeHFONT(font_icons_16));
        }

        // Recreate FONT_ICONS_18
        {
            let mut font_guard = state::FONT_ICONS_18
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(SafeHFONT(old_font)) = font_guard.take() {
                win32::DeleteObject(old_font);
            }
            let font_name = util::to_wstring("Segoe MDL2 Assets");
            let font_icons_18 = win32::CreateFontW(
                icon_18_size,
                0,
                0,
                0,
                700, // Bold to thicken icon lines
                0,
                0,
                0,
                1, // DEFAULT_CHARSET (Required for Segoe MDL2 Assets)
                0,
                0,
                5, // CLEARTYPE_QUALITY
                0,
                font_name.as_ptr(),
            );
            *font_guard = Some(SafeHFONT(font_icons_18));
        }

        // Apply to main window and child controls
        darkmode::apply_to_window(hwnd, is_dark);

        if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) =
            (EDIT_HWND.get(), LISTBOX_HWND.get())
        {
            let empty_str = [0u16];
            win32::SetWindowTheme(*hwnd_edit, empty_str.as_ptr(), empty_str.as_ptr());
            darkmode::apply_to_control(*hwnd_listbox, is_dark);

            if let Some(SafeHFONT(font)) =
                FONT_EDIT.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            {
                win32::SendMessageW(*hwnd_edit, win32::WM_SETFONT, *font as win32::WPARAM, 1);
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
                win32::SendMessageW(*hwnd_listbox, win32::WM_SETFONT, *font as win32::WPARAM, 1);
            }

            // Update listbox item height dynamically for high-DPI scaling
            let item_h = (26.0 * scale) as usize;
            win32::SendMessageW(
                *hwnd_listbox,
                0x01A0, /* LB_SETITEMHEIGHT */
                0,
                item_h as win32::LPARAM,
            );

            // Add padding to edit box (6px margin left and right)
            let margin_scaled = (6.0 * scale) as i32;
            win32::SendMessageW(
                *hwnd_edit,
                EM_SETMARGINS,
                EC_LEFTMARGIN | EC_RIGHTMARGIN,
                (margin_scaled | (margin_scaled << 16)) as win32::LPARAM,
            );

            // Set text placeholder (Cue Banner)
            win32::SendMessageW(
                *hwnd_edit,
                win32::EM_SETCUEBANNER,
                1,
                util::wstr_cue().as_ptr() as win32::LPARAM,
            );

            win32::InvalidateRect(*hwnd_edit, std::ptr::null(), 1);
            win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
        }

        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

pub fn update_top_index() {
    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
        unsafe {
            let top_index =
                win32::SendMessageW(*hwnd_listbox, win32::LB_GETTOPINDEX, 0, 0) as usize;
            let mut state_guard = lock_state();
            if let Some(state) = &mut *state_guard {
                state.top_index = top_index;
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn edit_subclass_proc(
    hwnd: win32::HWND,
    msg: u32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    if msg == win32::WM_KEYDOWN {
        state::log_debug(&format!("Edit KeyDown: vk={}", wparam));
        match wparam {
            9 => {
                // Tab key
                let target_mode = {
                    let state_guard = lock_state();
                    state_guard
                        .as_ref()
                        .map(|s| match s.mode {
                            Mode::History => Mode::Snippet,
                            Mode::Snippet => Mode::History,
                        })
                        .unwrap_or(Mode::History)
                };

                let changed = {
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        state.mode = target_mode;
                        if target_mode == Mode::Snippet {
                            state.snippets = std::sync::Arc::new(util::load_snippets());
                        }
                        true
                    } else {
                        false
                    }
                };

                if changed {
                    unsafe {
                        win32::SetWindowTextW(hwnd, [0u16].as_ptr());
                    }
                    ui::update_listbox_items();
                    ui::update_search_cue_banner();
                    if let Some(SafeHWND(hwnd_main)) = MAIN_HWND.get() {
                        unsafe {
                            win32::InvalidateRect(*hwnd_main, std::ptr::null(), 1);
                        }
                    }
                }
                return 0;
            }
            37 => {
                // Left Arrow
                let len = unsafe { win32::GetWindowTextLengthW(hwnd) };
                if len == 0 {
                    let has_parent = {
                        let state_guard = lock_state();
                        state_guard.as_ref().is_some_and(|s| {
                            s.mode == Mode::Snippet && !s.current_folder.is_empty()
                        })
                    };
                    if has_parent {
                        let mut state_guard = lock_state();
                        if let Some(state) = &mut *state_guard {
                            let mut parts = util::split_path(&state.current_folder);
                            if parts.len() > 1 {
                                parts.pop();
                                state.current_folder = util::join_path(&parts);
                            } else {
                                state.current_folder.clear();
                            }
                        }
                        std::mem::drop(state_guard);
                        ui::update_listbox_items();
                        ui::update_search_cue_banner();
                        return 0;
                    }
                }
            }
            39 => {
                // Right Arrow
                let len = unsafe { win32::GetWindowTextLengthW(hwnd) };
                if len == 0 {
                    let mut target_path = String::new();
                    let mut is_snippet_mode = false;

                    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                        let cur = unsafe {
                            win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0)
                        } as isize;
                        if cur != win32::LB_ERR {
                            let state_guard = lock_state();
                            if let Some(state) = &*state_guard {
                                is_snippet_mode = state.mode == Mode::Snippet;
                                if (cur as usize) < state.current_full_paths.len() {
                                    target_path = state.current_full_paths[cur as usize].clone();
                                }
                            }
                        }
                    }

                    if is_snippet_mode && !target_path.is_empty() {
                        if target_path == ".." {
                            let mut state_guard = lock_state();
                            if let Some(state) = &mut *state_guard {
                                let mut parts = util::split_path(&state.current_folder);
                                if parts.len() > 1 {
                                    parts.pop();
                                    state.current_folder = util::join_path(&parts);
                                } else {
                                    state.current_folder.clear();
                                }
                            }
                            std::mem::drop(state_guard);
                            ui::update_listbox_items();
                            ui::update_search_cue_banner();
                            return 0;
                        } else if target_path.starts_with("dir:") {
                            let folder = target_path["dir:".len()..].to_string();
                            let mut state_guard = lock_state();
                            if let Some(state) = &mut *state_guard {
                                state.current_folder = folder;
                            }
                            std::mem::drop(state_guard);
                            ui::update_listbox_items();
                            ui::update_search_cue_banner();
                            return 0;
                        }
                    }
                }
            }
            38 => {
                let repeat_count = (lparam & 0xFFFF) as i32;
                ui::move_listbox_selection(-repeat_count);
                return 0;
            }
            40 => {
                let repeat_count = (lparam & 0xFFFF) as i32;
                ui::move_listbox_selection(repeat_count);
                return 0;
            }
            33 | 34 => {
                // Page Up / Page Down
                if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                    unsafe {
                        win32::SendMessageW(*hwnd_listbox, win32::WM_KEYDOWN, wparam, lparam);
                    }
                    update_top_index();
                    unsafe {
                        win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 0);
                    }
                }
                return 0;
            }
            13 => {
                ui::on_select();
                return 0;
            }
            27 => {
                ui::hide_window();
                return 0;
            }
            46 => {
                let repeat_count = (lparam & 0xFFFF) as usize;
                ui::delete_selected_items(repeat_count);
                return 0;
            }
            8 => {
                // Backspace
                let len = unsafe { win32::GetWindowTextLengthW(hwnd) };
                if len == 0 {
                    let has_parent = {
                        let state_guard = lock_state();
                        state_guard
                            .as_ref()
                            .is_some_and(|s| !s.current_folder.is_empty())
                    };
                    if has_parent {
                        let mut state_guard = lock_state();
                        if let Some(state) = &mut *state_guard {
                            let mut parts = util::split_path(&state.current_folder);
                            if parts.len() > 1 {
                                parts.pop();
                                state.current_folder = util::join_path(&parts);
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
                0x4E | 0x4A => {
                    ui::move_listbox_selection(1);
                    return 0;
                }
                0x50 | 0x4B => {
                    ui::move_listbox_selection(-1);
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
pub unsafe extern "system" fn listbox_subclass_proc(
    hwnd: win32::HWND,
    msg: u32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    let is_scroll_msg =
        msg == WM_VSCROLL || msg == WM_MOUSEWHEEL || msg == win32::WM_KEYDOWN || msg == 0x0101; // WM_KEYUP

    if msg == win32::WM_KEYDOWN && wparam == 46 {
        let repeat_count = (lparam & 0xFFFF) as usize;
        ui::delete_selected_items(repeat_count);
        return 0;
    }

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
    let res = unsafe {
        if let Some(SafeWndProc(old_proc)) = old_proc_opt {
            old_proc(hwnd, msg, wparam, lparam)
        } else {
            win32::DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    };

    if is_scroll_msg {
        unsafe {
            win32::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
        update_top_index();
    }

    res
}

#[cfg(target_os = "windows")]
pub unsafe extern "system" fn window_proc(
    hwnd: win32::HWND,
    msg: u32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
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
                    win32::WS_CHILD
                        | win32::WS_VISIBLE
                        | win32::ES_AUTOHSCROLL
                        | win32::ES_LEFT
                        | 0x0004,
                    0,
                    0,
                    0,
                    0,
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
                    win32::WS_CHILD
                        | win32::WS_VISIBLE
                        | win32::WS_VSCROLL
                        | win32::LBS_NOTIFY
                        | win32::LBS_HASSTRINGS
                        | win32::LBS_NOINTEGRALHEIGHT
                        | win32::LBS_OWNERDRAWFIXED,
                    0,
                    0,
                    0,
                    0,
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
                win32::SetWindowLongPtrW(
                    hwnd_edit,
                    win32::GWLP_WNDPROC,
                    edit_subclass_proc as *const () as win32::LONG_PTR,
                )
            };
            let _ = OLD_EDIT_PROC.set(SafeWndProc(unsafe { std::mem::transmute(old_proc) }));
            state::log_debug(&format!("Edit subclass applied. Old proc: {:?}", old_proc));

            let old_listbox_proc = unsafe {
                win32::SetWindowLongPtrW(
                    hwnd_listbox,
                    win32::GWLP_WNDPROC,
                    listbox_subclass_proc as *const () as win32::LONG_PTR,
                )
            };
            let _ = state::OLD_LISTBOX_PROC.set(SafeWndProc(unsafe {
                std::mem::transmute(old_listbox_proc)
            }));
            state::log_debug(&format!(
                "ListBox subclass applied. Old proc: {:?}",
                old_listbox_proc
            ));

            // Apply theme resources right after controls are initialized
            let is_dark = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            update_theme_resources(hwnd, is_dark);
        }
        win32::WM_SIZE => {
            if let (Some(SafeHWND(hwnd_edit)), Some(SafeHWND(hwnd_listbox))) =
                (EDIT_HWND.get(), LISTBOX_HWND.get())
            {
                let scale = unsafe { win32::GetDpiForWindow(hwnd) } as f32 / 96.0;
                let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
                unsafe { win32::GetClientRect(hwnd, &mut rc) };
                let cw = rc.right - rc.left;
                let ch = rc.bottom - rc.top;

                let margin = (6.0 * scale) as i32;
                let tab_bar_h = (28.0 * scale) as i32;
                let edit_container_h = (34.0 * scale) as i32;
                let edit_h = (26.0 * scale) as i32;
                let gap = (4.0 * scale) as i32;

                let edit_container_y = margin + tab_bar_h + gap;
                let listbox_y = edit_container_y + edit_container_h + gap;
                let listbox_w = cw - margin * 2;
                let listbox_h = ch - listbox_y - margin;

                unsafe {
                    win32::MoveWindow(*hwnd_listbox, margin, listbox_y, listbox_w, listbox_h, 1);
                }

                let edit_y = edit_container_y + (edit_container_h - edit_h) / 2;
                let edit_x = margin + (28.0 * scale) as i32; // margin + icon area
                let edit_w = listbox_w - (60.0 * scale) as i32; // Align right edge with space for clear button

                unsafe {
                    win32::MoveWindow(*hwnd_edit, edit_x, edit_y, edit_w, edit_h, 1);
                }
            }
        }
        win32::WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let scale = unsafe { win32::GetDpiForWindow(hwnd) } as f32 / 96.0;
            let margin = (6.0 * scale) as i32;
            let tab_bar_h = (28.0 * scale) as i32;

            let mut client_rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut client_rc) };
            let cw = client_rc.right - client_rc.left;
            let tab_bar_w = cw - margin * 2;

            // Check if click was inside the tab bar
            if y >= margin && y <= margin + tab_bar_h && x >= margin && x <= margin + tab_bar_w {
                let clicked_idx = ((x - margin) / (tab_bar_w / 2)).clamp(0, 1);
                let target_mode = if clicked_idx == 0 {
                    Mode::History
                } else {
                    Mode::Snippet
                };

                let changed = {
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        if state.mode != target_mode {
                            state.mode = target_mode;
                            if target_mode == Mode::Snippet {
                                state.snippets = std::sync::Arc::new(util::load_snippets());
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if changed {
                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, [0u16].as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    ui::update_listbox_items();
                    ui::update_search_cue_banner();
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                }
            }

            // Check if click was inside the clear button area of the search box
            let gap = (4.0 * scale) as i32;
            let edit_container_h = (28.0 * scale) as i32;
            let edit_container_y = margin + tab_bar_h + gap;
            let listbox_w = cw - margin * 2;
            let clear_btn_left = margin + listbox_w - (34.0 * scale) as i32;
            let clear_btn_right = margin + listbox_w - (6.0 * scale) as i32;
            let clear_btn_top = edit_container_y;
            let clear_btn_bottom = edit_container_y + edit_container_h;

            if x >= clear_btn_left && x <= clear_btn_right && y >= clear_btn_top && y <= clear_btn_bottom {
                let has_text = unsafe {
                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        win32::GetWindowTextLengthW(*hwnd_edit) > 0
                    } else {
                        false
                    }
                };

                if has_text {
                    if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                        unsafe {
                            win32::SetWindowTextW(*hwnd_edit, [0u16].as_ptr());
                            win32::SetFocus(*hwnd_edit);
                        }
                    }
                    ui::update_listbox_items();
                    ui::update_search_cue_banner();
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                    return 0; // Handled click
                }
            }
        }
        win32::WM_COMMAND => {
            let ctrl_id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if ctrl_id == 101 {
                if code == win32::EN_CHANGE as usize {
                    ui::update_listbox_items();
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                } else if code == 0x0100
                /* EN_SETFOCUS */
                {
                    EDIT_FOCUSED.store(true, Ordering::SeqCst);
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                } else if code == 0x0200
                /* EN_KILLFOCUS */
                {
                    EDIT_FOCUSED.store(false, Ordering::SeqCst);
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }

                    let focus_hwnd = unsafe { win32::GetFocus() };
                    let is_child = if focus_hwnd.is_null() {
                        false
                    } else {
                        focus_hwnd == hwnd
                            || EDIT_HWND.get().is_some_and(|h| focus_hwnd == h.0)
                            || LISTBOX_HWND.get().is_some_and(|h| focus_hwnd == h.0)
                    };
                    if !is_child {
                        ui::hide_window();
                    }
                }
            } else if ctrl_id == 102 {
                if code == 2 {
                    // LBN_DBLCLK
                    ui::on_select();
                } else if code == LBN_SELCHANGE as usize {
                    update_top_index();

                    // Navigate folders immediately on a single click
                    if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                        let cur = unsafe {
                            win32::SendMessageW(*hwnd_listbox, win32::LB_GETCURSEL, 0, 0)
                        } as isize;
                        if cur != win32::LB_ERR {
                            let mut is_folder = false;
                            {
                                let state_guard = lock_state();
                                if let Some(state) = &*state_guard
                                    && state.mode == Mode::Snippet
                                    && (cur as usize) < state.current_full_paths.len()
                                {
                                    let target = &state.current_full_paths[cur as usize];
                                    is_folder = target == ".." || target.starts_with("dir:");
                                }
                            }
                            if is_folder {
                                ui::on_select();
                            }
                        }
                    }
                } else if code == 5 {
                    // LBN_KILLFOCUS
                    let focus_hwnd = unsafe { win32::GetFocus() };
                    let is_child = if focus_hwnd.is_null() {
                        false
                    } else {
                        focus_hwnd == hwnd
                            || EDIT_HWND.get().is_some_and(|h| focus_hwnd == h.0)
                            || LISTBOX_HWND.get().is_some_and(|h| focus_hwnd == h.0)
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
                let state_guard = lock_state();
                state_guard.as_ref().and_then(|state| {
                    if state.filter_generation == generation {
                        Some(state.current_results.clone())
                    } else {
                        None
                    }
                })
            };

            if let Some(items) = display_items
                && let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get()
            {
                unsafe {
                    win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 0, 0);
                    win32::SendMessageW(*hwnd_listbox, win32::LB_RESETCONTENT, 0, 0);
                    for item in &items {
                        let item_w = util::to_wstring(item);
                        win32::SendMessageW(
                            *hwnd_listbox,
                            win32::LB_ADDSTRING,
                            0,
                            item_w.as_ptr() as win32::LPARAM,
                        );
                    }
                    if !items.is_empty() {
                        win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, 0, 0);
                    }
                    win32::SendMessageW(*hwnd_listbox, 0x000B /* WM_SETREDRAW */, 1, 0);
                    win32::InvalidateRect(*hwnd_listbox, std::ptr::null(), 1);
                }
                update_top_index();
            }
        }
        win32::WM_CTLCOLOREDIT => {
            let hdc = wparam as win32::HDC;
            let is_dark = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            unsafe {
                win32::SetTextColor(hdc, colors.text_color);
                win32::SetBkColor(hdc, colors.edit_bg);
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_EDIT
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
                return *brush as win32::LRESULT;
            }
        }
        win32::WM_CTLCOLORLISTBOX => {
            let hdc = wparam as win32::HDC;
            let is_dark = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            unsafe {
                win32::SetTextColor(hdc, colors.text_color);
                win32::SetBkColor(hdc, colors.window_bg);
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
                return *brush as win32::LRESULT;
            }
        }
        win32::WM_SETTINGCHANGE => {
            let is_sys_dark = darkmode::is_system_dark_mode();
            let icon_id = if is_sys_dark { 2 } else { 1 };
            unsafe {
                let hinstance = win32::GetModuleHandleW(std::ptr::null());
                let new_icon = win32::LoadIconW(hinstance, icon_id as *const u16);
                if !new_icon.is_null() {
                    let mut nid: win32::NOTIFYICONDATAW = std::mem::zeroed();
                    nid.cbSize = std::mem::size_of::<win32::NOTIFYICONDATAW>() as u32;
                    nid.hWnd = hwnd;
                    nid.uID = 1;
                    nid.uFlags = win32::NIF_ICON;
                    nid.hIcon = new_icon;
                    win32::Shell_NotifyIconW(win32::NIM_MODIFY, &nid);
                }
            }

            // Dynamically update window theme if currently visible and active theme has changed
            let mut state_guard = lock_state();
            if let Some(state) = &mut *state_guard {
                let new_is_dark = darkmode::is_dark_active();
                if state.visible && state.is_dark != new_is_dark {
                    state.is_dark = new_is_dark;
                    update_theme_resources(hwnd, new_is_dark);
                    unsafe {
                        win32::InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                }
            }
        }
        win32::WM_MEASUREITEM => {
            let mis = lparam as *mut win32::MEASUREITEMSTRUCT;
            if !mis.is_null() {
                let scale = unsafe { win32::GetDpiForWindow(hwnd) } as f32 / 96.0;
                unsafe {
                    (*mis).item_height = (26.0 * scale) as u32;
                }
            }
            return 1;
        }
        win32::WM_DRAWITEM => {
            let dis = lparam as *const win32::DRAWITEMSTRUCT;
            if dis.is_null() {
                return 0;
            }
            let dis = unsafe { *dis };

            if dis.item_id == u32::MAX {
                return 1;
            }

            let hdc = dis.hdc;
            let rc = dis.rc_item;
            let selected = (dis.item_state & win32::ODS_SELECTED) != 0;

            let (is_dark, _mode, is_folder) = {
                let state_guard = lock_state();
                state_guard
                    .as_ref()
                    .map_or((false, Mode::Snippet, false), |s| {
                        let is_folder = if s.mode == Mode::Snippet
                            && (dis.item_id as usize) < s.current_full_paths.len()
                        {
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
            let len = unsafe {
                win32::SendMessageW(dis.hwnd_item, win32::LB_GETTEXTLEN, dis.item_id as usize, 0)
            } as usize;

            // Use stack-allocated buffer for typical lengths to avoid heap allocation
            let mut stack_buf = [0u16; 512];
            let mut heap_buf = Vec::new();
            let buf_ptr = if len < 512 {
                stack_buf.as_mut_ptr()
            } else {
                heap_buf = vec![0u16; len + 1];
                heap_buf.as_mut_ptr()
            };
            unsafe {
                win32::SendMessageW(
                    dis.hwnd_item,
                    win32::LB_GETTEXT,
                    dis.item_id as usize,
                    buf_ptr as win32::LPARAM,
                )
            };

            // Setup colors based on selection status
            let bg_color = if selected {
                colors.sel_bg
            } else {
                colors.window_bg
            };
            let text_color = if selected {
                colors.sel_text
            } else {
                colors.text_color
            };

            // Draw item background using cached brushes
            let (bg_brush, delete_brush) = if selected {
                if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                {
                    (*brush, false)
                } else {
                    (unsafe { win32::CreateSolidBrush(bg_color) }, true)
                }
            } else {
                if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                {
                    (*brush, false)
                } else {
                    (unsafe { win32::CreateSolidBrush(bg_color) }, true)
                }
            };

            let scale = unsafe { win32::GetDpiForWindow(dis.hwnd_item) } as f32 / 96.0;

            let top_index =
                unsafe { win32::SendMessageW(dis.hwnd_item, win32::LB_GETTOPINDEX, 0, 0) } as usize;
            let relative_idx = (dis.item_id as usize).checked_sub(top_index);
            let shortcut_char_opt = if let Some(idx) = relative_idx {
                if idx < SHORTCUT_CHARS.len() {
                    Some(SHORTCUT_CHARS[idx])
                } else {
                    None
                }
            } else {
                None
            };

            if selected {
                let gap_x = (2.0 * scale) as i32;
                let gap_y = (1.0 * scale) as i32;
                // Pill-shaped rounded floating background: add 2px horizontal and 1px vertical gap (scaled)
                let pill_rc = win32::RECT {
                    left: rc.left + gap_x,
                    top: rc.top + gap_y,
                    right: rc.right - gap_x,
                    bottom: rc.bottom - gap_y,
                };
                unsafe {
                    let old_brush = win32::SelectObject(hdc, bg_brush);
                    let old_pen = win32::SelectObject(hdc, win32::GetStockObject(8 /* NULL_PEN */));

                    let round_size = (4.0 * scale) as i32;
                    // Draw rounded rectangle with scaled ellipse diameter (originally 8px, reduced to 4px for sharper look)
                    win32::RoundRect(
                        hdc,
                        pill_rc.left,
                        pill_rc.top,
                        pill_rc.right,
                        pill_rc.bottom,
                        round_size,
                        round_size,
                    );

                    win32::SelectObject(hdc, old_brush);
                    win32::SelectObject(hdc, old_pen);
                }
            } else {
                unsafe { win32::FillRect(hdc, &rc, bg_brush) };
            }
            if delete_brush {
                unsafe { win32::DeleteObject(bg_brush) };
            }

            unsafe {
                win32::SetBkMode(hdc, 1 /* TRANSPARENT */)
            };

            let is_in_fifo_lifo_queue = {
                let state_guard = lock_state();
                state_guard.as_ref().map_or(false, |s| {
                    s.mode == Mode::History
                        && (dis.item_id as usize) < s.current_full_paths.len()
                        && s.fifo_lifo_queue
                            .contains(&s.current_full_paths[dis.item_id as usize])
                })
            };

            // Draw shortcut keycap if applicable
            let shortcut_width = (16.0 * scale) as i32;

            if let Some(shortcut_char) = shortcut_char_opt {
                let key_size_w = (16.0 * scale) as i32;
                let key_size_h = (16.0 * scale) as i32;
                let key_x = rc.left + (4.0 * scale) as i32;
                let key_y = rc.top + (rc.bottom - rc.top - key_size_h) / 2;
                let mut key_rc = win32::RECT {
                    left: key_x,
                    top: key_y,
                    right: key_x + key_size_w,
                    bottom: key_y + key_size_h,
                };

                unsafe {
                    let mut pen_color = colors.border_color;
                    let mut key_text_color = colors.text_color;

                    if is_in_fifo_lifo_queue && !selected {
                        pen_color = colors.sel_bg;
                        key_text_color = colors.sel_bg;
                    }

                    let key_pen = win32::CreatePen(win32::PS_SOLID, 1, pen_color);
                    let key_brush_guard = BRUSH_EDIT.lock().unwrap_or_else(|e| e.into_inner());
                    let key_brush = if let Some(SafeHBRUSH(brush)) = key_brush_guard.as_ref() {
                        *brush
                    } else {
                        std::ptr::null_mut()
                    };

                    let old_pen = win32::SelectObject(hdc, key_pen);
                    let old_brush = win32::SelectObject(hdc, key_brush);

                    let round_size = (3.0 * scale) as i32;
                    win32::RoundRect(
                        hdc,
                        key_rc.left,
                        key_rc.top,
                        key_rc.right,
                        key_rc.bottom,
                        round_size,
                        round_size,
                    );

                    win32::SelectObject(hdc, old_pen);
                    win32::SelectObject(hdc, old_brush);
                    win32::DeleteObject(key_pen);

                    let old_text_color = win32::SetTextColor(hdc, key_text_color);

                    let font_to_use = FONT_LISTBOX.lock().unwrap_or_else(|e| e.into_inner());
                    let mut old_font = None;
                    if let Some(SafeHFONT(font)) = font_to_use.as_ref() {
                        old_font = Some(win32::SelectObject(hdc, *font as win32::HGDIOBJ));
                    }

                    let char_utf16 = [shortcut_char as u16];
                    win32::DrawTextW(
                        hdc,
                        char_utf16.as_ptr(),
                        1,
                        &mut key_rc,
                        win32::DT_SINGLELINE
                            | win32::DT_CENTER
                            | win32::DT_VCENTER
                            | win32::DT_NOPREFIX,
                    );

                    if let Some(o_font) = old_font {
                        win32::SelectObject(hdc, o_font);
                    }
                    win32::SetTextColor(hdc, old_text_color);
                }
            }

            // Parse text and tags directly on UTF-16 slice to prevent heap allocations
            let wide_text = if len < 512 {
                &stack_buf[..len]
            } else {
                &heap_buf[..len]
            };
            let (icon_type_opt, clean_text_w) = if wide_text.starts_with(&[91, 68, 73, 82, 93, 32])
            {
                // "[DIR] "
                let contains_dots = wide_text.windows(2).any(|w| w == [46, 46]);
                let icon = if contains_dots {
                    IconType::ParentFolder
                } else {
                    IconType::Folder
                };
                (Some(icon), &wide_text[6..])
            } else if wide_text.starts_with(&[91, 83, 78, 73, 80, 93, 32]) {
                // "[SNIP] "
                (None, &wide_text[7..])
            } else if wide_text.starts_with(&[91, 72, 73, 83, 84, 93, 32]) {
                // "[HIST] "
                (None, &wide_text[7..])
            } else {
                (None, wide_text)
            };

            // Draw icon if present
            let has_icon = icon_type_opt.is_some();
            if let Some(icon_type) = icon_type_opt {
                let icon_size = (16.0 * scale) as i32;
                let icon_left = (12.0 * scale) as i32;
                let icon_x = rc.left + icon_left + shortcut_width;
                let icon_y = rc.top + (rc.bottom - rc.top - icon_size) / 2;
                // Use dim color for non-selected icons, accent color when selected
                let icon_color = if selected {
                    colors.sel_text
                } else {
                    colors.dim_text_color
                };
                draw_vector_icon(hdc, icon_type, icon_x, icon_y, icon_size, icon_color);
            }

            // Apply normal/bold font based on selection
            let font_to_use = if selected {
                FONT_LISTBOX_BOLD.lock().unwrap_or_else(|e| e.into_inner())
            } else {
                FONT_LISTBOX.lock().unwrap_or_else(|e| e.into_inner())
            };
            if let Some(SafeHFONT(font)) = font_to_use.as_ref() {
                unsafe { win32::SelectObject(hdc, *font as win32::HGDIOBJ) };
            }

            // Draw candidate text (adjust margin based on pill shape padding)
            let text_left_margin = if has_icon {
                shortcut_width + (34.0 * scale) as i32
            } else {
                shortcut_width + (12.0 * scale) as i32
            };
            let text_right_margin = if is_folder {
                (26.0 * scale) as i32
            } else {
                (12.0 * scale) as i32
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
                    hdc,
                    clean_text_w.as_ptr(),
                    clean_text_w.len() as i32,
                    &mut text_rc,
                    win32::DT_SINGLELINE
                        | win32::DT_VCENTER
                        | win32::DT_LEFT
                        | win32::DT_END_ELLIPSIS
                        | win32::DT_NOPREFIX,
                );
            }

            // Draw folder indicator (chevron-right) on the right edge using native vector drawing
            if is_folder {
                let arrow_x = rc.right - 20;
                let arrow_y = rc.top + (rc.bottom - rc.top - 14) / 2;
                let chevron_color = if selected {
                    colors.sel_text
                } else {
                    colors.dim_text_color
                };
                draw_vector_icon(
                    hdc,
                    IconType::ChevronRight,
                    arrow_x,
                    arrow_y,
                    14,
                    chevron_color,
                );
            }

            return 1;
        }
        win32::WM_PAINT => {
            let mut ps = unsafe { std::mem::zeroed::<win32::PAINTSTRUCT>() };
            let hdc = unsafe { win32::BeginPaint(hwnd, &mut ps) };

            let is_dark = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            let scale = unsafe { win32::GetDpiForWindow(hwnd) } as f32 / 96.0;

            let mut client_rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut client_rc) };
            let cw = client_rc.right - client_rc.left;

            // Draw a rounded input container border/background (scaled)
            let margin = (6.0 * scale) as i32;
            let gap_x = (2.0 * scale) as i32;
            let tab_bar_h = (28.0 * scale) as i32;
            let edit_container_h = (34.0 * scale) as i32;
            let gap = (4.0 * scale) as i32;

            let listbox_w = cw - margin * 2;

            // Draw the tab bar (segment control) at the top
            let tab_bar_rc = win32::RECT {
                left: margin,
                top: margin,
                right: cw - margin,
                bottom: margin + tab_bar_h,
            };

            let active_mode = {
                let state_guard = lock_state();
                state_guard
                    .as_ref()
                    .map(|s| s.mode)
                    .unwrap_or(Mode::History)
            };

            unsafe {
                let border_pen = win32::CreatePen(win32::PS_SOLID, 1, colors.border_color);
                let old_pen = win32::SelectObject(hdc, border_pen);
                let bg_brush = win32::CreateSolidBrush(colors.edit_bg);
                let old_brush = win32::SelectObject(hdc, bg_brush);
                let round_r = (4.0 * scale) as i32;
                win32::RoundRect(
                    hdc,
                    tab_bar_rc.left,
                    tab_bar_rc.top,
                    tab_bar_rc.right,
                    tab_bar_rc.bottom,
                    round_r,
                    round_r,
                );
                win32::SelectObject(hdc, old_brush);
                win32::DeleteObject(bg_brush);
                win32::SelectObject(hdc, old_pen);
                win32::DeleteObject(border_pen);
            }

            let padding = (2.0 * scale) as i32;
            let tab_bar_w = tab_bar_rc.right - tab_bar_rc.left;
            let active_idx = match active_mode {
                Mode::History => 0,
                Mode::Snippet => 1,
            };

            let font_to_use = FONT_LISTBOX.lock().unwrap_or_else(|e| e.into_inner());
            let mut old_font = None;
            if let Some(SafeHFONT(font)) = font_to_use.as_ref() {
                old_font = Some(unsafe { win32::SelectObject(hdc, *font as win32::HGDIOBJ) });
            }

            for idx in 0..2 {
                let (tab_name, icon_type) = if idx == 0 {
                    ("履歴", IconType::History)
                } else {
                    ("スニペット", IconType::Snippet)
                };

                let seg_left = tab_bar_rc.left + idx * (tab_bar_w / 2);
                let seg_right = if idx == 0 {
                    tab_bar_rc.left + tab_bar_w / 2
                } else {
                    tab_bar_rc.right
                };

                let is_active = idx == active_idx;

                if is_active {
                    let pill_rc = win32::RECT {
                        left: seg_left + padding,
                        top: tab_bar_rc.top + padding,
                        right: seg_right - padding,
                        bottom: tab_bar_rc.bottom - padding,
                    };
                    unsafe {
                        let pill_brush = win32::CreateSolidBrush(colors.sel_bg);
                        let old_brush = win32::SelectObject(hdc, pill_brush);
                        let old_pen =
                            win32::SelectObject(hdc, win32::GetStockObject(8 /* NULL_PEN */));
                        let round_size = (3.0 * scale) as i32;
                        win32::RoundRect(
                            hdc,
                            pill_rc.left,
                            pill_rc.top,
                            pill_rc.right,
                            pill_rc.bottom,
                            round_size,
                            round_size,
                        );
                        win32::SelectObject(hdc, old_brush);
                        win32::SelectObject(hdc, old_pen);
                        win32::DeleteObject(pill_brush);
                    }
                }

                let w_text = util::to_wstring(tab_name);
                let mut size_struct = win32::SIZE { cx: 0, cy: 0 };
                unsafe {
                    win32::GetTextExtentPoint32W(
                        hdc,
                        w_text.as_ptr(),
                        w_text.len() as i32,
                        &mut size_struct,
                    );
                }

                let icon_w = (14.0 * scale) as i32;
                let spacing = (4.0 * scale) as i32;
                let total_content_w = icon_w + spacing + size_struct.cx;

                let seg_w = seg_right - seg_left;
                let content_left = seg_left + (seg_w - total_content_w) / 2;

                let icon_x = content_left;
                let icon_y = tab_bar_rc.top + (tab_bar_h - icon_w) / 2;

                let text_color = if is_active {
                    colors.sel_text
                } else {
                    colors.dim_text_color
                };

                draw_vector_icon(hdc, icon_type, icon_x, icon_y, icon_w, text_color);

                let mut text_rc = win32::RECT {
                    left: icon_x + icon_w + spacing,
                    top: tab_bar_rc.top,
                    right: seg_right,
                    bottom: tab_bar_rc.bottom,
                };

                unsafe {
                    win32::SetTextColor(hdc, text_color);
                    win32::SetBkMode(hdc, 1 /* TRANSPARENT */);
                    win32::DrawTextW(
                        hdc,
                        w_text.as_ptr(),
                        w_text.len() as i32,
                        &mut text_rc,
                        win32::DT_SINGLELINE
                            | win32::DT_VCENTER
                            | win32::DT_LEFT
                            | win32::DT_NOPREFIX,
                    );
                }
            }

            if let Some(o_font) = old_font {
                unsafe { win32::SelectObject(hdc, o_font) };
            }

            let edit_container_y = margin + tab_bar_h + gap;
            let container_rc = win32::RECT {
                left: margin + gap_x,
                top: edit_container_y,
                right: margin + listbox_w - gap_x,
                bottom: edit_container_y + edit_container_h,
            };

            // Choose border color and width based on focus
            let focus = EDIT_FOCUSED.load(Ordering::SeqCst);
            let border_color = if focus {
                colors.sel_bg
            } else {
                colors.border_color
            };
            let border_width = if focus { (2.0 * scale) as i32 } else { 1 };

            // Draw container background and border using RoundRect
            unsafe {
                let border_pen = win32::CreatePen(win32::PS_SOLID, border_width, border_color);
                let old_pen = win32::SelectObject(hdc, border_pen);

                let bg_brush = win32::CreateSolidBrush(colors.edit_bg);
                let old_brush = win32::SelectObject(hdc, bg_brush);

                let round_r = (4.0 * scale) as i32;
                // Draw rounded rect with scaled corner radius (originally 10px, reduced to 4px for sharper look)
                win32::RoundRect(
                    hdc,
                    container_rc.left,
                    container_rc.top,
                    container_rc.right,
                    container_rc.bottom,
                    round_r,
                    round_r,
                );

                win32::SelectObject(hdc, old_brush);
                win32::DeleteObject(bg_brush);

                win32::SelectObject(hdc, old_pen);
                win32::DeleteObject(border_pen);
            }

            // Draw the 🔍 search icon inside the container using native vector drawing
            let icon_color = colors.dim_text_color;
            let icon_size = (18.0 * scale) as i32;
            let icon_margin_left = (6.0 * scale) as i32;
            draw_vector_icon(
                hdc,
                IconType::Search,
                margin + icon_margin_left,
                edit_container_y + (edit_container_h - icon_size) / 2,
                icon_size,
                icon_color,
            );

            // Draw clear button if edit control has text
            let has_text = unsafe {
                if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                    win32::GetWindowTextLengthW(*hwnd_edit) > 0
                } else {
                    false
                }
            };
            if has_text {
                let clear_icon_size = (16.0 * scale) as i32;
                let clear_icon_x = margin + listbox_w - (30.0 * scale) as i32;
                let clear_icon_y = edit_container_y + (edit_container_h - clear_icon_size) / 2;
                draw_vector_icon(
                    hdc,
                    IconType::Clear,
                    clear_icon_x,
                    clear_icon_y,
                    clear_icon_size,
                    colors.dim_text_color,
                );
            }


            // Draw 1px clean border of the main window itself
            let mut delete_border = false;
            let border_brush = if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
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
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            let colors = if is_dark { &DARK_THEME } else { &LIGHT_THEME };

            let mut rc: win32::RECT = unsafe { std::mem::zeroed() };
            unsafe { win32::GetClientRect(hwnd, &mut rc) };

            let mut delete_bg = false;
            let bg_brush = if let Some(SafeHBRUSH(brush)) =
                BRUSH_BG.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            {
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
                    let state_guard = lock_state();
                    state_guard.as_ref().is_some_and(|s| s.visible)
                };
                if is_visible {
                    let now = unsafe { win32::GetTickCount() };
                    let show_time = state::LAST_SHOW_TIME.load(Ordering::Relaxed);
                    if now.wrapping_sub(show_time) > 500 {
                        state::log_debug("Window inactive, hiding window...");
                        ui::hide_window();
                    } else {
                        state::log_debug(
                            "WM_ACTIVATE: Ignored inactive state because window was just shown.",
                        );
                    }
                }
            } else {
                if let Some(SafeHWND(hwnd_edit)) = EDIT_HWND.get() {
                    unsafe { win32::SetFocus(*hwnd_edit) };
                    state::log_debug("SetFocus called on Edit control.");
                }
            }
        }
        0x001C => {
            // WM_ACTIVATEAPP
            state::log_debug(&format!("WM_ACTIVATEAPP: wparam={}", wparam));
            if wparam == 0 {
                // Being deactivated
                let is_visible = {
                    let state_guard = lock_state();
                    state_guard.as_ref().is_some_and(|s| s.visible)
                };
                if is_visible {
                    let now = unsafe { win32::GetTickCount() };
                    let show_time = state::LAST_SHOW_TIME.load(Ordering::Relaxed);
                    if now.wrapping_sub(show_time) > 500 {
                        state::log_debug("App inactive, hiding window...");
                        ui::hide_window();
                    } else {
                        state::log_debug(
                            "WM_ACTIVATEAPP: Ignored inactive state because window was just shown.",
                        );
                    }
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
        WM_HIDE_WINDOW => {
            ui::hide_window();
        }
        WM_FIFO_LIFO_PASTE => {
            // [連打対策のディレイ制御]
            // 前回のペースト処理完了から 150ms 以上経過するまで次の処理を遅延させます。
            // これにより、OSのキーイベント配送やターゲットアプリのクリップボード読み込みラグを
            // 安全に待機し、重複ペーストや文字 'v' の誤入力を完全に防止します。
            static LAST_PASTE_TIME: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let last = LAST_PASTE_TIME.load(Ordering::Relaxed);
            let diff = now.saturating_sub(last);

            if diff < 150 {
                // まだ 150ms 経過していない場合は、メインスレッドをスリープでブロックせず、
                // 非同期のバックグラウンドスレッドで残余時間だけスリープした後に、
                // 本ウィンドウプロシージャへ同じメッセージを再送（PostMessage）して遅延処理させます。
                let hwnd_val = hwnd as isize;
                let delay = 150 - diff;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    unsafe {
                        win32::PostMessageW(hwnd_val as win32::HWND, WM_FIFO_LIFO_PASTE, 0, 0);
                    }
                });
                return 0; // メインスレッドでの現在の処理はスキップして即時リターンします
            }
            LAST_PASTE_TIME.store(now, Ordering::Relaxed);

            let text_to_paste = {
                let mut state_guard = lock_state();
                if let Some(state) = &mut *state_guard {
                    match state.fifo_lifo_mode {
                        FifoLifoMode::Fifo => state.fifo_lifo_queue.pop_front(),
                        FifoLifoMode::Lifo => state.fifo_lifo_queue.pop_back(),
                        _ => None,
                    }
                } else {
                    None
                }
            };

            if let Some(text) = text_to_paste {
                let mut success = false;
                for _ in 0..10 {
                    if util::set_clipboard_text(&text) {
                        let mut state_guard = lock_state();
                        if let Some(state) = &mut *state_guard {
                            state.last_clipboard_value = text.clone();
                        }
                        success = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                if success {
                    ui::simulate_paste();
                }

                ui::update_tray_tip_and_icon(hwnd);

                let queue_len = {
                    let state_guard = lock_state();
                    state_guard
                        .as_ref()
                        .map(|s| s.fifo_lifo_queue.len())
                        .unwrap_or(0)
                };
                if queue_len == 0 {
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        state.fifo_lifo_mode = FifoLifoMode::None;
                    }
                    std::mem::drop(state_guard);
                    ui::update_tray_tip_and_icon(hwnd);
                    ui::show_notification_ex(
                        "キューが空になりました",
                        "FIFO/LIFOペーストが完了し、通常モードに戻りました。",
                        false,
                        true,
                    );
                }
            }
        }
        WM_TOGGLE_FIFO_LIFO => {
            let target_mode = match wparam {
                1 => FifoLifoMode::Fifo,
                2 => FifoLifoMode::Lifo,
                _ => FifoLifoMode::None,
            };

            let (_old_mode, new_mode) = {
                let mut state_guard = lock_state();
                if let Some(state) = &mut *state_guard {
                    let old = state.fifo_lifo_mode;
                    if old == target_mode {
                        state.fifo_lifo_mode = FifoLifoMode::None;
                    } else {
                        state.fifo_lifo_mode = target_mode;
                    }
                    if state.fifo_lifo_mode == FifoLifoMode::None {
                        state.fifo_lifo_queue.clear();
                    }
                    (old, state.fifo_lifo_mode)
                } else {
                    (FifoLifoMode::None, FifoLifoMode::None)
                }
            };

            ui::update_tray_tip_and_icon(hwnd);

            let notification_title = match new_mode {
                FifoLifoMode::Fifo => "FIFO モード開始",
                FifoLifoMode::Lifo => "LIFO モード開始",
                FifoLifoMode::None => "通常モード (FIFO/LIFO 終了)",
            };
            let notification_body = match new_mode {
                FifoLifoMode::Fifo => {
                    "コピーしたデータが古い順にペーストされます。\n(Ctrl+Shift+F で解除)"
                }
                FifoLifoMode::Lifo => {
                    "コピーしたデータが新しい順にペーストされます。\n(Ctrl+Shift+L で解除)"
                }
                FifoLifoMode::None => "通常モードに戻りました。",
            };

            if new_mode == FifoLifoMode::None {
                ui::show_notification_ex(notification_title, notification_body, false, true);
            } else {
                ui::show_notification(notification_title, notification_body, false);
            }
        }
        win32::WM_CLIPBOARDUPDATE => {
            let is_excluded = if let Some(active_app) = util::get_active_process_name() {
                state::CONFIG.get().map_or(false, |c| {
                    c.exclude_apps
                        .iter()
                        .any(|app| app.eq_ignore_ascii_case(&active_app))
                })
            } else {
                false
            };

            if !is_excluded
                && let Some(text) = util::get_clipboard_text()
                && !text.is_empty()
            {
                let mut is_new = false;
                let (mode, queue_updated) = {
                    let mut state_guard = lock_state();
                    if let Some(state) = &mut *state_guard {
                        if text != state.last_clipboard_value {
                            state.last_clipboard_value = text.clone();
                            is_new = true;

                            let mut updated = false;
                            if state.fifo_lifo_mode != FifoLifoMode::None {
                                if state.fifo_lifo_queue.back() != Some(&text) {
                                    state.fifo_lifo_queue.push_back(text.clone());
                                    updated = true;
                                }
                            }
                            (state.fifo_lifo_mode, updated)
                        } else {
                            (FifoLifoMode::None, false)
                        }
                    } else {
                        (FifoLifoMode::None, false)
                    }
                };

                if is_new {
                    let history_to_save = {
                        let mut state_guard = lock_state();
                        if let Some(state) = &mut *state_guard {
                            let history = std::sync::Arc::make_mut(&mut state.history);
                            if let Some(pos) = history.iter().position(|x| x == &text) {
                                history.remove(pos);
                            }
                            history.push_front(text.clone());
                            let max_history = state::CONFIG.get().map_or(1000, |c| c.max_history);
                            if history.len() > max_history {
                                history.pop_back();
                            }
                            Some(std::sync::Arc::clone(&state.history))
                        } else {
                            None
                        }
                    };
                    if let Some(history_arc) = history_to_save
                        && state::SAVE_HISTORY_TO_FILE.load(std::sync::atomic::Ordering::Relaxed)
                    {
                        if let Some(sender) = state::HISTORY_SAVE_SENDER.get() {
                            let _ = sender.send(history_arc);
                        }
                    }
                }

                if queue_updated {
                    ui::update_tray_tip_and_icon(hwnd);

                    let len = {
                        let state_guard = lock_state();
                        state_guard
                            .as_ref()
                            .map(|s| s.fifo_lifo_queue.len())
                            .unwrap_or(0)
                    };
                    let mode_str = match mode {
                        FifoLifoMode::Fifo => "FIFO",
                        FifoLifoMode::Lifo => "LIFO",
                        _ => "",
                    };

                    fn limit_text(text: &str, limit: usize) -> String {
                        if text.chars().count() > limit {
                            let taken: String = text.chars().take(limit - 3).collect();
                            format!("{}...", taken)
                        } else {
                            text.to_string()
                        }
                    }

                    ui::show_notification(
                        &format!("{} キューに追加", mode_str),
                        &format!(
                            "アイテムが追加されました。現在のキュー: {} 件\n{}",
                            len,
                            limit_text(&text, 40)
                        ),
                        false,
                    );
                }
            }
        }
        win32::WM_DPICHANGED => {
            let is_dark = {
                let state_guard = lock_state();
                state_guard.as_ref().is_some_and(|s| s.is_dark)
            };
            update_theme_resources(hwnd, is_dark);

            let prc = lparam as *const win32::RECT;
            if !prc.is_null() {
                unsafe {
                    let rc = *prc;
                    win32::SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        0x0010 /* SWP_NOZORDER */ | 0x0004, /* SWP_NOACTIVATE */
                    );
                }
            }
        }
        win32::WM_DESTROY => {
            // Clean up font objects (Mutex-backed fonts use .take() to clear)
            if let Some(SafeHFONT(font)) =
                FONT_EDIT.lock().unwrap_or_else(|e| e.into_inner()).take()
            {
                unsafe { win32::DeleteObject(font) };
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(font) };
            }
            if let Some(SafeHFONT(font)) = FONT_LISTBOX_BOLD
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(font) };
            }
            if let Some(SafeHFONT(font)) = state::FONT_ICONS_16
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(font) };
            }
            if let Some(SafeHFONT(font)) = state::FONT_ICONS_18
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(font) };
            }

            // Clean up brush objects
            if let Some(SafeHBRUSH(brush)) =
                BRUSH_BG.lock().unwrap_or_else(|e| e.into_inner()).take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) =
                BRUSH_CTRL.lock().unwrap_or_else(|e| e.into_inner()).take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) =
                BRUSH_EDIT.lock().unwrap_or_else(|e| e.into_inner()).take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_LISTBOX
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_BORDER
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            if let Some(SafeHBRUSH(brush)) = BRUSH_SEL_BG
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                unsafe { win32::DeleteObject(brush) };
            }
            unsafe { win32::PostQuitMessage(0) };
        }
        _ => return unsafe { win32::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub enum IconType {
    Search,
    Folder,
    ParentFolder,
    Snippet,
    History,
    ChevronRight,
    Clear,
}

pub fn draw_vector_icon(
    hdc: win32::HDC,
    icon_type: IconType,
    x: i32,
    y: i32,
    size: i32,
    color: u32,
) {
    let mut drawn_with_font = false;

    unsafe {
        let scale = if let Some(SafeHWND(hwnd_main_ref)) = MAIN_HWND.get() {
            let hwnd_val: win32::HWND = *hwnd_main_ref;
            win32::GetDpiForWindow(hwnd_val) as f32 / 96.0
        } else {
            1.0
        };

        let font_icons_guard = if size as f32 >= 17.5 * scale {
            state::FONT_ICONS_18
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        } else {
            state::FONT_ICONS_16
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        };

        if let Some(SafeHFONT(font)) = font_icons_guard.as_ref()
            && !font.is_null()
        {
            let old_font = win32::SelectObject(hdc, *font as win32::HGDIOBJ);
            if !old_font.is_null() {
                let old_color = win32::SetTextColor(hdc, color);
                let old_mode = win32::SetBkMode(hdc, 1 /* TRANSPARENT */);

                let glyph_char = match icon_type {
                    IconType::Search => 0xE721u16,       // Search (🔍)
                    IconType::Folder => 0xE8D5u16,       // Folder (📁, filled)
                    IconType::ParentFolder => 0xE10Eu16, // FolderParent / Up (↩/⬆)
                    IconType::Snippet => 0xE7C3u16,      // Document (📄)
                    IconType::History => 0xE77Fu16,      // Clipboard (📋)
                    IconType::ChevronRight => 0xE974u16, // ChevronRight (＞)
                    IconType::Clear => 0xE711u16,        // Clear/Cancel (X)
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
                    win32::DT_SINGLELINE
                        | win32::DT_CENTER
                        | win32::DT_VCENTER
                        | win32::DT_NOPREFIX
                        | win32::DT_NOCLIP,
                );

                win32::SelectObject(hdc, old_font);
                win32::SetTextColor(hdc, old_color);
                win32::SetBkMode(hdc, old_mode);
                drawn_with_font = true;
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
                    win32::RoundRect(
                        hdc,
                        x + s(4.0),
                        y + s(9.0),
                        x + s(20.0),
                        y + s(19.0),
                        s(2.0),
                        s(2.0),
                    );
                }
                IconType::ParentFolder => {
                    win32::MoveToEx(hdc, x + s(4.0), y + s(9.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(4.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(9.0), y + s(6.0));
                    win32::LineTo(hdc, x + s(11.0), y + s(9.0));
                    win32::RoundRect(
                        hdc,
                        x + s(4.0),
                        y + s(9.0),
                        x + s(20.0),
                        y + s(19.0),
                        s(2.0),
                        s(2.0),
                    );
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
                    win32::RoundRect(
                        hdc,
                        x + s(5.0),
                        y + s(5.0),
                        x + s(19.0),
                        y + s(22.0),
                        s(3.0),
                        s(3.0),
                    );
                    win32::RoundRect(
                        hdc,
                        x + s(8.0),
                        y + s(2.0),
                        x + s(16.0),
                        y + s(6.0),
                        s(2.0),
                        s(2.0),
                    );
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
                IconType::Clear => {
                    win32::MoveToEx(hdc, x + s(6.0), y + s(6.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(18.0), y + s(18.0));
                    win32::MoveToEx(hdc, x + s(18.0), y + s(6.0), std::ptr::null_mut());
                    win32::LineTo(hdc, x + s(6.0), y + s(18.0));
                }
            }

            win32::SelectObject(hdc, old_pen);
            win32::SelectObject(hdc, old_brush);
            win32::DeleteObject(pen);
        }
    }
}
