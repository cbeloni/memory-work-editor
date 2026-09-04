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

/// Generates a filename like `archive-a3f9c1-20260528-091500.txt`.
pub fn generate_archive_filename() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 3] = rng.gen();
    let hash6 = format!("{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2]);
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S");
    format!("archive-{}-{}.txt", hash6, timestamp)
}

pub fn is_tab_filename(filename: &str) -> bool {
    filename.starts_with("tab-") && filename.ends_with(".txt")
}

pub fn is_archive_filename(filename: &str) -> bool {
    filename.starts_with("archive-") && filename.ends_with(".txt")
}

pub fn tab_to_archive_filename(tab_filename: &str) -> String {
    if let Some(rest) = tab_filename.strip_prefix("tab-") {
        format!("archive-{}", rest)
    } else {
        generate_archive_filename()
    }
}

pub fn archive_to_tab_filename(archive_filename: &str) -> String {
    if let Some(rest) = archive_filename.strip_prefix("archive-") {
        format!("tab-{}", rest)
    } else {
        generate_tab_filename()
    }
}

/// Extracts the sortable timestamp portion from a filename.
/// `tab-a3f9c1-20260528-091500.txt` or `archive-a3f9c1-20260528-091500.txt` → `"20260528-091500"`
pub fn filename_timestamp(filename: &str) -> &str {
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

/// Formats a raw timestamp `20260528-091500` to `28/05/2026 09:15:00`
pub fn format_timestamp_display(raw: &str) -> String {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y%m%d-%H%M%S") {
        dt.format("%d/%m/%Y %H:%M:%S").to_string()
    } else {
        raw.to_string()
    }
}

