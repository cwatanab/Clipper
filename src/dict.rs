use rustmigemo::migemo::compact_dictionary::CompactDictionary;

static DICT_DATA: &[u8] = include_bytes!("../assets/migemo-compact-dict");

pub fn load() -> Option<CompactDictionary> {
    let vec_data = DICT_DATA.to_vec();
    Some(CompactDictionary::new(&vec_data))
}
