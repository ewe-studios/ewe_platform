use std::collections::VecDeque;

/// In-memory history buffer.
pub struct ReplHistory {
    entries: VecDeque<String>,
    max_len: usize,
    cursor: Option<usize>,
}

#[allow(dead_code)]
impl ReplHistory {
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_len),
            max_len,
            cursor: None,
        }
    }

    /// Push a new entry. Empty entries are ignored; duplicates are moved to the end.
    pub fn push(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e == &entry) {
            self.entries.remove(pos);
        }
        if self.entries.len() >= self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.cursor = None;
    }

    /// Navigate up in history (oldest entries first).
    ///
    /// # Panics
    /// Never panics: `cursor` is assigned immediately above the lookup.
    pub fn up(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let last_idx = self.entries.len() - 1;
        let cursor = match self.cursor {
            None => last_idx,
            Some(0) => 0,
            Some(n) => n - 1,
        };
        self.cursor = Some(cursor);
        self.entries.get(cursor).map(String::as_str)
    }

    /// Navigate down in history. Returns None when past the bottom.
    pub fn down(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(n) if n >= self.entries.len() - 1 => {
                self.cursor = None;
                None
            }
            Some(n) => {
                self.cursor = Some(n + 1);
                self.entries.get(n + 1).map(std::string::String::as_str)
            }
        }
    }
}

#[cfg(feature = "history-file")]
impl ReplHistory {
    /// Load history from a JSON file.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or does not hold a JSON
    /// array of strings.
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let entries: Vec<String> = serde_json::from_str(&data)?;
        let max_len = entries.len().max(1000);
        let mut deque = VecDeque::with_capacity(max_len);
        for entry in entries.iter().skip(entries.len().saturating_sub(max_len)) {
            deque.push_back(entry.clone());
        }
        Ok(Self {
            entries: deque,
            max_len,
            cursor: None,
        })
    }

    /// Save history to a JSON file.
    ///
    /// # Errors
    /// Returns an error if the history cannot be serialised or the file cannot
    /// be written.
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let data = serde_json::to_string(&self.entries)?;
        std::fs::write(path, data)
    }
}
