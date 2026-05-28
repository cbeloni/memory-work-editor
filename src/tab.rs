use std::path::PathBuf;

pub struct Tab {
    pub id: usize,
    /// Short hash extracted from the filename (e.g. "a3f9c1").
    pub title: String,
    pub content: String,
    pub file_path: PathBuf,
    pub dirty: bool,
}

impl Tab {
    pub fn new(id: usize, file_path: PathBuf, content: String) -> Self {
        let title = Self::extract_hash(&file_path);
        Self { id, title, content, file_path, dirty: false }
    }

    fn extract_hash(path: &PathBuf) -> String {
        path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').nth(1))
            .unwrap_or("tab")
            .to_string()
    }

    /// Tab label shown in the tab bar.
    /// Shows the first non-empty line of content (truncated) or the hash when empty.
    pub fn display_title(&self) -> String {
        let base = self
            .content
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|line| {
                let trimmed = line.trim();
                let chars: Vec<char> = trimmed.chars().take(22).collect();
                if trimmed.chars().count() > 22 {
                    format!("{}…", chars.iter().collect::<String>())
                } else {
                    trimmed.to_string()
                }
            })
            .unwrap_or_else(|| self.title.clone());

        if self.dirty {
            format!("{}*", base)
        } else {
            base
        }
    }
}
