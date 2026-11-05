use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::regex_generator::{RegexGeneratorTrait, RegexOperator};
use std::sync::OnceLock;

static DICT_DATA: &[u8] = include_bytes!("../assets/migemo-compact-dict");
static DICTIONARY: OnceLock<Option<CompactDictionary>> = OnceLock::new();

pub fn get_dictionary() -> Option<&'static CompactDictionary> {
    DICTIONARY
        .get_or_init(|| {
            let vec_data = DICT_DATA.to_vec();
            Some(CompactDictionary::new(&vec_data))
        })
        .as_ref()
}

struct TrieNode {
    code: char,
    is_terminal: bool,
    children: Vec<TrieNode>,
}

impl TrieNode {
    fn new(code: char) -> Self {
        Self {
            code,
            is_terminal: false,
            children: Vec::new(),
        }
    }
}

pub struct TrieRegexGenerator {
    children: Vec<TrieNode>,
}

impl TrieRegexGenerator {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    fn add_to_nodes(nodes: &mut Vec<TrieNode>, word: &[char]) {
        if word.is_empty() {
            return;
        }
        let c = word[0];
        let pos = match nodes.iter().position(|n| n.code == c) {
            Some(i) => i,
            None => {
                nodes.push(TrieNode::new(c));
                nodes.len() - 1
            }
        };
        if word.len() == 1 {
            nodes[pos].is_terminal = true;
        } else {
            Self::add_to_nodes(&mut nodes[pos].children, &word[1..]);
        }
    }

    fn generate_nodes(nodes: &[TrieNode], buf: &mut String) {
        if nodes.is_empty() {
            return;
        }
        let mut leaves = Vec::new();
        let mut complex = Vec::new();

        for n in nodes {
            if n.children.is_empty() {
                leaves.push(n.code);
            } else {
                complex.push(n);
            }
        }

        let mut branches = Vec::new();

        if !leaves.is_empty() {
            let mut leaf_str = String::new();
            if leaves.len() == 1 {
                let c = leaves[0];
                if r"()[]{}?*+|^$.\".contains(c) {
                    leaf_str.push('\\');
                }
                leaf_str.push(c);
            } else {
                leaf_str.push('[');
                for &c in &leaves {
                    if r"-\]^".contains(c) {
                        leaf_str.push('\\');
                    }
                    leaf_str.push(c);
                }
                leaf_str.push(']');
            }
            branches.push(leaf_str);
        }

        for n in complex {
            let mut s = String::new();
            if r"()[]{}?*+|^$.\".contains(n.code) {
                s.push('\\');
            }
            s.push(n.code);

            let mut child_str = String::new();
            Self::generate_nodes(&n.children, &mut child_str);

            if n.is_terminal {
                s.push_str(&format!("(?:{})?", child_str));
            } else {
                s.push_str(&format!("(?:{})", child_str));
            }
            branches.push(s);
        }

        if branches.len() == 1 {
            buf.push_str(&branches[0]);
        } else {
            buf.push_str(&branches.join("|"));
        }
    }
}

impl RegexGeneratorTrait for TrieRegexGenerator {
    fn add(&mut self, word: &[char]) {
        Self::add_to_nodes(&mut self.children, word);
    }

    fn generate(&self, _op: &RegexOperator) -> String {
        let mut buf = String::new();
        Self::generate_nodes(&self.children, &mut buf);
        if self.children.len() > 1 {
            format!("(?:{})", buf)
        } else {
            buf
        }
    }
}

/// Executes Migemo query with Trie-based prefix/terminal preservation.
/// Single ASCII letters (< 2 chars) and non-alphabetic tokens are matched as literals
/// to avoid generating 100,000+ char regexes and freezing incremental search.
pub fn migemo_query(word: &str, dict: &CompactDictionary) -> String {
    let mut result = String::new();
    for token in rustmigemo::migemo::query::tokenize(word) {
        let is_ascii_alpha = token.chars().all(|c| c.is_ascii_alphabetic());
        if is_ascii_alpha && token.len() >= 2 {
            let mut tg = TrieRegexGenerator::new();
            let q = rustmigemo::migemo::query::query_a_word_with_generator(
                token,
                dict,
                &RegexOperator::Default,
                &mut tg,
            );
            result.push_str(&q);
        } else {
            result.push_str(&regex::escape(token));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::RegexBuilder;

    #[test]
    fn test_get_dictionary() {
        let dict1 = get_dictionary();
        assert!(dict1.is_some());
        let dict2 = get_dictionary();
        assert!(dict2.is_some());
        // Verify pointer equality of static reference
        assert!(std::ptr::eq(dict1.unwrap(), dict2.unwrap()));
    }

    #[test]
    fn test_migemo_query_accuracy() {
        let dict = get_dictionary().expect("dict should load");
        let re_str = migemo_query("nihon", dict);
        let re = RegexBuilder::new(&re_str).case_insensitive(true).build().expect("valid regex");
        assert!(re.is_match("日本"));
        assert!(re.is_match("日本人"));
        assert!(re.is_match("にほん"));
        assert!(re.is_match("nihon"));

        let re_kensaku = migemo_query("kensaku", dict);
        let re2 = RegexBuilder::new(&re_kensaku).case_insensitive(true).build().expect("valid regex");
        assert!(re2.is_match("検索"));

        // Single letter should not match Japanese kanji starting with s
        let re_s = migemo_query("s", dict);
        let re3 = RegexBuilder::new(&re_s).case_insensitive(true).build().expect("valid regex");
        assert!(!re3.is_match("日本"));
        assert!(re3.is_match("sample"));
    }
}
