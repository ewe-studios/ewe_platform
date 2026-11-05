//! Error type for `foundation_sshkit`.
//!
//! **WHY:** SSH orchestration fails in distinct ways: the connection cannot be
//! made, the host key does not match, authentication fails, a channel or
//! command fails, or a file transfer fails. Callers need one trace to carry the
//! operation that failed and the host or path it failed on.
//!
//! **WHAT:** [`SshKitError`] and the [`SshKitResult`] alias.
//!
//! **HOW:** `derive_more` supplies `Display` and `Error`. A `From` impl converts
//! the enum into an errstacks trace at the crate boundary.

use derive_more::{Display, Error};
use foundation_errstacks::ErrorTrace;

/// Result type of the public fallible API.
pub type SshKitResult<T> = Result<T, ErrorTrace<SshKitError>>;

/// SSH operation errors.
#[derive(Debug, Display, Error)]
pub enum SshKitError {
    /// A connection, channel, command or transfer step failed.
    #[display("ssh operation failed: {_0}")]
    Operation(#[error(not(source))] String),

    /// The server's host key did not match `known_hosts`.
    #[display("ssh host key verification failed: {_0}")]
    HostKey(#[error(not(source))] String),
}

impl From<SshKitError> for ErrorTrace<SshKitError> {
    fn from(err: SshKitError) -> Self {
        ErrorTrace::new(err)
    }
}
