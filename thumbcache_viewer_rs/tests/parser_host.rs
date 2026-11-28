// Host-side check of the thumbcache parser: builds a small database in the
// temp directory and verifies the entries it produces.
//
// Run with:  rustc --edition 2021 -O tests/parser_host.rs -o /tmp/parser_host
#[path = "../src/thumbcache.rs"]
mod thumbcache;

fn main() {
    let dir = std::env::temp_dir().join("tcv_parser_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("thumbcache_256.db");

    let mut d: Vec<u8> = Vec::new();
    d.extend_from_slice(b"CMMM");
    d.extend_from_slice(&0x20u32.to_le_bytes()); // Windows 10
    d.extend_from_slice(&4u32.to_le_bytes()); // type
    d.resize(24, 0);

    let name: Vec<u16> = "testfile".encode_utf16().collect();
    let name_bytes: Vec<u8> = name.iter().flat_map(|c| c.to_le_bytes()).collect();
    let data: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];
    let entry_size = 56u32 + name_bytes.len() as u32 + data.len() as u32;

    let entry_start = d.len();
    d.extend_from_slice(b"CMMM");
    d.extend_from_slice(&entry_size.to_le_bytes());
    d.extend_from_slice(&0x1122334455667788u64.to_le_bytes());
    d.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes()); // padding
    d.extend_from_slice(&(data.len() as u32).to_le_bytes());
    d.extend_from_slice(&16u32.to_le_bytes()); // width
    d.extend_from_slice(&16u32.to_le_bytes()); // height
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0xAAAAu64.to_le_bytes());
    d.extend_from_slice(&0xBBBBu64.to_le_bytes());
    assert_eq!(d.len() - entry_start, 56, "entry header size");
    d.extend_from_slice(&name_bytes);
    d.extend_from_slice(&data);

    // A second, blank entry (no image data).
    let name2: Vec<u16> = "blank".encode_utf16().collect();
    let name2_bytes: Vec<u8> = name2.iter().flat_map(|c| c.to_le_bytes()).collect();
    let entry2_size = 56u32 + name2_bytes.len() as u32;
    d.extend_from_slice(b"CMMM");
    d.extend_from_slice(&entry2_size.to_le_bytes());
    d.extend_from_slice(&0x99u64.to_le_bytes());
    d.extend_from_slice(&(name2_bytes.len() as u32).to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes()); // data_size = 0
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u64.to_le_bytes());
    d.extend_from_slice(&0u64.to_le_bytes());
    d.extend_from_slice(&name2_bytes);

    std::fs::write(&path, &d).unwrap();

    let entries = thumbcache::read_database(&path, false).expect("parse");
    assert_eq!(entries.len(), 2, "two entries");
    let e = &entries[0];
    assert_eq!(e.filename, "testfile.png");
    assert_eq!(e.entry_hash, 0x1122334455667788);
    assert_eq!(e.header_offset as usize, entry_start);
    assert_eq!(
        e.data_offset as usize,
        entry_start + 56 + name_bytes.len(),
        "data offset"
    );
    assert_eq!(e.data_size as usize, data.len());
    assert_eq!(e.data().unwrap(), data);
    assert_eq!(e.flag, thumbcache::FIF_TYPE_PNG);
    assert_eq!(e.system_name(), "Windows 10");
    assert_eq!(e.width, 16);
    assert_eq!(e.height, 16);

    let e2 = &entries[1];
    assert_eq!(e2.filename, "blank");
    assert_eq!(e2.data_size, 0);

    let filtered = thumbcache::read_database(&path, true).expect("parse filtered");
    assert_eq!(filtered.len(), 1, "blank entries hidden");

    // Not a thumbcache database.
    let bad = dir.join("bad.db");
    std::fs::write(&bad, b"not a database at all").unwrap();
    assert!(thumbcache::read_database(&bad, false).is_err());

    println!("parser checks passed: 2 entries, 1 after hiding blanks, PNG detected at offset {}", e.data_offset);
}
