//! A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
//! interactive terminal sessions.
//!
//! # Quick start
//!
//! ```no_run
//! use foundation_shell_repl::Repl;
//!
//! fn main() {
//!     let mut repl = Repl::new();
//!
//!     // Register custom commands (optional)
//!     repl.register_command("greet", |cmd| {
//!         let name = cmd.strip_prefix("/greet ").unwrap_or("world");
//!         format!("Hello, {name}!")
//!     });
//!
//!     // Process user input
//!     for input in repl.messages() {
//!         let response = format!("You said: {input}");
//!         repl.reply(&response);
//!     }
//! }
//! ```
//!
//! # How it works
//!
//! The REPL has three layers:
//!
//! 1. **Input** — reads keystrokes in raw terminal mode (native) or stubs
//!    (wasm32). Enter submits the current buffer; Shift+Enter appends a
//!    newline. Ctrl+C interrupts; Ctrl+D ends the session.
//! 2. **Commands** — lines starting with `/` are intercepted before reaching
//!    the iterator. Built-ins: `/help`, `/clear`, `/exit`, `/version`.
//!    Register your own with [`Repl::register_command()`].
//! 3. **Output** — [`Repl::reply()`] prints the response below the prompt
//!    and re-displays the input line.
//!
//! # Multiline input
//!
//! Pressing **Shift+Enter** (or Ctrl+Enter) inserts a newline into the buffer
//! and continues editing. Plain **Enter** submits the full text as one string.
//!
//! # Customization
//!
//! Use [`Repl::builder()`] to customize prompts, colors, banners:
//!
//! ```no_run
//! use foundation_shell_repl::{Repl, ReplConfig};
//!
//! fn main() {
//!     let mut repl = Repl::builder()
//!         .prompt("myapp> ")
//!         .continuation_prompt("      ")
//!         .banner("Welcome to MyApp REPL v1.0")
//!         .goodbye("Goodbye!")
//!         .build();
//!
//!     for input in repl.messages() {
//!         repl.reply(&format!("got: {input}"));
//!     }
//! }
//! ```
//!
//! # Cross-platform
//!
//! | Target | Feature | Input | Display |
//! |--------|---------|-------|---------|
//! | unix/windows | `native` (default) | crossterm raw-mode | crossterm terminal |
//! | wasm32 | `wasm` | stub (returns error) | console.log stub |
//!
//! # Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `native` | ✅ | crossterm-based terminal I/O |
//! | `colors` | — | ANSI color support for prompts and errors |
//! | `history-file` | — | Save/load history to disk (serde + file I/O) |
//! | `wasm` | — | wasm32 stub backends |

mod shared;

#[cfg(all(feature = "native", not(feature = "wasm")))]
mod native;

#[cfg(feature = "wasm")]
mod wasm;

pub use shared::config::ReplConfig;
#[cfg(feature = "colors")]
pub use shared::config::ReplColors;
pub use shared::commands::{CommandRegistry, CommandResult};
pub use shared::repl::{Repl, ReplBuilder, ReplMessageIter};
pub use shared::traits::{ReplDisplay, ReplInput};
