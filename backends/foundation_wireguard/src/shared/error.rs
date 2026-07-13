//! Error types for `foundation_wireguard`.

use derive_more::{Display, Error, From};

/// Convenience alias for results in this crate.
pub type WgResult<T> = Result<T, WgError>;

/// WHY: One error type spanning key parsing, tunnel crypto, and transport so callers
/// handle a single `Result`.
///
/// WHAT: All failure modes surfaced by `foundation_wireguard`.
///
/// HOW: `derive_more` supplies `Display`/`Error`/`From`; each variant carries context.
#[derive(Debug, Display, Error, From)]
pub enum WgError {
    /// A seed was the wrong length (must be 16 or 32 bytes).
    #[display("invalid seed length {_0}: must be 16 (128-bit) or 32 (256-bit) bytes")]
    #[from(ignore)]
    InvalidSeedLength(#[error(not(source))] usize),

    /// A key string could not be parsed as hex-64 or base64.
    #[display("invalid key encoding: {_0}")]
    #[from(ignore)]
    InvalidKey(#[error(not(source))] String),

    /// A bootstrap token was malformed, truncated, or failed its CRC check.
    #[display("invalid bootstrap token: {_0}")]
    #[from(ignore)]
    InvalidToken(#[error(not(source))] String),

    /// Base64 decoding failed.
    #[display("base64 decode error: {_0}")]
    Base64(base64::DecodeError),

    /// Hex decoding failed.
    #[display("hex decode error: {_0}")]
    Hex(hex::FromHexError),

    /// The OS random source failed.
    #[display("randomness unavailable: {_0}")]
    Random(getrandom::Error),

    /// The WireGuard Noise layer reported an error.
    #[display("wireguard protocol error: {_0}")]
    #[from(ignore)]
    Protocol(#[error(not(source))] String),

    /// A transport / I/O error.
    #[display("io error: {_0}")]
    Io(std::io::Error),

    /// Configuration parsing or validation error.
    #[display("config error: {_0}")]
    #[from(ignore)]
    Config(#[error(not(source))] String),
}
