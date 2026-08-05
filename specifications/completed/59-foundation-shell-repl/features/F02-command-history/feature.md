---
workspace_name: "ewe_platform"
spec_directory: "specifications/59-foundation-shell-repl"
feature_directory: "specifications/59-foundation-shell-repl/features/F02-command-history"
this_file: "specifications/59-foundation-shell-repl/features/F02-command-history/feature.md"

status: complete
priority: medium
created: 2026-07-20
completed: 2026-07-31

depends_on: ["F01-core-repl"]

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

> **Delivered** in `foundation_repl::ReplHistory` (ring buffer + up/down + dedup +
> empty-skip; disk persistence via the `history-file` feature,
> `load_from_file`/`save_to_file`, JSON). AC #5 (survives restart) is covered by
> `tests/history_file.rs::history_survives_save_and_reload`.

# F02 — Command history: ring buffer + up/down navigation + disk persistence

## Overview

Users can recall previous inputs via Up/Down arrow keys. History is stored in
an in-memory ring buffer (default 1000 entries). With the `history-file`
feature, history persists to disk as JSON between sessions.

[spec](../spec.md).

---

## Part A — History ring buffer

```rust
// foundation_shell_repl/src/history.rs

use std::collections::VecDeque;

/// In-memory history buffer.
pub struct ReplHistory {
    entries: VecDeque<String>,
    max_len: usize,
    cursor: Option<usize>, // None = at the bottom (editing new input)
}

impl ReplHistory {
    pub fn new(max_len: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_len),
            max_len,
            cursor: None,
        }
    }

    /// Push a new entry. Submissions already in history are not deduplicated.
    pub fn push(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }
        // Remove duplicates of the same entry if it exists
        if let Some(pos) = self.entries.iter().position(|e| e == &entry) {
            self.entries.remove(pos);
        }
        if self.entries.len() >= self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.cursor = None;
    }

    /// Navigate up in history. Returns the entry at the new cursor position.
    pub fn up(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let last_idx = self.entries.len() - 1;
        self.cursor = Some(match self.cursor {
            None => last_idx,
            Some(0) => 0,
            Some(n) => n - 1,
        });
        self.entries.get(self.cursor.unwrap()).map(|s| s.as_str())
    }

    /// Navigate down in history. Returns the entry at the new cursor position,
    /// or None if we've gone past the bottom (user is editing new input).
    pub fn down(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(n) if n >= self.entries.len() - 1 => {
                self.cursor = None;
                None
            }
            Some(n) => {
                self.cursor = Some(n + 1);
                self.entries.get(n + 1).map(|s| s.as_str())
            }
        }
    }
}
```

## Part B — Input integration

Up/Down arrow keys in `ReplInputReader` query the history buffer and replace
the current input line:

```rust
// In ReplInputReader::read_message():

// Up arrow
(_, KeyCode::Up) => {
    if let Some(entry) = self.history.up() {
        self.buffer = entry.to_string();
        display.replace_line(entry, prompt);
    }
}

// Down arrow
(_, KeyCode::Down) => {
    match self.history.down() {
        Some(entry) => {
            self.buffer = entry.to_string();
            display.replace_line(entry, prompt);
        }
        None => {
            self.buffer.clear();
            display.replace_line("", prompt);
        }
    }
}
```

After a message is successfully submitted, it gets pushed to history:

```rust
// In ReplMessageIter::next():
let msg = self.repl.input.read_message(...)?;
if !msg.is_empty() {
    self.repl.history.push(msg.clone());
}
Some(msg)
```

## Part C — Disk persistence (history-file feature)

```rust
#[cfg(feature = "history-file")]
impl ReplHistory {
    /// Load history from a JSON file.
    pub fn load_from_file(path: &std::path::Path) -> io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let entries: Vec<String> = serde_json::from_str(&data)?;
        let max_len = entries.len().max(1000);
        let mut deque = VecDeque::with_capacity(max_len);
        // Keep only the last max_len entries
        for entry in entries.into_iter().skip(entries.len().saturating_sub(max_len)) {
            deque.push_back(entry);
        }
        Ok(Self { entries: deque, max_len, cursor: None })
    }

    /// Save history to a JSON file.
    pub fn save_to_file(&self, path: &std::path::Path) -> io::Result<()> {
        let data = serde_json::to_string(&self.entries)?;
        std::fs::write(path, data)
    }
}
```

Enabled via `ReplBuilder::history_file(path)`.

## Acceptance criteria

1. After typing `hello` and pressing Enter, pressing Up recalls `hello`.
2. Pressing Down after Up clears the buffer (back to new input).
3. Duplicate submissions replace the earlier copy in history.
4. Empty submissions are not stored.
5. With `history-file`, history survives a REPL restart.
