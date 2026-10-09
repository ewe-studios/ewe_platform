//! A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
//! interactive terminal sessions.
//!
//! # Quick start
//!
//! ```no_run
//! use foundation_repl::Repl;
//!
//! let repl = Repl::new();
//!
//! // Register custom commands (optional)
//! repl.register_command("greet", |cmd| {
//!     let name = cmd.strip_prefix("/greet ").unwrap_or("world");
//!     format!("Hello, {name}!")
//! });
//!
//! // Process user input
//! for input in repl.messages() {
//!     repl.reply(&format!("You said: {input}"));
//! }
//! ```
//!
//! # What it looks like
//!
//! Input is edited inside a bordered, padded box pinned below the session's
//! output. Responses are printed above it, painted the full width of the
//! terminal in a darker shade, so the two regions never blur together:
//!
//! ```text
//!   ╭────────────────────────────────────────────╮
//!   │                                            │
//!   │   | what is the capital of France?         │
//!   │                                            │
//!   ╰────────────────────────────────────────────╯
//!   Paris.
//!
//!   ╭────────────────────────────────────────────╮
//!   │                                            │
//!   │   | █                                      │
//!   │                                            │
//!   ╰────────────────────────────────────────────╯
//! ```
//!
//! # How it works
//!
//! 1. **Input** — reads keystrokes in raw terminal mode (native) or stubs
//!    (wasm32). The buffer and the caret are owned together, so arrow keys,
//!    Home/End, Ctrl+W and history recall all agree with what is on screen.
//!    Enter submits; Shift+Enter (or any modifier) appends a newline. Ctrl+C
//!    abandons the current line; Ctrl+D on an empty line ends the session.
//! 2. **Commands** — lines starting with `/` are intercepted before reaching
//!    the iterator. Built-ins: `/help`, `/clear`, `/exit`, `/version`.
//!    Register your own with [`Repl::register_command()`].
//! 3. **Output** — [`Repl::reply()`] prints above the input box, which is then
//!    redrawn empty for the next message.
//!
//! Every redraw renders the whole input area from the buffer rather than
//! patching it, which is what keeps the prompt, the wrapping and the caret
//! consistent no matter what was typed.
//!
//! # Showing that work is happening
//!
//! A reply that takes seconds leaves the terminal looking hung.
//! [`Repl::animation()`] replaces the input box with an animated indicator
//! until the guard is dropped, and doubles as a sink for results that arrive a
//! piece at a time:
//!
//! ```no_run
//! use foundation_repl::Repl;
//!
//! let repl = Repl::new();
//! for input in repl.messages() {
//!     let stream = repl.animation("thinking");
//!     for token in generate(&input) {
//!         stream.push(&token);   // appears as it arrives
//!     }
//!     stream.finish();
//! }
//! # fn generate(input: &str) -> Vec<String> { vec![input.to_string()] }
//! ```
//!
//! Streamed text is committed to the terminal's scrollback a row at a time as
//! each row fills, so a long reply scrolls normally instead of being trapped in
//! a redrawn region. Calling [`Activity::set_progress`] with a ratio swaps the
//! spinner for a progress bar.
//!
//! # Multiline input
//!
//! Pressing **Shift+Enter** (or Ctrl+Enter) inserts a newline into the buffer
//! and continues editing, with the continuation prompt in the gutter. Plain
//! **Enter** submits the whole text as one string. The box grows as needed and
//! scrolls internally past [`ReplTheme::max_input_rows`].
//!
//! # Customization
//!
//! Use [`Repl::builder()`] to change prompts, banners and the theme:
//!
//! ```no_run
//! use foundation_repl::{BorderKind, Repl, ReplColor, ReplPadding, ReplTheme};
//!
//! let repl = Repl::builder()
//!     .prompt("myapp> ")
//!     .continuation_prompt("       ")
//!     .banner("Welcome to MyApp REPL v1.0")
//!     .goodbye("Goodbye!")
//!     .theme(ReplTheme {
//!         border: ReplColor::rgb(0x7a, 0x7a, 0x7a),
//!         border_kind: BorderKind::Thick,
//!         padding: ReplPadding::uniform(2),
//!         ..ReplTheme::default()
//!     })
//!     .build();
//!
//! for input in repl.messages() {
//!     repl.reply(&format!("got: {input}"));
//! }
//! ```
//!
//! # Clearing what you are typing
//!
//! **Ctrl+U** clears the line without submitting, cancelling or otherwise
//! interrupting anything. Because that is not guessable, the input box carries
//! a reminder in its bottom border — where it costs no rows, since the border
//! is being drawn anyway:
//!
//! ```text
//!   ╰──────────── ctrl+u clear · shift+enter newline · ctrl+c cancel ╯
//! ```
//!
//! Change the text with [`ReplTheme::hint`], or set it to `None` for no hint.
//!
//! # Palettes
//!
//! The default is **nebula** — deep violet-black with pastel mint, sky and rose
//! accents, built for people who cannot stand light mode. Three more ship with
//! it: `dusk` (warm), `abyss` (cool, highest contrast) and `slate` (the neutral
//! greyscale this crate used to default to).
//!
//! ```
//! use foundation_repl::{Repl, ReplTheme};
//!
//! let repl = Repl::builder().theme(ReplTheme::abyss()).build();
//! # drop(repl);
//! ```
//!
//! Every one is checked in `tests/palette.rs`: body text clears WCAG AA
//! against the surface it is actually drawn on, chrome that should recede is
//! held to a lower floor so it stays visible without competing, and the two
//! surfaces sit at least 1.25:1 apart in luminance so the box and the output
//! behind it never blur into one region.
//!
//! # Backends
//!
//! Nothing in the rendering is tied to a terminal: the box, the wrapping and
//! the activity indicator are measured into a cell buffer and flushed through
//! ratatui's `Backend`. A surface is added by implementing [`ReplHost`] —
//! create a backend, report a size, say whether the surface has a scrollback
//! of its own — and [`ReplRenderer`] does the rest.
//!
//! | Surface | Feature | Host |
//! |---------|---------|------|
//! | terminal | `native` | [`CrosstermHost`] |
//! | browser | `ratzilla` | [`RatzillaHost`] over any [ratzilla] backend |
//! | off-screen | — | your own, over ratatui's `TestBackend` |
//!
//! [`ViewportMode`] picks the output strategy: `Inline` hands finished lines to
//! a surface that has its own scrollback, `Fullscreen` keeps the transcript in
//! the renderer for one that does not.
//!
//! Note that the `ratzilla` feature supplies **rendering** only.
//! [`Repl::messages()`] blocks waiting on a keystroke, and a browser tab has no
//! blocking read — drive the renderer from ratzilla's `on_key_event` instead.
//!
//! [ratzilla]: https://github.com/ratatui/ratzilla
//!
//! # Cross-platform
//!
//! | Target | Feature | Input | Display |
//! |--------|---------|-------|---------|
//! | unix/windows | `native` (default) | crossterm raw-mode | ratatui inline viewport |
//! | wasm32 | `wasm` | stub (returns error) | `console.log` stub |
//!
//! # Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `native` | yes | crossterm + ratatui terminal I/O |
//! | `colors` | yes | colour the box, prompt and output shading |
//! | `history-file` | — | Save/load history to disk (serde + file I/O) |
//! | `wasm` | — | wasm32 stub backends |
//! | `ratzilla` | — | browser rendering via [ratzilla]; use instead of `native` |
//!
//! Turning `colors` off keeps the box, its padding and the layout, and drops
//! only the styling — useful for terminals without colour, or captured output.

mod shared;

#[cfg(all(feature = "native", not(feature = "wasm")))]
mod native;

#[cfg(any(feature = "wasm", feature = "ratzilla"))]
mod wasm;

pub use shared::activity::{Activity, ActivityView};
pub use shared::commands::{CommandHandler, CommandRegistry, CommandResult};
pub use shared::config::ReplConfig;
pub use shared::history::ReplHistory;
pub use shared::layout::{
    layout_input, split_committed, visible_window, wrap_text, InputLayout, InputRow, InputView,
};
#[cfg(any(feature = "native", feature = "wasm"))]
pub use shared::repl::{Repl, ReplBuilder, ReplMessageIter};
pub use shared::theme::{ActivityStyle, BorderKind, ReplColor, ReplPadding, ReplTheme};
#[cfg(any(feature = "native", feature = "ratzilla"))]
pub use shared::host::{ReplHost, ViewportMode};
#[cfg(any(feature = "native", feature = "ratzilla"))]
pub use shared::render::ReplRenderer;
pub use shared::traits::{ReplDisplay, ReplInput, SharedDisplay};

#[cfg(all(feature = "native", not(feature = "wasm")))]
pub use native::display::{native_display, CrosstermHost, NativeDisplay};

#[cfg(feature = "ratzilla")]
pub use wasm::ratzilla::{ratzilla_display, RatzillaHost};
