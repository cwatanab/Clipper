pub mod emulation;
pub mod key_condition;
pub mod key_emulator;
pub mod strategy;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub use key_emulator::{InputBackend, KeyEmulator, ULE4JIS_MAGIC_INFO};

static ENABLED: AtomicBool = AtomicBool::new(false);
static EMULATOR: Mutex<Option<KeyEmulator>> = Mutex::new(None);

pub fn init(enabled: bool, backend: InputBackend) {
    ENABLED.store(enabled, Ordering::Release);
    let mut guard = EMULATOR.lock().unwrap();
    *guard = Some(KeyEmulator::new(backend));
}

#[inline]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        if let Ok(mut guard) = EMULATOR.lock() {
            if let Some(emu) = guard.as_mut() {
                emu.reset_state();
            }
        }
    }
}

pub fn toggle_enabled() -> bool {
    let current = is_enabled();
    let next = !current;
    set_enabled(next);
    next
}

#[allow(dead_code)]
pub fn get_backend() -> InputBackend {
    if let Ok(guard) = EMULATOR.lock() {
        if let Some(emu) = guard.as_ref() {
            return emu.input_backend();
        }
    }
    InputBackend::SendInput
}

#[allow(dead_code)]
pub fn set_backend(backend: InputBackend) {
    if let Ok(mut guard) = EMULATOR.lock() {
        if let Some(emu) = guard.as_mut() {
            emu.set_input_backend(backend);
        }
    }
}

#[inline]
pub fn process_hook_key(vkey: u8, up: bool, dw_extra_info: usize) -> bool {
    if !is_enabled() {
        return false;
    }
    if dw_extra_info == ULE4JIS_MAGIC_INFO {
        return false;
    }
    if let Ok(mut guard) = EMULATOR.lock() {
        if let Some(emu) = guard.as_mut() {
            return emu.process_key_event(vkey, up, dw_extra_info);
        }
    }
    false
}
