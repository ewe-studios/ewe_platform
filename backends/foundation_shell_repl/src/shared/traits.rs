/// Reads keystrokes and assembles multiline input.
pub trait ReplInput {
    fn new() -> Self
    where
        Self: Sized;

    /// Read one complete message from the terminal.
    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
    ) -> std::io::Result<String>;

    /// Check if the last read was EOF (Ctrl+D or equivalent).
    fn eof(&self) -> bool;
}

/// Renders prompts, responses, and cursor movements.
pub trait ReplDisplay {
    fn new() -> Self
    where
        Self: Sized;

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
