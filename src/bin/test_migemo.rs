use rustmigemo::migemo::compact_dictionary::CompactDictionary;
use rustmigemo::migemo::query::query;
use rustmigemo::migemo::regex_generator::RegexOperator;
use regex::Regex;
use std::fs;
use std::path::PathBuf;

fn main() {
    let dict_path = PathBuf::from(
        std::env::var("APPDATA").unwrap_or_default()
    ).join("clipper").join("dict").join("migemo-compact-dict");

    let dict_bytes = fs::read(&dict_path).expect(&format!("Failed to read {:?}", dict_path));
    let dict = CompactDictionary::new(&dict_bytes);

    let test_queries = vec!["genzai", "gen", "nichiji", "genz"];
    
    for q in &test_queries {
        let regex_str = query(q.to_string(), &dict, &RegexOperator::Default);
        let display: String = regex_str.chars().take(200).collect();
        println!("Query: {:?}", q);
        println!("  Regex (first 200 chars): {:?}", display);
        println!("  Regex len: {}", regex_str.len());
        match Regex::new(&regex_str) {
            Ok(re) => {
                let test = "現在日時: 2024/06/28 16:33:20";
                println!("  match {:?}: {}", test, re.is_match(test));
            }
            Err(e) => {
                println!("  Regex INVALID: {}", e);
            }
        }
        println!();
    }
}
