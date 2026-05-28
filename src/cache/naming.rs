use chrono::Local;
use rand::Rng;
use std::path::PathBuf;

pub fn cache_dir() -> PathBuf {
    PathBuf::from(".memory-work-cache")
}

/// Generates a filename like `tab-a3f9c1-20260528-091500.txt`.
pub fn generate_tab_filename() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 3] = rng.gen();
    let hash6 = format!("{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2]);
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S");
    format!("tab-{}-{}.txt", hash6, timestamp)
}

/// Extracts the sortable timestamp portion from a filename.
/// `tab-a3f9c1-20260528-091500.txt` → `"20260528-091500"`
pub fn filename_timestamp(filename: &str) -> &str {
    // parts: ["tab", "a3f9c1", "20260528", "091500.txt"]
    let without_ext = filename.trim_end_matches(".txt");
    // find the index after the second '-'
    let mut dashes = 0;
    for (i, c) in without_ext.char_indices() {
        if c == '-' {
            dashes += 1;
            if dashes == 2 {
                return &without_ext[i + 1..];
            }
        }
    }
    filename
}
