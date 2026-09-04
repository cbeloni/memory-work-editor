pub mod naming;
pub mod writer;

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub use naming::*;
pub use writer::CacheWriter;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ArchivedNote {
    pub path: PathBuf,
    pub filename: String,
    pub title: String,
    pub preview: String,
    pub timestamp_raw: String,
    pub timestamp_display: String,
    pub char_count: usize,
    pub line_count: usize,
}

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

/// Scans the cache directory and returns (path, content) pairs of active tabs, sorted by creation timestamp.
/// Archived files (`archive-*.txt`) are explicitly ignored so they are not loaded on application startup.
pub fn scan_cache() -> Result<Vec<(PathBuf, String)>> {
    let dir = cache_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries: Vec<(PathBuf, String)> = vec![];
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if naming::is_tab_filename(&name) {
            let path = entry.path();
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

/// Scans the cache directory for archived notes (`archive-*.txt`), sorted newest first.
pub fn scan_archives() -> Result<Vec<ArchivedNote>> {
    let dir = cache_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut notes = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if naming::is_archive_filename(&name) {
            let path = entry.path();
            let content = fs::read_to_string(&path).unwrap_or_default();
            let ts_raw = naming::filename_timestamp(&name).to_string();
            let ts_disp = naming::format_timestamp_display(&ts_raw);
            let char_count = content.chars().count();
            let line_count = content.lines().count().max(1);

            let first_line = content
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string());

            let hash = name
                .strip_prefix("archive-")
                .and_then(|s| s.split('-').next())
                .unwrap_or("nota")
                .to_string();

            let title = first_line.unwrap_or_else(|| format!("Nota {}", hash));
            
            // First 3 non-empty lines for preview
            let preview = content
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join("\n");

            notes.push(ArchivedNote {
                path,
                filename: name,
                title,
                preview,
                timestamp_raw: ts_raw,
                timestamp_display: ts_disp,
                char_count,
                line_count,
            });
        }
    }

    // Sort newest first
    notes.sort_by(|a, b| b.timestamp_raw.cmp(&a.timestamp_raw));

    Ok(notes)
}

/// Archives a tab: saves content into `archive-...txt` and deletes the active `tab-...txt` file.
pub fn archive_tab(tab_path: &PathBuf, content: &str) -> Result<PathBuf> {
    ensure_cache_dir()?;
    let tab_filename = tab_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let archive_filename = naming::tab_to_archive_filename(tab_filename);
    let archive_path = cache_dir().join(archive_filename);

    let tmp = archive_path.with_extension("tmp");
    fs::write(&tmp, content.as_bytes())?;
    fs::rename(&tmp, &archive_path)?;

    if tab_path.exists() {
        let _ = fs::remove_file(tab_path);
    }

    Ok(archive_path)
}

/// Unarchives a note: reads content from `archive-...txt`, creates active `tab-...txt`,
/// removes `archive-...txt`, and returns `(new_tab_path, content)`.
pub fn unarchive_tab(archive_path: &PathBuf) -> Result<(PathBuf, String)> {
    ensure_cache_dir()?;
    let content = fs::read_to_string(archive_path)?;
    let archive_filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let tab_filename = naming::archive_to_tab_filename(archive_filename);
    let tab_path = cache_dir().join(tab_filename);

    let tmp = tab_path.with_extension("tmp");
    fs::write(&tmp, content.as_bytes())?;
    fs::rename(&tmp, &tab_path)?;

    if archive_path.exists() {
        let _ = fs::remove_file(archive_path);
    }

    Ok((tab_path, content))
}

/// Permanently deletes an archive file.
pub fn delete_archive_file(archive_path: &PathBuf) -> Result<()> {
    if archive_path.exists() {
        fs::remove_file(archive_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naming_predicates() {
        assert!(naming::is_tab_filename("tab-a3f9c1-20260528-091500.txt"));
        assert!(!naming::is_tab_filename("archive-a3f9c1-20260528-091500.txt"));
        assert!(!naming::is_tab_filename("window.cfg"));

        assert!(naming::is_archive_filename("archive-a3f9c1-20260528-091500.txt"));
        assert!(!naming::is_archive_filename("tab-a3f9c1-20260528-091500.txt"));
    }

    #[test]
    fn test_tab_archive_conversions() {
        let tab = "tab-a3f9c1-20260528-091500.txt";
        let arch = naming::tab_to_archive_filename(tab);
        assert_eq!(arch, "archive-a3f9c1-20260528-091500.txt");

        let restored = naming::archive_to_tab_filename(&arch);
        assert_eq!(restored, tab);
    }

    #[test]
    fn test_timestamp_formatting() {
        let raw = "20260528-091500";
        let formatted = naming::format_timestamp_display(raw);
        assert_eq!(formatted, "28/05/2026 09:15:00");
    }
}


