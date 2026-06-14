use std::io;

use derive_more::From;

/// Aggregate error type for all tooling operations.
#[derive(Debug, From)]
pub enum ToolingError {
    /// I/O error during file or process operations.
    Io(io::Error),
    /// File watcher failed to initialize or poll.
    #[from(ignore)]
    Watch(String),
    /// Build command failed (cargo, wasm-pack, etc.).
    #[from(ignore)]
    Build(String),
    /// Binary spawn or kill failed.
    #[from(ignore)]
    Run(String),
    /// Proxy connection or forwarding failed.
    #[from(ignore)]
    Proxy(String),
    /// SSE endpoint failed to write or client disconnected.
    #[from(ignore)]
    Sse(String),
}

impl std::error::Error for ToolingError {}

impl core::fmt::Display for ToolingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolingError::Io(e) => write!(f, "Io: {e}"),
            ToolingError::Watch(m) => write!(f, "Watch: {m}"),
            ToolingError::Build(m) => write!(f, "Build: {m}"),
            ToolingError::Run(m) => write!(f, "Run: {m}"),
            ToolingError::Proxy(m) => write!(f, "Proxy: {m}"),
            ToolingError::Sse(m) => write!(f, "Sse: {m}"),
        }
    }
}
