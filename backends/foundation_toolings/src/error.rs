use std::io;

use derive_more::{Display, Error, From};
use foundation_errstacks::ErrorTrace;

/// Result type of the crate's public fallible API.
pub type ToolingResult<T> = Result<T, ErrorTrace<ToolingError>>;

/// Aggregate error type for all tooling operations.
#[derive(Debug, Display, Error, From)]
pub enum ToolingError {
    /// I/O error during file or process operations.
    #[display("Io: {_0}")]
    Io(io::Error),
    /// File watcher failed to initialize or poll.
    #[display("Watch: {_0}")]
    #[from(ignore)]
    Watch(#[error(not(source))] String),
    /// Build command failed (cargo, wasm-pack, etc.).
    #[display("Build: {_0}")]
    #[from(ignore)]
    Build(#[error(not(source))] String),
    /// The background job pool is missing or closed, so a build cannot start.
    #[display("Background job unavailable: {_0}")]
    #[from(ignore)]
    BackgroundJob(#[error(not(source))] String),
    /// Binary spawn or kill failed.
    #[display("Run: {_0}")]
    #[from(ignore)]
    Run(#[error(not(source))] String),
    /// Proxy connection or forwarding failed.
    #[display("Proxy: {_0}")]
    #[from(ignore)]
    Proxy(#[error(not(source))] String),
    /// SSE endpoint failed to write or client disconnected.
    #[display("Sse: {_0}")]
    #[from(ignore)]
    Sse(#[error(not(source))] String),
}

impl From<ToolingError> for ErrorTrace<ToolingError> {
    fn from(err: ToolingError) -> Self {
        ErrorTrace::new(err)
    }
}
