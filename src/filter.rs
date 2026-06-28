use regex::Regex;
use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use rustmigemo::migemo::romaji_processor::RomajiProcessor;

use crate::state::{AppState, Mode};

pub fn filter_items(query_text: &str, state: &AppState, dict_opt: Option<&CompactDictionary>) -> Vec<String> {
    if query_text.is_empty() {
        return match state.mode {
            Mode::Snippet => state.snippets.iter().map(|(name, _)| name.clone()).collect(),
            Mode::History => state.history.iter().cloned().collect(),
        };
    }

    let romaji_proc = RomajiProcessor::new();
    let hiragana = romaji_proc.romaji_to_hiragana(query_text);

    let regex_str = if let Some(dict) = dict_opt {
        query(query_text.to_string(), dict, &RegexOperator::Default)
    } else {
        String::new()
    };

    let re_opt = Regex::new(&regex_str).ok();

    let katakana: String = hiragana.chars().map(|c| {
        if ('ぁ'..='ん').contains(&c) {
            char::from_u32(c as u32 + 0x60).unwrap_or(c)
        } else {
            c
        }
    }).collect();

    let matches_text = |text: &str| -> bool {
        if let Some(ref re) = re_opt {
            if re.is_match(text) {
                return true;
            }
        }
        if !hiragana.is_empty() && hiragana != query_text {
            if text.contains(&hiragana) || text.contains(&katakana) {
                return true;
            }
        }
        false
    };

    match state.mode {
        Mode::Snippet => {
            state.snippets.iter()
                .filter(|(name, content)| matches_text(name) || matches_text(content))
                .map(|(name, _)| name.clone())
                .collect()
        }
        Mode::History => {
            state.history.iter()
                .filter(|text| matches_text(text))
                .cloned()
                .collect()
        }
    }
}
