use rustmigemo::migemo::compact_dictionary::CompactDictionary;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_dictionary() {
        let dict1 = get_dictionary();
        assert!(dict1.is_some());
        let dict2 = get_dictionary();
        assert!(dict2.is_some());
        // Verify pointer equality of static reference
        assert!(std::ptr::eq(dict1.unwrap(), dict2.unwrap()));
    }
}

