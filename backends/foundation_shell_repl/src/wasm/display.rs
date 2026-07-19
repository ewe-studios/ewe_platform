macro_rules! log_wasm {
    ($($arg:tt)*) => {{
        #[cfg(feature = "wasm")]
        web_sys::console::log_1(&format!($($arg)*).into());
        #[cfg(not(feature = "wasm"))]
        eprintln!($($arg)*);
    }};
}

use crate::shared::traits::ReplDisplay;

/// Wasm display stub — logs to console.log / a DOM element.
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

    fn replace_line(&mut self, content: &str, prompt: &str) {
        log_wasm!("REPLACE: {prompt}{content}");
    }
}
