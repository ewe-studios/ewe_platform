/// Error types for FD registration.

use std::io;

/// Error when FdRegistration fails.
pub enum FdRegistrationError<T: std::os::unix::io::AsRawFd> {
    /// The fd could not be registered with the poll selector.
    Registration(io::Error),
    /// Failed to register, returns the original value so caller can handle it.
    Failed { error: io::Error, inner: T },
}

impl<T: std::os::unix::io::AsRawFd> std::fmt::Debug for FdRegistrationError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FdRegistrationError::Registration(e) => {
                f.debug_tuple("Registration").field(e).finish()
            }
            FdRegistrationError::Failed { error, .. } => {
                f.debug_struct("Failed").field("error", error).finish()
            }
        }
    }
}

impl<T: std::os::unix::io::AsRawFd> std::fmt::Display for FdRegistrationError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FdRegistrationError::Registration(e) => {
                write!(f, "failed to register fd with poll selector: {}", e)
            }
            FdRegistrationError::Failed { error, .. } => {
                write!(f, "failed to register fd with poll selector: {}", error)
            }
        }
    }
}

impl<T: std::os::unix::io::AsRawFd> std::error::Error for FdRegistrationError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FdRegistrationError::Registration(e) => Some(e),
            FdRegistrationError::Failed { error, .. } => Some(error),
        }
    }
}

/// Generic registration error (without the inner type).
#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("failed to register fd with poll selector: {0}")]
    Registration(#[from] io::Error),
    #[error("fd is already registered with this selector")]
    AlreadyRegistered,
}
