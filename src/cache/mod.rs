pub mod naming;
pub mod writer;

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub use naming::{cache_dir, generate_tab_filename};
pub use writer::CacheWriter;

pub fn ensure_cache_dir() -> Result<()> {
    let dir = cache_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Creates an empty cache file for a new tab and returns its path.
pub fn create_tab_file() -> Result<PathBuf> {
    ensure_cache_dir()?;
    let path = cache_dir().join(generate_tab_filename());
    fs::write(&path, b"")?;
    Ok(path)
}

/// Scans the cache directory and returns (path, content) pairs sorted by creation timestamp.
pub fn scan_cache() -> Result<Vec<(PathBuf, String)>> {
    let dir = cache_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries: Vec<(PathBuf, String)> = vec![];
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("txt") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            entries.push((path, content));
        }
    }

    // Sort by the timestamp portion of the filename (positions [2]-[3] when split by '-').
    entries.sort_by(|(a, _), (b, _)| {
        let ts_a = a
            .file_name()
            .and_then(|n| n.to_str())
            .map(naming::filename_timestamp)
            .unwrap_or("");
        let ts_b = b
            .file_name()
            .and_then(|n| n.to_str())
            .map(naming::filename_timestamp)
            .unwrap_or("");
        ts_a.cmp(ts_b)
    });

    Ok(entries)
}
