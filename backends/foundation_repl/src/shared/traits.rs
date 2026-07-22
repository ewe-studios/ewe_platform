//! Platform-agnostic I/O traits.
//!
//! [`ReplInput`] reads keystrokes and assembles a message.
//! [`ReplDisplay`] draws the input box and everything printed above it.
//! Both are implemented by the native (ratatui + crossterm) and wasm backends.

use crate::shared::layout::InputView;
use crate::shared::theme::ReplTheme;

/// Reads keystrokes and assembles multiline input.
pub trait ReplInput {
    /// Create the backend's reader.
    fn new() -> Self
    where
        Self: Sized;

    /// Read one complete message from the terminal.
    ///
    /// Redraws `display` after every edit, and leaves the finished input on
    /// screen before returning.
    ///
    /// # Errors
    /// Returns [`std::io::ErrorKind::Interrupted`] when the user presses
    /// Ctrl+C, and any error the terminal itself reports.
    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
        history: Option<&mut super::history::ReplHistory>,
    ) -> std::io::Result<String>;

    /// Whether the last read ended in EOF (Ctrl+D or equivalent).
    fn eof(&self) -> bool;
}

/// Draws the input box, and the banner, responses and errors above it.
///
/// WHY: the input area is redrawn in full on every keystroke rather than
/// patched in place — that is what keeps the prompt, the box and the caret
/// consistent with the buffer no matter how the text wrapped.
///
/// WHAT: implementors own whatever terminal state the redraw needs, and are
/// expected to restore the terminal when dropped.
pub trait ReplDisplay {
    /// Create the backend's renderer with the theme it should draw in.
    fn new(theme: ReplTheme) -> Self
    where
        Self: Sized;

    /// Print the startup banner above the input area.
    fn print_banner(&mut self, banner: &str);

    /// Draw the input box for the current buffer and caret position.
    ///
    /// Called once per edit. Implementations must render the whole area, not a
    /// delta, and must leave the caret visible at `view.cursor`.
    fn render_input(&mut self, view: &InputView<'_>);

    /// Commit the finished input, moving it above the live input area.
    ///
    /// Called once when the user submits, so the message stays on screen while
    /// its response is printed beneath it.
    fn finish_input(&mut self, view: &InputView<'_>);

    /// Print a response above the input area.
    fn print_response(&mut self, response: &str);

    /// Print an error above the input area.
    fn print_error(&mut self, msg: &str);

    /// Clear the screen, keeping the input area alive.
    fn clear_screen(&mut self);

    /// Restore the terminal to the state it was in before the REPL started.
    ///
    /// Called when the REPL is dropped. Must be safe to call more than once.
    fn shutdown(&mut self);
}
