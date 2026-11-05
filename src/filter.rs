use std::sync::{Mutex, OnceLock};

use regex::{Regex, RegexBuilder};
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::romaji_processor::RomajiProcessor;

use crate::state::{AppState, Mode};

static ROMAJI_PROCESSOR: OnceLock<RomajiProcessor> = OnceLock::new();

fn get_romaji_processor() -> &'static RomajiProcessor {
    ROMAJI_PROCESSOR.get_or_init(RomajiProcessor::new)
}

struct CachedRegex {
    query: String,
    has_dict: bool,
    regex: Option<Regex>,
}

static REGEX_CACHE: Mutex<Vec<CachedRegex>> = Mutex::new(Vec::new());

fn get_compiled_regex_for_word(word: &str, dict_opt: Option<&CompactDictionary>) -> Option<Regex> {
    let word_lower = word.to_lowercase();
    let has_dict = dict_opt.is_some();
    {
        let mut cache = REGEX_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = cache
            .iter()
            .position(|c| c.query == word_lower && c.has_dict == has_dict)
        {
            let entry = cache.remove(pos);
            let regex = entry.regex.clone();
            cache.insert(0, entry); // Move to MRU position
            return regex;
        }
    }

    let mut regex_parts = Vec::new();

    // 1. Add Migemo query regex if dictionary is available
    if let Some(dict) = dict_opt {
        let migemo_re = crate::dict::migemo_query(&word_lower, dict);
        if !migemo_re.is_empty() {
            regex_parts.push(migemo_re);
        }
    }

    // 2. Add literal query escaped
    regex_parts.push(regex::escape(&word_lower));

    // 3. Add full-width version if different
    let fullwidth = crate::util::to_fullwidth(&word_lower);
    if fullwidth != word_lower {
        regex_parts.push(regex::escape(&fullwidth));
    }

    // 4. Add Hiragana/Katakana escaped if applicable
    let romaji_proc = get_romaji_processor();
    let hiragana = romaji_proc.romaji_to_hiragana(&word_lower);
    let check_hira = !hiragana.is_empty() && hiragana != word_lower;
    if check_hira {
        regex_parts.push(regex::escape(&hiragana));

        let katakana = crate::util::to_katakana(&hiragana);
        if !katakana.is_empty() && katakana != hiragana {
            regex_parts.push(regex::escape(&katakana));
        }
    }

    // Combine them with OR operator
    let combined_pattern = regex_parts.join("|");

    let regex = RegexBuilder::new(&combined_pattern)
        .case_insensitive(true)
        .build()
        .ok()
        .or_else(|| {
            RegexBuilder::new(&regex::escape(&word_lower))
                .case_insensitive(true)
                .build()
                .ok()
        });

    let mut cache = REGEX_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.insert(
        0,
        CachedRegex {
            query: word_lower,
            has_dict,
            regex: regex.clone(),
        },
    );
    if cache.len() > 32 {
        cache.pop();
    }

    regex
}

pub fn filter_items(
    query_text: &str,
    state: &AppState,
    dict_opt: Option<&CompactDictionary>,
) -> (Vec<String>, Vec<String>) {
    let normalized_query = crate::util::to_halfwidth(query_text);
    let words: Vec<&str> = normalized_query.split_whitespace().collect();

    if words.is_empty() {
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

                let sort_snippets = crate::state::CONFIG
                    .get()
                    .is_some_and(|c| c.sort_snippets);

                // Add subdirectories (always sorted)
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

                // Add snippets (conditionally sorted)
                if sort_snippets {
                    local_snippets.sort_by(|a, b| a.1.cmp(&b.1));
                }
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

    let word_matchers: Vec<(Option<Regex>, String)> = words
        .iter()
        .map(|w| {
            let lower = w.to_lowercase();
            let re = get_compiled_regex_for_word(w, dict_opt);
            (re, lower)
        })
        .collect();

    let matches_all = |text: &str| -> bool {
        for (re_opt, lower) in &word_matchers {
            let matched = if let Some(re) = re_opt {
                re.is_match(text)
            } else {
                text.to_lowercase().contains(lower)
            };
            if !matched {
                return false;
            }
        }
        true
    };

    let matches_snippet = |name: &str, content: &str| -> bool {
        for (re_opt, lower) in &word_matchers {
            let matched = if let Some(re) = re_opt {
                re.is_match(name) || re.is_match(content)
            } else {
                name.to_lowercase().contains(lower) || content.to_lowercase().contains(lower)
            };
            if !matched {
                return false;
            }
        }
        true
    };

    let mut display_items = Vec::new();
    let mut full_paths = Vec::new();

    match state.mode {
        Mode::Snippet => {
            let sort_snippets = crate::state::CONFIG
                .get()
                .is_some_and(|c| c.sort_snippets);
            let cur_folder = &state.current_folder;

            let mut matches = Vec::new();

            for (name, content) in state.snippets.iter() {
                if matches_snippet(name, content) {
                    let in_cur_folder = !cur_folder.is_empty()
                        && (name == cur_folder || name.starts_with(&format!("{}/", cur_folder)));
                    matches.push((name.as_str(), in_cur_folder));
                }
            }

            if sort_snippets {
                // If in a subfolder, prioritize current folder items, then sort by name
                matches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            } else if !cur_folder.is_empty() {
                // Keep file definition order, but prioritize current folder
                matches.sort_by(|a, b| b.1.cmp(&a.1));
            }

            for (name, _) in matches {
                display_items.push(format!("[SNIP] {}", name));
                full_paths.push(name.to_string());
            }
        }
        Mode::History => {
            let mut matches_display = Vec::new();
            let mut matches_full = Vec::new();
            for text in state.history.iter() {
                if matches_all(text) {
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
    const MAX_LEN: usize = 200;

    let has_control = s
        .as_bytes()
        .iter()
        .any(|&b| b == b'\r' || b == b'\n' || b == b'\t');

    if !has_control && s.chars().take(MAX_LEN + 1).count() <= MAX_LEN {
        let mut clean = String::with_capacity("[HIST] ".len() + s.len());
        clean.push_str("[HIST] ");
        clean.push_str(s);
        return clean;
    }

    let mut clean = String::with_capacity("[HIST] ".len() + MAX_LEN + 3);
    clean.push_str("[HIST] ");
    let mut chars = s.chars().peekable();
    let mut count = 0;
    let mut truncated = false;

    while let Some(c) = chars.next() {
        if count >= MAX_LEN {
            truncated = true;
            break;
        }
        if c == '\r' && chars.peek() == Some(&'\n') {
            chars.next(); // consume \n
            clean.push(' ');
        } else if c == '\r' || c == '\n' || c == '\t' {
            clean.push(' ');
        } else {
            clean.push(c);
        }
        count += 1;
    }

    if truncated {
        clean.push_str("...");
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_items_natural_order() {
        let state = AppState {
            history: std::sync::Arc::new(std::collections::VecDeque::new()),
            snippets: std::sync::Arc::new(vec![
                ("zebra".to_string(), "Zebra".to_string()),
                ("apple".to_string(), "Apple".to_string()),
            ]),
            mode: Mode::Snippet,
            visible: false,
            current_results: Vec::new(),
            current_full_paths: Vec::new(),
            last_clipboard_value: String::new(),
            current_selection: String::new(),
            last_active_window: None,
            is_dark: false,
            current_folder: String::new(),
            top_index: 0,
            filter_generation: 0,
            fifo_lifo_mode: crate::state::FifoLifoMode::None,
            fifo_lifo_queue: std::collections::VecDeque::new(),
        };

        let (display_items, _) = filter_items("", &state, None);

        let zebra_idx = display_items
            .iter()
            .position(|x| x == "[SNIP] zebra" || x == "[SNIP] Zebra");
        let apple_idx = display_items
            .iter()
            .position(|x| x == "[SNIP] apple" || x == "[SNIP] Apple");

        assert!(
            zebra_idx.is_some(),
            "Zebra/zebra should be in display items"
        );
        assert!(
            apple_idx.is_some(),
            "Apple/apple should be in display items"
        );
        assert!(
            zebra_idx.unwrap() < apple_idx.unwrap(),
            "Zebra/zebra should come before Apple/apple in natural order"
        );
    }

    #[test]
    fn test_filter_subfolder_snippets() {
        let state = AppState {
            history: std::sync::Arc::new(std::collections::VecDeque::new()),
            snippets: std::sync::Arc::new(vec![
                ("git/commit".to_string(), "feat: {{ input }}".to_string()),
                ("markdown/link".to_string(), "[{{ input }}]()".to_string()),
                ("sign".to_string(), "--\ncwatanab".to_string()),
            ]),
            mode: Mode::Snippet,
            visible: true,
            current_results: Vec::new(),
            current_full_paths: Vec::new(),
            last_clipboard_value: String::new(),
            current_selection: String::new(),
            last_active_window: None,
            is_dark: false,
            current_folder: String::new(),
            top_index: 0,
            filter_generation: 0,
            fifo_lifo_mode: crate::state::FifoLifoMode::None,
            fifo_lifo_queue: std::collections::VecDeque::new(),
        };

        // Searching "commit" should find git/commit even from root folder
        let (display, full) = filter_items("commit", &state, None);
        assert_eq!(display.len(), 1);
        assert_eq!(display[0], "[SNIP] git/commit");
        assert_eq!(full[0], "git/commit");

        // Searching content "cwatanab" should find sign
        let (display, full) = filter_items("cwatanab", &state, None);
        assert_eq!(display.len(), 1);
        assert_eq!(display[0], "[SNIP] sign");
        assert_eq!(full[0], "sign");
    }

    #[test]
    fn test_filter_multiword_and_search() {
        let mut history = std::collections::VecDeque::new();
        history.push_back("git checkout master && git commit -m 'fix'".to_string());
        history.push_back("cargo check --release".to_string());
        history.push_back("git push origin main".to_string());

        let state = AppState {
            history: std::sync::Arc::new(history),
            snippets: std::sync::Arc::new(Vec::new()),
            mode: Mode::History,
            visible: true,
            current_results: Vec::new(),
            current_full_paths: Vec::new(),
            last_clipboard_value: String::new(),
            current_selection: String::new(),
            last_active_window: None,
            is_dark: false,
            current_folder: String::new(),
            top_index: 0,
            filter_generation: 0,
            fifo_lifo_mode: crate::state::FifoLifoMode::None,
            fifo_lifo_queue: std::collections::VecDeque::new(),
        };

        // AND search with "git commit"
        let (display, full) = filter_items("git commit", &state, None);
        assert_eq!(display.len(), 1);
        assert!(full[0].contains("git commit"));

        // Reversed order "commit git" should still match
        let (display, full) = filter_items("commit git", &state, None);
        assert_eq!(display.len(), 1);
        assert!(full[0].contains("git commit"));

        // Full-width space "git　commit" should also work
        let (display, full) = filter_items("git　commit", &state, None);
        assert_eq!(display.len(), 1);
        assert!(full[0].contains("git commit"));
    }

    #[test]
    fn test_filter_uppercase_romaji() {
        let mut history = std::collections::VecDeque::new();
        history.push_back("これはテストです".to_string());
        history.push_back("sample code".to_string());

        let state = AppState {
            history: std::sync::Arc::new(history),
            snippets: std::sync::Arc::new(Vec::new()),
            mode: Mode::History,
            visible: true,
            current_results: Vec::new(),
            current_full_paths: Vec::new(),
            last_clipboard_value: String::new(),
            current_selection: String::new(),
            last_active_window: None,
            is_dark: false,
            current_folder: String::new(),
            top_index: 0,
            filter_generation: 0,
            fifo_lifo_mode: crate::state::FifoLifoMode::None,
            fifo_lifo_queue: std::collections::VecDeque::new(),
        };

        // Uppercase romaji "TESUTO" should convert to hiragana/katakana and match "テスト"
        let (display, _) = filter_items("TESUTO", &state, None);
        assert_eq!(display.len(), 1);
        assert!(display[0].contains("これはテストです"));
    }
}
