use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rustmigemo::migemo::compact_dictionary_builder;

fn decode_dict(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (decoded, _, _) = encoding_rs::EUC_JP.decode(bytes);
    decoded.into_owned()
}

fn load_dicts(dict_dir: &PathBuf) -> HashMap<String, Vec<String>> {
    let mut combined = HashMap::new();

    if let Ok(entries) = fs::read_dir(dict_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let is_skk = name.to_uppercase().starts_with("SKK-JISYO");
            let is_migemo_dict = name == "migemo-dict";

            if !is_skk && !is_migemo_dict {
                continue;
            }

            if let Ok(bytes) = fs::read(&path) {
                let content = decode_dict(&bytes);
                if content.is_empty() {
                    continue;
                }

                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with(';') {
                        continue;
                    }

                    if is_migemo_dict {
                        if let Some(tab_idx) = line.find('\t') {
                            let reading = line[..tab_idx].to_string();
                            for value in line[tab_idx + 1..].split('\t') {
                                if !value.is_empty() {
                                    combined.entry(reading.clone())
                                        .or_insert_with(Vec::new)
                                        .push(value.to_string());
                                }
                            }
                        }
                    } else if let Some(space_idx) = line.find(' ') {
                        let reading = line[..space_idx].to_string();
                        let values_part = &line[space_idx + 1..];
                        if values_part.starts_with('/') && values_part.ends_with('/') {
                            for value in values_part[1..values_part.len() - 1].split('/') {
                                if !value.is_empty() {
                                    combined.entry(reading.clone())
                                        .or_insert_with(Vec::new)
                                        .push(value.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    combined
}

fn main() {
    let dict_dir = PathBuf::from(
        std::env::var("APPDATA").unwrap_or_default()
    ).join("clipper").join("dict");

    let out_path = dict_dir.join("migemo-compact-dict");

    println!("Loading dictionaries from {:?}...", dict_dir);
    let dict = load_dicts(&dict_dir);
    println!("Loaded {} entries", dict.len());

    println!("Building compact dictionary...");
    let compact = compact_dictionary_builder::build(dict);

    println!("Writing {} bytes to {:?}...", compact.len(), out_path);
    fs::write(&out_path, compact).expect("Failed to write compact dict");

    println!("Done!");
}
