---
workspace_name: "ewe_platform"
spec_directory: "specifications/59-foundation-shell-repl"
feature_directory: "specifications/59-foundation-shell-repl/features/F01-core-repl"
this_file: "specifications/59-foundation-shell-repl/features/F01-core-repl/feature.md"

status: planned
priority: high
created: 2026-07-20

depends_on: []

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# F01 — Core REPL engine: prompt, multiline input, reply rendering

## Overview

The foundational REPL loop: initialize a terminal session, display a prompt,
collect user input (single or multiline), yield it to the caller, then display
the caller's response. This is the minimal viable REPL — everything else
(history, commands, customization) layers on top.

[spec](../spec.md).

---

## Part A — Core types

```rust
// foundation_shell_repl/src/lib.rs

mod repl;
mod input;
mod display;
mod config;

pub use config::ReplConfig;
pub use repl::{Repl, ReplBuilder};
pub use input::ReplMessage;
pub use display::ReplResponse;
```

```rust
// foundation_shell_repl/src/config.rs

use std::io::{self, Write};

/// Configuration for a REPL session.
#[derive(Debug, Clone)]
pub struct ReplConfig {
    /// Primary prompt string (e.g. ">>> ").
    pub prompt: String,
    /// Continuation prompt for multiline input (e.g. "... ").
    pub continuation_prompt: String,
    /// Welcome banner shown on startup.
    pub banner: Option<String>,
    /// Goodbye message shown on exit.
    pub goodbye: Option<String>,
    /// Maximum input length (chars). None = unlimited.
    pub max_input_length: Option<usize>,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: ">>> ".into(),
            continuation_prompt: "... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
        }
    }
}
```

```rust
// foundation_shell_repl/src/repl.rs

use std::io::{self, Write};
use crate::config::ReplConfig;
use crate::input::ReplInputReader;
use crate::display::ReplDisplay;

/// Main REPL handle.
pub struct Repl {
    config: ReplConfig,
    input: ReplInputReader,
    display: ReplDisplay,
    exited: bool,
}

impl Repl {
    /// Create a new REPL with default config, using stdout/stderr.
    pub fn new() -> Self {
        Self::with_config(ReplConfig::default())
    }

    /// Create a new REPL with custom config.
    pub fn with_config(config: ReplConfig) -> Self {
        let input = ReplInputReader::new();
        let display = ReplDisplay::new();
        Self { config, input, display, exited: false }
    }

    /// Start building a custom REPL.
    pub fn builder() -> ReplBuilder {
        ReplBuilder::default()
    }

    /// Returns an iterator over user messages.
    ///
    /// Each iteration blocks until the user presses Enter (not Shift+Enter).
    /// The iterator ends when the user types `exit` or presses Ctrl+D.
    pub fn messages(&mut self) -> ReplMessageIter<'_> {
        // Show banner on first messages() call
        if let Some(ref banner) = self.config.banner {
            self.display.print_banner(banner);
        }
        ReplMessageIter { repl: self }
    }

    /// Display a response from the caller.
    ///
    /// Clears the current input line, prints the response with proper
    /// formatting, then redraws the prompt + any buffered input so the
    /// cursor ends up at the right position.
    pub fn reply(&mut self, response: impl AsRef<str>) {
        self.display.print_response(response.as_ref());
    }

    /// Signal that the REPL should exit after the current message.
    pub fn exit(&mut self) {
        self.exited = true;
    }
}

impl Drop for Repl {
    fn drop(&mut self) {
        if let Some(ref msg) = self.config.goodbye {
            eprintln!("{msg}");
        }
    }
}

/// Iterator that yields complete user messages.
pub struct ReplMessageIter<'a> {
    repl: &'a mut Repl,
}

impl Iterator for ReplMessageIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.repl.exited {
            return None;
        }

        let result = self.repl.input.read_message(
            &self.repl.config.prompt,
            &self.repl.config.continuation_prompt,
            &mut self.repl.display,
            self.repl.config.max_input_length,
        );

        match result {
            Ok(msg) if msg.is_empty() && self.repl.input.eof() => {
                // Ctrl+D on empty line = exit
                None
            }
            Ok(msg) => Some(msg),
            Err(e) => {
                // Ctrl+C or terminal error — yield None to end iteration
                self.repl.display.print_error(&format!("error: {e}"));
                None
            }
        }
    }
}
```

```rust
// foundation_shell_repl/src/repl.rs — builder

#[derive(Default)]
pub struct ReplBuilder {
    config: ReplConfig,
}

impl ReplBuilder {
    pub fn prompt(mut self, s: impl Into<String>) -> Self {
        self.config.prompt = s.into();
        self
    }

    pub fn continuation_prompt(mut self, s: impl Into<String>) -> Self {
        self.config.continuation_prompt = s.into();
        self
    }

    pub fn banner(mut self, s: impl Into<String>) -> Self {
        self.config.banner = Some(s.into());
        self
    }

    pub fn goodbye(mut self, s: impl Into<String>) -> Self {
        self.config.goodbye = Some(s.into());
        self
    }

    pub fn max_input_length(mut self, n: usize) -> Self {
        self.config.max_input_length = Some(n);
        self
    }

    pub fn build(self) -> Repl {
        Repl::with_config(self.config)
    }
}
```

---

## Part B — Input reader

Handles raw terminal input: reads keypresses, assembles multiline buffers,
distinguishes Enter (submit) from Shift+Enter (newline).

```rust
// foundation_shell_repl/src/input.rs

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::io;

pub struct ReplInputReader {
    buffer: String,
    had_eof: bool,
}

impl ReplInputReader {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            had_eof: false,
        }
    }

    /// Check if the last read was EOF (Ctrl+D).
    pub fn eof(&self) -> bool {
        self.had_eof
    }

    /// Read one complete message from the terminal.
    ///
    /// Enters raw mode, reads keypresses until Enter is pressed (not
    /// Shift+Enter), then leaves raw mode and returns the assembled string.
    pub fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut ReplDisplay,
        max_len: Option<usize>,
    ) -> io::Result<String> {
        // Enter raw + no-echo mode
        crossterm::terminal::enable_raw_mode()?;

        self.buffer.clear();
        self.had_eof = false;

        // Draw initial prompt
        display.draw_prompt(prompt, &self.buffer);

        let mut line_count = 0;

        loop {
            if !event::poll(std::time::Duration::from_millis(16))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };

            if key.kind != event::KeyEventKind::Press {
                continue;
            }

            match (key.modifiers, key.code) {
                // Ctrl+C — interrupt, return error
                (_, KeyCode::Char('c')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
                }

                // Ctrl+D — EOF
                (_, KeyCode::Char('d')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.had_eof = true;
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Ok(self.buffer.clone());
                }

                // Enter — submit
                (m, KeyCode::Enter) if !m.contains(KeyModifiers::SHIFT) => {
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Ok(self.buffer.clone());
                }

                // Shift+Enter — insert newline and continue
                (_, KeyCode::Enter) => {
                    self.buffer.push('\n');
                    line_count += 1;
                    let cp = if line_count > 0 {
                        continuation_prompt
                    } else {
                        prompt
                    };
                    display.redraw_with_newline(cp, &self.buffer);
                }

                // Printable characters
                (_, KeyCode::Char(c)) => {
                    if max_len.map_or(false, |n| self.buffer.len() >= n) {
                        continue; // silently ignore over-limit input
                    }
                    self.buffer.push(c);
                    display.append_char(c, &self.buffer);
                }

                // Backspace
                (_, KeyCode::Backspace) => {
                    if !self.buffer.is_empty() {
                        self.buffer.pop();
                        display.backspace(&self.buffer);
                    }
                }

                // Ctrl+U — clear entire line
                (_, KeyCode::Char('u')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.buffer.clear();
                    display.clear_line(prompt);
                }

                // Left/Right arrow — move cursor (basic line editing)
                (_, KeyCode::Left) => {
                    display.move_cursor_left();
                }
                (_, KeyCode::Right) => {
                    display.move_cursor_right();
                }

                _ => {} // Ignore unrecognized keys
            }
        }
    }
}
```

---

## Part C — Display

Manages terminal output: drawing prompts, responses, scrolling.

```rust
// foundation_shell_repl/src/display.rs

use crossterm::{
    cursor::{self, MoveTo, MoveToColumn},
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType, ScrollUp},
    QueueableCommand,
};
use std::io::{self, stderr, Stderr, Write};

pub struct ReplDisplay {
    stderr: Stderr,
    cursor_col: u16,
    cursor_row: u16,
}

impl ReplDisplay {
    pub fn new() -> Self {
        Self {
            stderr: stderr(),
            cursor_col: 0,
            cursor_row: 0,
        }
    }

    /// Print a banner on startup.
    pub fn print_banner(&mut self, banner: &str) {
        let _ = writeln!(self.stderr, "\n{banner}\n");
    }

    /// Draw the initial prompt.
    pub fn draw_prompt(&mut self, prompt: &str, _buffer: &str) {
        let _ = self.stderr
            .queue(SetForegroundColor(Color::DarkGreen))
            .queue(Print(prompt))
            .queue(ResetColor)
            .flush();
        self.cursor_col = prompt.len() as u16;
    }

    /// Redraw after Shift+Enter: newline + continuation prompt.
    pub fn redraw_with_newline(&mut self, prompt: &str, buffer: &str) {
        let _ = writeln!(self.stderr);
        let _ = self.stderr
            .queue(SetForegroundColor(Color::DarkGreen))
            .queue(Print(prompt))
            .queue(ResetColor)
            .flush();
        self.cursor_col = prompt.len() as u16;
    }

    /// Append a character to the current line.
    pub fn append_char(&mut self, _c: char, buffer: &str) {
        // For simplicity, redraw the entire current line
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine))
            .queue(Print(buffer))
            .flush();
    }

    /// Handle backspace.
    pub fn backspace(&mut self, buffer: &str) {
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine))
            .queue(Print(buffer))
            .flush();
    }

    /// Clear the entire input line (Ctrl+U).
    pub fn clear_line(&mut self, prompt: &str) {
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine))
            .queue(SetForegroundColor(Color::DarkGreen))
            .queue(Print(prompt))
            .queue(ResetColor)
            .flush();
        self.cursor_col = prompt.len() as u16;
    }

    /// Print a newline and disable raw mode cleanup.
    pub fn newline(&mut self) {
        let _ = writeln!(self.stderr);
    }

    /// Print a response from the caller.
    pub fn print_response(&mut self, response: &str) {
        let _ = writeln!(self.stderr);
        for line in response.lines() {
            let _ = writeln!(self.stderr, "  {line}");
        }
        let _ = writeln!(self.stderr);
    }

    /// Print an error message.
    pub fn print_error(&mut self, msg: &str) {
        let _ = self.stderr
            .queue(SetForegroundColor(Color::Red))
            .queue(Print(msg))
            .queue(ResetColor)
            .flush();
        let _ = writeln!(self.stderr);
    }

    /// Move cursor left (basic line editing).
    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            let _ = self.stderr.queue(MoveToColumn(self.cursor_col)).flush();
        }
    }

    /// Move cursor right (basic line editing).
    pub fn move_cursor_right(&mut self) {
        self.cursor_col += 1;
        let _ = self.stderr.queue(MoveToColumn(self.cursor_col)).flush();
    }
}

/// A response that can be printed by the REPL.
pub struct ReplResponse {
    pub text: String,
    pub is_error: bool,
}

impl ReplResponse {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: false }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: true }
    }
}
```

---

## Part D — Cargo.toml

```toml
[package]
name = "foundation_shell_repl"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "A lightweight, general-purpose REPL framework for interactive terminal sessions"

[dependencies]
crossterm = "0.28"

[features]
history-file = ["dep:serde", "dep:serde_json"]
colors = []

default = ["colors"]
```

---

## Acceptance criteria

1. `Repl::new()` starts a session that displays `>>> ` and waits for input.
2. Typing `hello` + Enter yields `"hello"` from `messages()`.
3. Typing `line one` + Shift+Enter + `line two` + Enter yields
   `"line one\nline two"` from `messages()`.
4. `repl.reply("ok")` prints `ok` below the prompt and re-displays the prompt.
5. Ctrl+C interrupts and ends the iteration.
6. Ctrl+D on an empty line ends the iteration.
7. The builder allows custom prompt/banner/goodbye strings.
8. Ctrl+U clears the current input line.
