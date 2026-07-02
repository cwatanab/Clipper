use std::sync::OnceLock;

use regex::RegexBuilder;
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use rustmigemo::migemo::romaji_processor::RomajiProcessor;

use crate::state::{AppState, Mode};

static ROMAJI_PROCESSOR: OnceLock<RomajiProcessor> = OnceLock::new();

fn get_romaji_processor() -> &'static RomajiProcessor {
    ROMAJI_PROCESSOR.get_or_init(RomajiProcessor::new)
}

pub fn filter_items(
    query_text: &str,
    state: &AppState,
    dict_opt: Option<&CompactDictionary>,
) -> (Vec<String>, Vec<String>) {
    if query_text.is_empty() {
        return match state.mode {
            Mode::Snippet => {
                let mut display_items = Vec::new();
                let mut full_paths = Vec::new();
                let cur_folder = &state.current_folder;

                // Parent folder navigation
                if !cur_folder.is_empty() {
                    display_items.push(format!("[DIR] .. / {}", cur_folder));
                    full_paths.push("..".to_string());
                }

                let cur_folder_parts = if cur_folder.is_empty() {
                    Vec::new()
                } else {
                    crate::util::split_path(cur_folder)
                };

                let mut folder_names = std::collections::HashSet::new();
                let mut local_snippets = Vec::new();

                for (name, _) in state.snippets.iter() {
                    let snippet_parts = crate::util::split_path(name);
                    if snippet_parts.starts_with(&cur_folder_parts) {
                        let n = cur_folder_parts.len();
                        let m = snippet_parts.len();
                        if m == n + 1 {
                            local_snippets.push((name.as_str(), snippet_parts[n].clone()));
                        } else if m > n + 1 {
                            folder_names.insert(snippet_parts[n].clone());
                        }
                    }
                }

                // Add sorted subdirectories
                let mut folders: Vec<String> = folder_names.into_iter().collect();
                folders.sort();
                for f in folders {
                    display_items.push(format!("[DIR] {}", f));
                    if cur_folder.is_empty() {
                        let escaped = f.replace('/', "\\/");
                        full_paths.push(format!("dir:{}", escaped));
                    } else {
                        let escaped = f.replace('/', "\\/");
                        full_paths.push(format!("dir:{}/{}", cur_folder, escaped));
                    }
                }

                // Add sorted snippets
                local_snippets.sort_by(|a, b| a.1.cmp(&b.1));
                for (full_path, display_name) in local_snippets {
                    display_items.push(format!("[SNIP] {}", display_name));
                    full_paths.push(full_path.to_string());
                }

                (display_items, full_paths)
            }
            Mode::History => {
                let display: Vec<String> = state
                    .history
                    .iter()
                    .map(|s| clean_history_item(s))
                    .collect();
                let full: Vec<String> = state.history.iter().cloned().collect();
                (display, full)
            }
        };
    }

    let mut regex_parts = Vec::new();

    // 1. Add Migemo query regex if dictionary is available
    if let Some(dict) = dict_opt {
        let migemo_re = query(query_text.to_string(), dict, &RegexOperator::Default);
        if !migemo_re.is_empty() {
            regex_parts.push(migemo_re);
        }
    }

    // 2. Add literal query escaped
    regex_parts.push(regex::escape(query_text));

    // 3. Add Hiragana/Katakana escaped if applicable
    let romaji_proc = get_romaji_processor();
    let hiragana = romaji_proc.romaji_to_hiragana(query_text);
    let check_hira = !hiragana.is_empty() && hiragana != query_text;
    if check_hira {
        regex_parts.push(regex::escape(&hiragana));

        let katakana: String = hiragana
            .chars()
            .map(|c| {
                if ('ぁ'..='ん').contains(&c) {
                    char::from_u32(c as u32 + 0x60).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect();
        if !katakana.is_empty() && katakana != hiragana {
            regex_parts.push(regex::escape(&katakana));
        }
    }

    // Combine them with OR operator
    let combined_pattern = regex_parts.join("|");

    let re_opt = RegexBuilder::new(&combined_pattern)
        .case_insensitive(true)
        .build()
        .ok();

    let query_lower = query_text.to_lowercase();

    let matches_text = |text: &str| -> bool {
        if let Some(ref re) = re_opt {
            re.is_match(text)
        } else {
            // Fallback case-insensitive literal check if regex fails
            text.to_lowercase().contains(&query_lower)
        }
    };

    let mut display_items = Vec::new();
    let mut full_paths = Vec::new();

    match state.mode {
        Mode::Snippet => {
            let cur_folder = &state.current_folder;
            let cur_folder_parts = if cur_folder.is_empty() {
                Vec::new()
            } else {
                crate::util::split_path(cur_folder)
            };

            let mut folder_names = std::collections::HashSet::new();
            let mut local_snippets = Vec::new();

            for (name, content) in state.snippets.iter() {
                let snippet_parts = crate::util::split_path(name);
                if snippet_parts.starts_with(&cur_folder_parts) {
                    let n = cur_folder_parts.len();
                    let m = snippet_parts.len();
                    if m == n + 1 {
                        local_snippets.push((
                            name.as_str(),
                            snippet_parts[n].clone(),
                            content.as_str(),
                        ));
                    } else if m > n + 1 {
                        folder_names.insert(snippet_parts[n].clone());
                    }
                }
            }

            if !cur_folder.is_empty() {
                display_items.push(format!("[DIR] .. / {}", cur_folder));
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
                display_items.push(format!("[DIR] {}", f));
                if cur_folder.is_empty() {
                    let escaped = f.replace('/', "\\/");
                    full_paths.push(format!("dir:{}", escaped));
                } else {
                    let escaped = f.replace('/', "\\/");
                    full_paths.push(format!("dir:{}/{}", cur_folder, escaped));
                }
            }

            let mut snippets_matches = Vec::new();
            for (name, display_name, content) in local_snippets {
                if matches_text(&display_name) || matches_text(name) || matches_text(content) {
                    snippets_matches.push((name, display_name));
                }
            }
            snippets_matches.sort_by(|a, b| a.1.cmp(&b.1));
            for (full_path, display_name) in snippets_matches {
                display_items.push(format!("[SNIP] {}", display_name));
                full_paths.push(full_path.to_string());
            }
        }
        Mode::History => {
            let mut matches_display = Vec::new();
            let mut matches_full = Vec::new();
            for text in state.history.iter() {
                if matches_text(text) {
                    matches_display.push(clean_history_item(text));
                    matches_full.push(text.clone());
                }
            }
            display_items = matches_display;
            full_paths = matches_full;
        }
    }

    (display_items, full_paths)
}

fn clean_history_item(s: &str) -> String {
    let has_control = s
        .as_bytes()
        .iter()
        .any(|&b| b == b'\r' || b == b'\n' || b == b'\t');
    if !has_control {
        let mut clean = String::with_capacity("[HIST] ".len() + s.len());
        clean.push_str("[HIST] ");
        clean.push_str(s);
        return clean;
    }

    let mut clean = String::with_capacity("[HIST] ".len() + s.len());
    clean.push_str("[HIST] ");
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            chars.next(); // consume \n
            clean.push(' ');
        } else if c == '\r' || c == '\n' || c == '\t' {
            clean.push(' ');
        } else {
            clean.push(c);
        }
    }
    clean
}
