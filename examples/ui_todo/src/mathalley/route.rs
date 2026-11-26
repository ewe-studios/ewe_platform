//! Share the game navigation command with every Todo host.
use serde::de::{Deserializer, Visitor};
use std::{fmt, io::Cursor};

const OPEN: &str = "open:mathalley";

/// Encode the command that opens the Mathalley view.
///
/// # Errors
/// Return a MessagePack encoding error.
pub fn open_command() -> Result<Vec<u8>, rmp_serde::encode::Error> {
    rmp_serde::to_vec(OPEN)
}

struct OpenCommand;
impl Visitor<'_> for OpenCommand {
    type Value = bool;

    fn expecting(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("a Todo view command string")
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<bool, E> {
        Ok(value == OPEN)
    }
}

/// Return true only for one complete Mathalley open command.
#[must_use]
#[cfg_attr(all(feature = "mobile", not(target_family = "wasm")), uniffi::export)]
#[cfg_attr(
    all(feature = "web", target_arch = "wasm32"),
    wasm_bindgen::prelude::wasm_bindgen
)]
pub fn is_mathalley_open(bytes: Vec<u8>) -> bool {
    let length = bytes.len() as u64;
    let mut reader = rmp_serde::Deserializer::new(Cursor::new(bytes));
    matches!(reader.deserialize_any(OpenCommand), Ok(true)) && reader.position() == length
}
