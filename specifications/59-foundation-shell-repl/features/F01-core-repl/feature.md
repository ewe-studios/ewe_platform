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

# F01 — Core REPL: shared traits + native input/display + wasm stubs + REPL loop

## Overview

The foundational REPL loop: initialize a terminal session, display a prompt,
collect user input (single or multiline), yield it to the caller, then display
the caller's response. The crate uses `target_family` cfg gates to pick native
(stdin/stdout + crossterm) or wasm (stub/web-sys) backends.

[spec](../spec.md).

---

## Part A — Crate layout (lib.rs)

```rust
// foundation_shell_repl/src/lib.rs

mod shared;

#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(target_family = "wasm")]
mod wasm;

pub use shared::config::{ReplConfig, ReplColors};
pub use shared::repl::{Repl, ReplBuilder};
pub use shared::traits::{ReplInput, ReplDisplay};
```

## Part B — Shared traits

```rust
// foundation_shell_repl/src/shared/traits.rs

use std::io;

/// Reads keystrokes and assembles multiline input.
pub trait ReplInput: Sized {
    fn new() -> Self;

    /// Read one complete message from the terminal.
    ///
    /// Enters raw mode (native) or equivalent, reads keypresses until Enter
    /// is pressed (not Shift+Enter), then returns the assembled string.
    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn crate::ReplDisplay,
        max_len: Option<usize>,
    ) -> io::Result<String>;

    /// Check if the last read was EOF (Ctrl+D or equivalent).
    fn eof(&self) -> bool;
}

/// Renders prompts, responses, and cursor movements.
pub trait ReplDisplay: Sized {
    fn new() -> Self;

    /// Print a banner on startup.
    fn print_banner(&mut self, banner: &str);

    /// Draw the initial prompt.
    fn draw_prompt(&mut self, prompt: &str, buffer: &str);

    /// Redraw after Shift+Enter: newline + continuation prompt.
    fn redraw_with_newline(&mut self, prompt: &str, buffer: &str);

    /// Append a character to the current line (redraws the line).
    fn append_char(&mut self, c: char, buffer: &str);

    /// Handle backspace (redraws the line).
    fn backspace(&mut self, buffer: &str);

    /// Clear the entire input line (Ctrl+U).
    fn clear_line(&mut self, prompt: &str);

    /// Print a newline and flush.
    fn newline(&mut self);

    /// Print a response from the caller.
    fn print_response(&mut self, response: &str);

    /// Print an error message.
    fn print_error(&mut self, msg: &str);

    /// Move cursor left (basic line editing).
    fn move_cursor_left(&mut self);

    /// Move cursor right (basic line editing).
    fn move_cursor_right(&mut self);
}

// Re-export as top-level trait aliases
pub use crate::{ReplInput, ReplDisplay};
```

## Part C — Shared config

```rust
// foundation_shell_repl/src/shared/config.rs

#[cfg(feature = "colors")]
use crossterm::style::Color;

/// Color configuration for the REPL display.
#[cfg(feature = "colors")]
#[derive(Debug, Clone)]
pub struct ReplColors {
    /// Prompt foreground color. Default: DarkGreen.
    pub prompt_color: Color,
    /// Continuation prompt foreground color. Default: DarkYellow.
    pub continuation_prompt_color: Color,
    /// Response text foreground color. Default: None (no change).
    pub response_color: Option<Color>,
    /// Error text foreground color. Default: Red.
    pub error_color: Color,
}

#[cfg(feature = "colors")]
impl Default for ReplColors {
    fn default() -> Self {
        Self {
            prompt_color: Color::DarkGreen,
            continuation_prompt_color: Color::DarkYellow,
            response_color: None,
            error_color: Color::Red,
        }
    }
}

/// Full configuration for a REPL session.
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
    #[cfg(feature = "colors")]
    pub colors: ReplColors,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: ">>> ".into(),
            continuation_prompt: "... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
            #[cfg(feature = "colors")]
            colors: ReplColors::default(),
        }
    }
}
```

## Part D — Shared REPL struct (repl.rs)

```rust
// foundation_shell_repl/src/shared/repl.rs

use std::io;
use crate::config::ReplConfig;
use crate::traits::ReplInput;
use crate::traits::ReplDisplay;
use crate::commands::CommandRegistry;

/// Main REPL handle.
pub struct Repl {
    config: ReplConfig,
    input: Box<dyn ReplInput>,
    display: Box<dyn ReplDisplay>,
    commands: CommandRegistry,
    exited: bool,
}

impl Repl {
    /// Create a new REPL with default config, using the platform-native backend.
    pub fn new() -> Self {
        Self::with_config(ReplConfig::default())
    }

    /// Create a new REPL with custom config.
    pub fn with_config(config: ReplConfig) -> Self {
        let input = <impl ReplInput>::new();
        let display = <impl ReplDisplay>::new();
        Self {
            config,
            input: Box::new(input),
            display: Box::new(display),
            commands: CommandRegistry::new(),
            exited: false,
        }
    }

    /// Start building a custom REPL.
    pub fn builder() -> ReplBuilder {
        ReplBuilder::default()
    }

    /// Returns an iterator over user messages.
    pub fn messages(&mut self) -> ReplMessageIter<'_> {
        if let Some(ref banner) = self.config.banner {
            self.display.print_banner(banner);
        }
        ReplMessageIter { repl: self }
    }

    /// Display a response from the caller.
    pub fn reply(&mut self, response: impl AsRef<str>) {
        self.display.print_response(response.as_ref());
    }

    /// Register a custom `/command`.
    pub fn register_command(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&str) -> String + Send + 'static,
    ) {
        self.commands.register(name, handler);
    }

    /// Signal that the REPL should exit after the current message.
    pub fn exit(&mut self) {
        self.exited = true;
    }
}

impl Drop for Repl {
    fn drop(&mut self) {
        if let Some(ref msg) = self.config.goodbye {
            // Use eprintln for native, console.log for wasm — handled by display
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
            &mut *self.repl.display,
            self.repl.config.max_input_length,
        );

        match result {
            Ok(msg) if msg.is_empty() && self.repl.input.eof() => None,
            Ok(msg) => {
                // Check for commands before yielding
                if let Some(cmd_result) = self.repl.commands.try_handle(&msg) {
                    use crate::commands::CommandResult;
                    match cmd_result {
                        CommandResult::Output(text) => {
                            self.repl.display.print_response(&text);
                            return Some(msg);
                        }
                        CommandResult::Terminal(escapes) => {
                            print!("\x1b[2J\x1b[H");
                            return Some(msg);
                        }
                        CommandResult::Exit => {
                            self.repl.exit();
                            return None;
                        }
                    }
                }
                Some(msg)
            }
            Err(_) => None,
        }
    }
}
```

## Part E — ReplBuilder

```rust
// foundation_shell_repl/src/shared/repl.rs — builder

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

    #[cfg(feature = "colors")]
    pub fn colors(mut self, colors: ReplColors) -> Self {
        self.config.colors = colors;
        self
    }

    pub fn build(self) -> Repl {
        Repl::with_config(self.config)
    }
}
```

## Part F — Native backend (input.rs)

```rust
// foundation_shell_repl/src/native/mod.rs

pub mod input;
pub mod display;
```

```rust
// foundation_shell_repl/src/native/input.rs

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use crate::traits::{ReplInput, ReplDisplay};

pub struct NativeInput {
    buffer: String,
    had_eof: bool,
}

impl ReplInput for NativeInput {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            had_eof: false,
        }
    }

    fn eof(&self) -> bool {
        self.had_eof
    }

    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
    ) -> io::Result<String> {
        crossterm::terminal::enable_raw_mode()?;

        self.buffer.clear();
        self.had_eof = false;

        display.draw_prompt(prompt, &self.buffer);

        let mut line_count = 0;

        loop {
            if !event::poll(std::time::Duration::from_millis(16))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };

            if key.kind != KeyEventKind::Press {
                continue;
            }

            match (key.modifiers, key.code) {
                // Ctrl+C
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

                // Enter (no shift) — submit
                (m, KeyCode::Enter) if !m.contains(KeyModifiers::SHIFT) && !m.contains(KeyModifiers::CONTROL) => {
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Ok(self.buffer.clone());
                }

                // Shift+Enter or Ctrl+Enter — insert newline
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
                        continue;
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

                // Ctrl+U — clear line
                (_, KeyCode::Char('u')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.buffer.clear();
                    display.clear_line(prompt);
                }

                // Left/Right arrows
                (_, KeyCode::Left) => display.move_cursor_left(),
                (_, KeyCode::Right) => display.move_cursor_right(),

                // Up/Down — history navigation (F02)
                (_, KeyCode::Up) => {}
                (_, KeyCode::Down) => {}

                _ => {}
            }
        }
    }
}
```

## Part G — Native backend (display.rs)

```rust
// foundation_shell_repl/src/native/display.rs

use crossterm::{
    cursor::MoveToColumn,
    style::{Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    QueueableCommand,
};
use std::io::{self, stderr, Stderr, Write};

#[cfg(feature = "colors")]
use crossterm::style::Color;

use crate::traits::ReplDisplay;

pub struct NativeDisplay {
    stderr: Stderr,
    cursor_col: u16,
    #[cfg(feature = "colors")]
    colors: crate::config::ReplColors,
}

impl ReplDisplay for NativeDisplay {
    fn new() -> Self {
        Self {
            stderr: stderr(),
            cursor_col: 0,
            #[cfg(feature = "colors")]
            colors: crate::config::ReplColors::default(),
        }
    }

    fn print_banner(&mut self, banner: &str) {
        let _ = writeln!(self.stderr, "\n{banner}\n");
    }

    fn draw_prompt(&mut self, prompt: &str, _buffer: &str) {
        #[cfg(feature = "colors")]
        let _ = self.stderr
            .queue(SetForegroundColor(self.colors.prompt_color))
            .queue(Print(prompt))
            .queue(ResetColor);
        #[cfg(not(feature = "colors"))]
        let _ = write!(self.stderr, "{prompt}");
        let _ = self.stderr.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn redraw_with_newline(&mut self, prompt: &str, buffer: &str) {
        let _ = writeln!(self.stderr);
        #[cfg(feature = "colors")]
        let _ = self.stderr
            .queue(SetForegroundColor(self.colors.continuation_prompt_color))
            .queue(Print(prompt))
            .queue(ResetColor);
        #[cfg(not(feature = "colors"))]
        let _ = write!(self.stderr, "{prompt}");
        let _ = self.stderr.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn append_char(&mut self, _c: char, buffer: &str) {
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine))
            .queue(Print(buffer))
            .flush();
    }

    fn backspace(&mut self, buffer: &str) {
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine))
            .queue(Print(buffer))
            .flush();
    }

    fn clear_line(&mut self, prompt: &str) {
        let _ = self.stderr
            .queue(MoveToColumn(0))
            .queue(Clear(ClearType::UntilNewLine));
        #[cfg(feature = "colors")]
        let _ = self.stderr
            .queue(SetForegroundColor(self.colors.prompt_color))
            .queue(Print(prompt))
            .queue(ResetColor);
        #[cfg(not(feature = "colors"))]
        let _ = write!(self.stderr, "{prompt}");
        let _ = self.stderr.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn newline(&mut self) {
        let _ = writeln!(self.stderr);
    }

    fn print_response(&mut self, response: &str) {
        let _ = writeln!(self.stderr);
        for line in response.lines() {
            let _ = writeln!(self.stderr, "  {line}");
        }
        let _ = writeln!(self.stderr);
    }

    fn print_banner(&mut self, banner: &str) {
        let _ = writeln!(self.stderr, "\n{banner}\n");
    }

    fn print_error(&mut self, msg: &str) {
        #[cfg(feature = "colors")]
        let _ = self.stderr
            .queue(SetForegroundColor(self.colors.error_color))
            .queue(Print(msg))
            .queue(ResetColor);
        #[cfg(not(feature = "colors"))]
        let _ = write!(self.stderr, "{msg}");
        let _ = self.stderr.flush();
        let _ = writeln!(self.stderr);
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            let _ = self.stderr.queue(MoveToColumn(self.cursor_col)).flush();
        }
    }

    fn move_cursor_right(&mut self) {
        self.cursor_col += 1;
        let _ = self.stderr.queue(MoveToColumn(self.cursor_col)).flush();
    }
}
```

## Part H — Wasm backend (stubs)

```rust
// foundation_shell_repl/src/wasm/mod.rs

pub mod input;
pub mod display;
```

```rust
// foundation_shell_repl/src/wasm/input.rs

use std::io;
use crate::traits::{ReplInput, ReplDisplay};

/// Wasm input stub — reads from a simulated buffer or returns placeholder text.
/// In a real wasm32 environment this would hook into web-sys for keyboard events.
pub struct WasmInput {
    had_eof: bool,
}

impl ReplInput for WasmInput {
    fn new() -> Self {
        Self { had_eof: false }
    }

    fn eof(&self) -> bool {
        self.had_eof
    }

    fn read_message(
        &mut self,
        _prompt: &str,
        _continuation_prompt: &str,
        _display: &mut dyn ReplDisplay,
        _max_len: Option<usize>,
    ) -> io::Result<String> {
        // Stub: in wasm32 we can't read stdin interactively.
        // A real implementation would use web-sys DOM events or a JS bridge.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive input not available on wasm32",
        ))
    }
}
```

```rust
// foundation_shell_repl/src/wasm/display.rs

use crate::traits::ReplDisplay;

/// Wasm display stub — prints to console.log / a DOM element.
pub struct WasmDisplay;

impl ReplDisplay for WasmDisplay {
    fn new() -> Self {
        Self
    }

    fn print_banner(&mut self, banner: &str) {
        log_wasm!("BANNER: {banner}");
    }

    fn draw_prompt(&mut self, prompt: &str, _buffer: &str) {
        log_wasm!("PROMPT: {prompt}");
    }

    fn redraw_with_newline(&mut self, prompt: &str, _buffer: &str) {
        log_wasm!("CONT: {prompt}");
    }

    fn append_char(&mut self, _c: char, buffer: &str) {
        log_wasm!("INPUT: {buffer}");
    }

    fn backspace(&mut self, buffer: &str) {
        log_wasm!("INPUT: {buffer}");
    }

    fn clear_line(&mut self, prompt: &str) {
        log_wasm!("CLEAR -> {prompt}");
    }

    fn newline(&mut self) {
        log_wasm!("");
    }

    fn print_response(&mut self, response: &str) {
        for line in response.lines() {
            log_wasm!("  {line}");
        }
    }

    fn print_error(&mut self, msg: &str) {
        log_wasm!("ERROR: {msg}");
    }

    fn move_cursor_left(&mut self) {}
    fn move_cursor_right(&mut self) {}
}

macro_rules! log_wasm {
    ($($arg:tt)*) => {
        #[cfg(feature = "wasm")]
        web_sys::console::log_1(&format!($($arg)*).into());
        #[cfg(not(feature = "wasm"))]
        eprintln!($($arg)*);
    };
}
```

## Part I — Cargo.toml

```toml
[package]
name = "foundation_shell_repl"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "A lightweight, general-purpose REPL framework for interactive terminal sessions"

[dependencies]
crossterm = { version = "0.28", optional = true }
js-sys = { version = "0.3", optional = true }
web-sys = { version = "0.3", features = ["console"], optional = true }

[features]
default = ["native"]
native = ["dep:crossterm"]
colors = ["native"]
history-file = ["dep:serde", "dep:serde_json"]
wasm = ["dep:js-sys", "dep:web-sys"]
```

---

## Acceptance criteria

1. `Repl::new()` starts a session on native that displays `>>> ` and waits for input.
2. On wasm32, `Repl::new()` compiles without error (stub backends).
3. Typing `hello` + Enter yields `"hello"` from `messages()`.
4. Typing `line one` + Shift+Enter + `line two` + Enter yields
   `"line one\nline two"` from `messages()`.
5. `repl.reply("ok")` prints `ok` below the prompt and re-displays the prompt.
6. Ctrl+C interrupts and ends the iteration.
7. Ctrl+D on an empty line ends the iteration.
8. The builder allows custom prompt/banner/goodbye strings.
9. Ctrl+U clears the current input line.
10. `cargo check --target wasm32-unknown-unknown` succeeds with the `wasm` feature.
