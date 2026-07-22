//! Wasm display stub — there is no terminal to draw a box in, so every render
//! is reported to the browser console instead.
//!
//! WHY: the crate compiles for wasm32 so callers can share REPL logic between a
//! terminal build and a browser build. A browser has no cursor addressing, so
//! the box, its padding and the caret have nowhere to go.
//!
//! WHAT: each trait method logs what a terminal would have drawn.

use crate::shared::activity::ActivityView;
use crate::shared::layout::InputView;
use crate::shared::theme::ReplTheme;
use crate::shared::traits::ReplDisplay;

macro_rules! log_wasm {
    ($($arg:tt)*) => {{
        #[cfg(feature = "wasm")]
        web_sys::console::log_1(&format!($($arg)*).into());
        #[cfg(not(feature = "wasm"))]
        tracing::info!($($arg)*);
    }};
}

/// Wasm display stub — logs to `console.log`.
#[derive(Debug)]
pub struct WasmDisplay {
    theme: ReplTheme,
}

impl core::fmt::Display for WasmDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "WasmDisplay({})", self.theme)
    }
}

impl WasmDisplay {
    /// Build the stub with the theme a real surface would draw in.
    #[must_use]
    pub fn new(theme: ReplTheme) -> Self {
        Self { theme }
    }
}

impl ReplDisplay for WasmDisplay {
    fn print_banner(&mut self, banner: &str) {
        log_wasm!("BANNER: {banner}");
    }

    fn render_input(&mut self, view: &InputView<'_>) {
        log_wasm!("INPUT: {}{}", view.prompt, view.buffer);
    }

    fn finish_input(&mut self, view: &InputView<'_>) {
        log_wasm!("SUBMIT: {}", view.buffer);
    }

    fn print_response(&mut self, response: &str) {
        for line in response.lines() {
            log_wasm!("  {line}");
        }
    }

    fn print_error(&mut self, msg: &str) {
        log_wasm!("ERROR: {msg}");
    }

    fn render_activity(&mut self, view: &ActivityView<'_>) {
        log_wasm!("BUSY: {view}");
    }

    fn stream_push(&mut self, text: &str) {
        log_wasm!("{text}");
    }

    fn end_activity(&mut self) {
        log_wasm!("BUSY: done");
    }

    fn clear_screen(&mut self) {
        log_wasm!("CLEAR");
    }

    fn shutdown(&mut self) {}
}
