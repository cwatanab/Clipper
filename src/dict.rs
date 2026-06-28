use std::fs;
use std::path::PathBuf;

use rustmigemo::migemo::compact_dictionary::CompactDictionary;

use crate::state;

pub fn load() -> Option<CompactDictionary> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for candidate in &[
                exe_dir.join("migemo-compact-dict"),
                exe_dir.join("assets").join("migemo-compact-dict"),
            ] {
                if let Ok(bytes) = fs::read(candidate) {
                    state::log_debug(&format!("Loaded migemo dict (exe dir): {:?}", candidate));
                    return Some(CompactDictionary::new(&bytes));
                }
            }
        }
    }

    let appdata_path = PathBuf::from(
        std::env::var("APPDATA").unwrap_or_default()
    ).join("clipper").join("dict").join("migemo-compact-dict");

    if let Ok(bytes) = fs::read(&appdata_path) {
        state::log_debug(&format!("Loaded migemo dict (appdata): {:?}", appdata_path));
        return Some(CompactDictionary::new(&bytes));
    }

    state::log_debug("migemo-compact-dict not found in exe dir or appdata");
    None
}
