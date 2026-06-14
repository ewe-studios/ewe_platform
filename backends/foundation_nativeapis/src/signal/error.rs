// Signal error types.

use derive_more::From;

/// Errors that can occur during signal handling setup.
#[derive(Debug, From)]
pub enum SignalError {
    /// I/O error during setup or delivery.
    Io(std::io::Error),
    /// The current platform is not supported.
    #[from(ignore)]
    UnsupportedPlatform,
    /// Failed to register signal handler (OS error).
    #[from(ignore)]
    RegistrationFailed(String),
}

impl std::error::Error for SignalError {}

impl core::fmt::Display for SignalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalError::Io(e) => write!(f, "Io: {e}"),
            SignalError::UnsupportedPlatform => write!(f, "Unsupported platform"),
            SignalError::RegistrationFailed(msg) => write!(f, "Registration failed: {msg}"),
        }
    }
}
