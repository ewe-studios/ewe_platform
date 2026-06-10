//! WHY: The CLI and build tooling must handle `.wat` (text) alongside `.wasm`
//! (binary) — feature 15's net-new extension over the wasmbin port, which is
//! binary-only.
//!
//! WHAT: [`from_wat`] (WAT text → typed [`Module`]) and [`to_wat`] (typed
//! [`Module`] → WAT text), plus [`WatError`].
//!
//! HOW: The WAT GRAMMAR rides the `wat` crate and printing rides `wasmprinter`
//! (both small, isolated, and opt-in behind the `wat` feature — the F14 boundary
//! rules; decision 031 prefers owned code, and an owned printer over the typed
//! model is the recorded follow-up). Conversion goes through the binary form, so
//! everything still flows through our typed model: WAT → binary → `Module`, and
//! `Module` → binary → WAT.

use super::io::DecodeError;
use super::Module;
use thiserror::Error;

/// WAT conversion error.
#[derive(Error, Debug)]
pub enum WatError {
    /// The WAT text failed to parse (syntax or validation).
    #[error(transparent)]
    Parse(#[from] wat::Error),

    /// The binary form failed to decode into the typed model.
    #[error(transparent)]
    Decode(#[from] DecodeError),

    /// The typed model failed to re-encode to binary.
    #[error(transparent)]
    Encode(#[from] std::io::Error),

    /// The binary form failed to print as WAT.
    #[error("printing WAT: {0}")]
    Print(#[source] anyhow_to_io::PrintError),
}

/// `wasmprinter` reports errors through `anyhow`; capture them as a plain
/// message so this crate doesn't take an `anyhow` dependency.
pub mod anyhow_to_io {
    /// Stringified `wasmprinter` failure.
    #[derive(Debug)]
    pub struct PrintError(pub String);

    impl std::fmt::Display for PrintError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl std::error::Error for PrintError {}
}

/// Parse WAT (or WAT-flavored `.wat`/`.wast` module text) into the typed model.
///
/// # Errors
/// [`WatError::Parse`] for grammar errors; [`WatError::Decode`] if the produced
/// binary doesn't decode (would indicate a `wat`-crate/model mismatch).
pub fn from_wat(text: &str) -> Result<Module, WatError> {
    let bytes = wat::parse_str(text)?;
    Ok(Module::decode_from(bytes.as_slice())?)
}

/// Pretty-print the typed model as WAT text.
///
/// # Errors
/// [`WatError::Encode`] if the model fails to serialize; [`WatError::Print`] if
/// the binary fails to print (would indicate an invalid module).
pub fn to_wat(module: &Module) -> Result<String, WatError> {
    let bytes = module.encode_into(Vec::new())?;
    wasmprinter::print_bytes(&bytes)
        .map_err(|err| WatError::Print(anyhow_to_io::PrintError(format!("{err:#}"))))
}
