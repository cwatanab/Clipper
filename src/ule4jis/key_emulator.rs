use std::mem;

use crate::ule4jis::key_condition::KeyCondition;
use crate::ule4jis::strategy::US_ON_JIS_MAP;
use crate::win32::{
    self, GetForegroundWindow, GetKeyboardLayout, GetWindowThreadProcessId, MapVirtualKeyExW,
    SendInput, keybd_event, HKL, INPUT, INPUT_union, KEYBDINPUT, INPUT_KEYBOARD,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC_EX, VK_CANCEL,
    VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT, VK_NEXT,
    VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_SNAPSHOT,
    VK_UP,
};

pub const ULE4JIS_MAGIC_INFO: usize = 0x011E4715;
const MAX_EMULATION_EVENTS: usize = 10;

#[rustfmt::skip]
const EXTENDED_KEYS: [u16; 15] = [
    VK_RCONTROL, VK_RMENU, VK_INSERT, VK_DELETE, VK_HOME, VK_END, VK_PRIOR, VK_NEXT,
    VK_UP, VK_DOWN, VK_RIGHT, VK_LEFT, VK_NUMLOCK, VK_CANCEL, VK_SNAPSHOT,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputBackend {
    SendInput,
    KeybdEvent,
}

impl InputBackend {
    #[allow(dead_code)]
    pub fn toggled(self) -> Self {
        match self {
            Self::SendInput => Self::KeybdEvent,
            Self::KeybdEvent => Self::SendInput,
        }
    }
}

pub struct KeyEmulator {
    key_condition: KeyCondition,
    emulator_id: usize,
    input_backend: InputBackend,
    logical_lshift_released: bool,
    logical_rshift_released: bool,
    logical_lshift_pressed: bool,
    logical_rshift_pressed: bool,
    active_emulation_key: Option<u8>,
}

impl KeyEmulator {
    pub fn new(input_backend: InputBackend) -> Self {
        Self {
            key_condition: KeyCondition::default(),
            emulator_id: ULE4JIS_MAGIC_INFO,
            input_backend,
            logical_lshift_released: false,
            logical_rshift_released: false,
            logical_lshift_pressed: false,
            logical_rshift_pressed: false,
            active_emulation_key: None,
        }
    }

    #[allow(dead_code)]
    pub fn input_backend(&self) -> InputBackend {
        self.input_backend
    }

    #[allow(dead_code)]
    pub fn set_input_backend(&mut self, backend: InputBackend) {
        self.input_backend = backend;
    }

    pub fn reset_state(&mut self) {
        self.key_condition = KeyCondition::default();
        self.logical_lshift_released = false;
        self.logical_rshift_released = false;
        self.logical_lshift_pressed = false;
        self.logical_rshift_pressed = false;
        self.active_emulation_key = None;
    }

    pub fn process_key_event(&mut self, vkey: u8, up: bool, dw_extra_info: usize) -> bool {
        if dw_extra_info == self.emulator_id {
            return false;
        }

        self.key_condition.change_key_state(vkey, up);

        // Reset logical adjustment flags when physical modifier keys are pressed/released
        if vkey == VK_LSHIFT as u8 || vkey == VK_SHIFT as u8 {
            self.logical_lshift_released = false;
            self.logical_lshift_pressed = false;
            self.active_emulation_key = None;
        } else if vkey == VK_RSHIFT as u8 {
            self.logical_rshift_released = false;
            self.logical_rshift_pressed = false;
            self.active_emulation_key = None;
        }

        let cmp = self.key_condition.cmp_value() as usize;
        let mapping = if cmp < US_ON_JIS_MAP.len() {
            US_ON_JIS_MAP[cmp]
        } else {
            None
        };

        // Restore modifiers if another key is pressed/released while modifier adjustments are active
        let mut restoration_events = Vec::new();
        if self.active_emulation_key.is_some_and(|k| k != vkey) {
            restoration_events = self.restore_modifiers();
        }

        if let Some(emulation) = mapping {
            let mut events = restoration_events;
            let key_condition = self.key_condition;
            let mut generated = Vec::new();
            emulation.execute(up, &key_condition, |ev_vkey, ev_key_up| {
                generated.push((ev_vkey, ev_key_up));
            });

            for &(ev_vkey, ev_key_up) in &generated {
                events.push((ev_vkey, ev_key_up));
                self.update_modifier_tracking(ev_vkey, ev_key_up);
            }

            if self.logical_lshift_released
                || self.logical_rshift_released
                || self.logical_lshift_pressed
                || self.logical_rshift_pressed
            {
                self.active_emulation_key = Some(vkey);
            } else {
                self.active_emulation_key = None;
            }

            if !events.is_empty() {
                self.send_events(&events);
            }
            true
        } else if !restoration_events.is_empty() {
            let mut events = restoration_events;
            events.push((vkey, up));
            self.send_events(&events);
            true
        } else {
            false
        }
    }

    fn restore_modifiers(&mut self) -> Vec<(u8, bool)> {
        let mut events = Vec::new();
        if self.logical_lshift_released {
            events.push((VK_LSHIFT as u8, false));
            self.logical_lshift_released = false;
        }
        if self.logical_rshift_released {
            events.push((VK_RSHIFT as u8, false));
            self.logical_rshift_released = false;
        }
        if self.logical_lshift_pressed {
            events.push((VK_LSHIFT as u8, true));
            self.logical_lshift_pressed = false;
        }
        if self.logical_rshift_pressed {
            events.push((VK_RSHIFT as u8, true));
            self.logical_rshift_pressed = false;
        }
        self.active_emulation_key = None;
        events
    }

    fn update_modifier_tracking(&mut self, vkey: u8, key_up: bool) {
        if vkey == VK_LSHIFT as u8 || vkey == VK_SHIFT as u8 {
            let physical_down = self.key_condition.get_mod_key_state(VK_LSHIFT);
            if key_up {
                if physical_down {
                    self.logical_lshift_released = true;
                    self.logical_lshift_pressed = false;
                } else {
                    self.logical_lshift_released = false;
                    self.logical_lshift_pressed = false;
                }
            } else if !physical_down {
                self.logical_lshift_pressed = true;
                self.logical_lshift_released = false;
            } else {
                self.logical_lshift_pressed = false;
                self.logical_lshift_released = false;
            }
        } else if vkey == VK_RSHIFT as u8 {
            let physical_down = self.key_condition.get_mod_key_state(VK_RSHIFT);
            if key_up {
                if physical_down {
                    self.logical_rshift_released = true;
                    self.logical_rshift_pressed = false;
                } else {
                    self.logical_rshift_released = false;
                    self.logical_rshift_pressed = false;
                }
            } else if !physical_down {
                self.logical_rshift_pressed = true;
                self.logical_rshift_released = false;
            } else {
                self.logical_rshift_pressed = false;
                self.logical_rshift_released = false;
            }
        }
    }

    fn send_events(&self, events: &[(u8, bool)]) {
        match self.input_backend {
            InputBackend::SendInput => {
                Self::send_inputs(events, self.emulator_id);
            }
            InputBackend::KeybdEvent => {
                for &(vkey, key_up) in events {
                    Self::send_legacy_input(vkey, key_up, self.emulator_id);
                }
            }
        }
    }

    fn send_inputs(events: &[(u8, bool)], emulator_id: usize) -> bool {
        let layout = Self::foreground_keyboard_layout();
        let mut inputs = [win32::INPUT {
            r#type: INPUT_KEYBOARD,
            u: INPUT_union {
                ki: KEYBDINPUT {
                    w_vk: 0,
                    w_scan: 0,
                    dw_flags: 0,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        }; MAX_EMULATION_EVENTS];

        for (input, &(vkey, key_up)) in inputs.iter_mut().zip(events) {
            *input = Self::input_for_event(vkey, key_up, emulator_id, layout);
        }

        let sent = unsafe {
            SendInput(
                events.len() as u32,
                inputs.as_ptr(),
                mem::size_of::<INPUT>() as i32,
            )
        };
        sent as usize == events.len()
    }

    fn send_legacy_input(vkey: u8, key_up: bool, emulator_id: usize) {
        let mut flags = 0u32;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if Self::is_legacy_extended_key(vkey) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        unsafe {
            keybd_event(vkey, 0, flags, emulator_id);
        }
    }

    fn input_for_event(vkey: u8, key_up: bool, emulator_id: usize, layout: HKL) -> INPUT {
        let (scan_code, scan_is_extended) = Self::map_vkey_to_scan(vkey, layout);

        let mut flags = 0u32;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if scan_code != 0 {
            flags |= KEYEVENTF_SCANCODE;
        }
        if scan_is_extended || Self::is_send_input_extended_key(vkey) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        INPUT {
            r#type: INPUT_KEYBOARD,
            u: INPUT_union {
                ki: KEYBDINPUT {
                    w_vk: if scan_code == 0 { vkey as u16 } else { 0 },
                    w_scan: scan_code,
                    dw_flags: flags,
                    time: 0,
                    dw_extra_info: emulator_id,
                },
            },
        }
    }

    fn foreground_keyboard_layout() -> HKL {
        unsafe {
            let hwnd = GetForegroundWindow();
            let thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
            GetKeyboardLayout(thread_id)
        }
    }

    fn map_vkey_to_scan(vkey: u8, layout: HKL) -> (u16, bool) {
        let mapped = unsafe { MapVirtualKeyExW(vkey as u32, MAPVK_VK_TO_VSC_EX, layout) };
        ((mapped & 0xFF) as u16, (mapped & 0xFF00) != 0)
    }

    const fn extended_key_table(include_rshift: bool) -> [bool; 256] {
        let mut t = [false; 256];
        let mut i = 0;
        while i < EXTENDED_KEYS.len() {
            t[EXTENDED_KEYS[i] as usize] = true;
            i += 1;
        }
        if include_rshift {
            t[VK_RSHIFT as usize] = true;
        }
        t[VK_DIVIDE as usize] = true;
        t
    }

    const SEND_INPUT_EXTENDED_KEY_TABLE: [bool; 256] = Self::extended_key_table(false);
    const LEGACY_EXTENDED_KEY_TABLE: [bool; 256] = Self::extended_key_table(true);

    fn is_send_input_extended_key(vkey: u8) -> bool {
        Self::SEND_INPUT_EXTENDED_KEY_TABLE[vkey as usize]
    }

    fn is_legacy_extended_key(vkey: u8) -> bool {
        Self::LEGACY_EXTENDED_KEY_TABLE[vkey as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_tracking_and_restoration() {
        let mut emulator = KeyEmulator::new(InputBackend::SendInput);

        // 1. Simulate physical shift held down
        emulator
            .key_condition
            .change_key_state(VK_LSHIFT as u8, false);

        // 2. Simulate mapping where LSHIFT was logically released (e.g. from ShiftRelease)
        emulator.update_modifier_tracking(VK_LSHIFT as u8, true);
        assert!(emulator.logical_lshift_released);
        assert!(!emulator.logical_lshift_pressed);

        // 3. Verify restoration logic generates the correct event (press LSHIFT down) and clears state
        emulator.active_emulation_key = Some(b'2');
        let events = emulator.restore_modifiers();
        assert_eq!(events, vec![(VK_LSHIFT as u8, false)]);
        assert!(!emulator.logical_lshift_released);
        assert!(emulator.active_emulation_key.is_none());
    }
}
