use std::io;

use crate::shared::traits::{ReplDisplay, ReplInput};

/// Wasm input stub — returns an error since interactive stdin is not available.
/// A real wasm32 implementation would hook into web-sys DOM events or a JS bridge.
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
        mut _history: Option<&mut crate::shared::history::ReplHistory>,
    ) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive input not available on wasm32",
        ))
    }
}
