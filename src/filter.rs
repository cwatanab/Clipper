use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use rustmigemo::migemo::romaji_processor::RomajiProcessor;

use crate::state::{AppState, Mode};

static ROMAJI_PROCESSOR: OnceLock<RomajiProcessor> = OnceLock::new();

fn get_romaji_processor() -> &'static RomajiProcessor {
    ROMAJI_PROCESSOR.get_or_init(|| RomajiProcessor::new())
}

pub fn filter_items(query_text: &str, state: &AppState, dict_opt: Option<&CompactDictionary>) -> (Vec<String>, Vec<String>) {
    if query_text.is_empty() {
        return match state.mode {
            Mode::Snippet => {
                let mut display_items = Vec::new();
                let mut full_paths = Vec::new();
                let cur_folder = &state.current_folder;

                // Parent folder navigation
                if !cur_folder.is_empty() {
                    display_items.push("📁 ..".to_string());
                    full_paths.push("..".to_string());
                }

                let mut folder_names = std::collections::HashSet::new();
                let mut local_snippets = Vec::new();

                for (name, _) in &state.snippets {
                    if cur_folder.is_empty() {
                        if let Some(pos) = name.find('/') {
                            let folder = &name[..pos];
                            folder_names.insert(folder.to_string());
                        } else {
                            local_snippets.push(name.clone());
                        }
                    } else {
                        let prefix = format!("{}/", cur_folder);
                        if name.starts_with(&prefix) {
                            let sub_name = &name[prefix.len()..];
                            if let Some(pos) = sub_name.find('/') {
                                let folder = &sub_name[..pos];
                                folder_names.insert(folder.to_string());
                            } else {
                                local_snippets.push(name.clone());
                            }
                        }
                    }
                }

                // Add sorted subdirectories
                let mut folders: Vec<String> = folder_names.into_iter().collect();
                folders.sort();
                for f in folders {
                    display_items.push(format!("📁 {}", f));
                    if cur_folder.is_empty() {
                        full_paths.push(format!("dir:{}", f));
                    } else {
                        full_paths.push(format!("dir:{}/{}", cur_folder, f));
                    }
                }

                // Add sorted snippets
                local_snippets.sort();
                for s in local_snippets {
                    let display_name = if let Some(pos) = s.rfind('/') {
                        s[pos + 1..].to_string()
                    } else {
                        s.clone()
                    };
                    display_items.push(display_name);
                    full_paths.push(s);
                }

                (display_items, full_paths)
            }
            Mode::History => {
                let display: Vec<String> = state.history.iter()
                    .map(|s| {
                        s.replace("\r\n", " ")
                         .replace('\n', " ")
                         .replace('\r', " ")
                         .replace('\t', " ")
                    })
                    .collect();
                let full: Vec<String> = state.history.iter().cloned().collect();
                (display, full)
            }
        };
    }

    let romaji_proc = get_romaji_processor();
    let hiragana = romaji_proc.romaji_to_hiragana(query_text);

    let regex_str = if let Some(dict) = dict_opt {
        query(query_text.to_string(), dict, &RegexOperator::Default)
    } else {
        String::new()
    };

    let re_opt = if !regex_str.is_empty() {
        RegexBuilder::new(&regex_str)
            .case_insensitive(true)
            .build()
            .ok()
    } else {
        None
    };

    let katakana: String = hiragana.chars().map(|c| {
        if ('ぁ'..='ん').contains(&c) {
            char::from_u32(c as u32 + 0x60).unwrap_or(c)
        } else {
            c
        }
    }).collect();

    let query_lower = query_text.to_lowercase();
    let hiragana_lower = hiragana.to_lowercase();
    let katakana_lower = katakana.to_lowercase();

    let matches_text = |text: &str| -> bool {
        let text_lower = text.to_lowercase();
        if let Some(ref re) = re_opt {
            if re.is_match(&text_lower) || re.is_match(text) {
                return true;
            }
        }
        if text_lower.contains(&query_lower) {
            return true;
        }
        if !hiragana_lower.is_empty() && hiragana_lower != query_lower {
            if text_lower.contains(&hiragana_lower) || text_lower.contains(&katakana_lower) {
                return true;
            }
        }
        false
    };

    let mut display_items = Vec::new();
    let mut full_paths = Vec::new();

    match state.mode {
        Mode::Snippet => {
            let cur_folder = &state.current_folder;
            let mut folder_names = std::collections::HashSet::new();
            let mut local_snippets = Vec::new();

            for (name, content) in &state.snippets {
                if cur_folder.is_empty() {
                    if let Some(pos) = name.find('/') {
                        let folder = &name[..pos];
                        folder_names.insert(folder.to_string());
                    } else {
                        local_snippets.push((name.clone(), content.clone()));
                    }
                } else {
                    let prefix = format!("{}/", cur_folder);
                    if name.starts_with(&prefix) {
                        let sub_name = &name[prefix.len()..];
                        if let Some(pos) = sub_name.find('/') {
                            let folder = &sub_name[..pos];
                            folder_names.insert(folder.to_string());
                        } else {
                            local_snippets.push((name.clone(), content.clone()));
                        }
                    }
                }
            }

            if !cur_folder.is_empty() {
                display_items.push("📁 ..".to_string());
                full_paths.push("..".to_string());
            }

            let mut folders_matches = Vec::new();
            for f in folder_names {
                if matches_text(&f) {
                    folders_matches.push(f);
                }
            }
            folders_matches.sort();
            for f in folders_matches {
                display_items.push(format!("📁 {}", f));
                if cur_folder.is_empty() {
                    full_paths.push(format!("dir:{}", f));
                } else {
                    full_paths.push(format!("dir:{}/{}", cur_folder, f));
                }
            }

            let mut snippets_matches = Vec::new();
            for (name, content) in local_snippets {
                let display_name = if let Some(pos) = name.rfind('/') {
                    name[pos + 1..].to_string()
                } else {
                    name.clone()
                };
                if matches_text(&display_name) || matches_text(&name) || matches_text(&content) {
                    snippets_matches.push(name);
                }
            }
            snippets_matches.sort();
            for s in snippets_matches {
                let display_name = if let Some(pos) = s.rfind('/') {
                    s[pos + 1..].to_string()
                } else {
                    s.clone()
                };
                display_items.push(display_name);
                full_paths.push(s);
            }
        }
        Mode::History => {
            let mut matches_display = Vec::new();
            let mut matches_full = Vec::new();
            for text in &state.history {
                if matches_text(text) {
                    let clean = text.replace("\r\n", " ")
                                    .replace('\n', " ")
                                    .replace('\r', " ")
                                    .replace('\t', " ");
                    matches_display.push(clean);
                    matches_full.push(text.clone());
                }
            }
            display_items = matches_display;
            full_paths = matches_full;
        }
    }

    (display_items, full_paths)
}
