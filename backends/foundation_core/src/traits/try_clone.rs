//! A clone that is allowed to fail.
//!
//! WHY: `Clone` is all-or-nothing. A type with even one non-clonable variant —
//! a handle to a single-shot stream, a file descriptor, a moved-once resource —
//! cannot implement it at all. Callers who need a copy are then left to
//! hand-roll a `match` at every call site, and they are free to disagree about
//! which variants are safe. That disagreement is a real defect class: one such
//! call site in the SSE reconnect path dropped a request body entirely, sending
//! a bodyless POST that the server rejected with 400.
//!
//! WHAT: [`TryClone`], a fallible clone whose error explains the refusal, and
//! [`TryCloneError`] as a ready-made error for the common "this is backed by a
//! single-shot source" case. Implementors are free to use their own error type.
//!
//! HOW: implement `TryClone` on the type that knows which of its states can be
//! duplicated, so that judgement lives in one place instead of at every caller.
//!
//! ```
//! use foundation_core::traits::{TryClone, TryCloneError};
//!
//! enum Payload {
//!     Owned(Vec<u8>),
//!     Streaming,
//! }
//!
//! impl TryClone for Payload {
//!     type Error = TryCloneError;
//!
//!     fn try_clone(&self) -> Result<Self, Self::Error> {
//!         match self {
//!             Self::Owned(bytes) => Ok(Self::Owned(bytes.clone())),
//!             Self::Streaming => Err(TryCloneError::NotReplayable("Payload::Streaming")),
//!         }
//!     }
//! }
//!
//! assert!(Payload::Owned(vec![1, 2]).try_clone().is_ok());
//! assert!(Payload::Streaming.try_clone().is_err());
//! ```

/// Why a value could not be cloned.
///
/// WHY: a bare `Option` cannot distinguish "there was nothing to clone" from
/// "this refuses to be cloned", and the two call for opposite handling. Naming
/// the offending variant also turns an otherwise mystifying log line into one
/// that points at the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TryCloneError {
    /// The value wraps a source that can only be consumed once, so any copy
    /// would be a second handle to the same exhausted stream.
    ///
    /// Carries the name of the refusing variant or type, for diagnostics.
    NotReplayable(&'static str),
}

impl core::fmt::Display for TryCloneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotReplayable(what) => {
                write!(f, "{what} is a single-shot stream and cannot be replayed")
            }
        }
    }
}

impl std::error::Error for TryCloneError {}

/// A clone that is allowed to fail.
///
/// Implement this when some — but not all — states of a type can be duplicated.
/// See the [module docs](self) for the rationale and an example.
pub trait TryClone: Sized {
    /// Why the clone was refused.
    type Error;

    /// Clone the value, or explain why it cannot be cloned.
    ///
    /// Implementations must leave `self` usable: callers may clone the same
    /// value repeatedly (a reconnect loop replaying a request body does exactly
    /// that), and a `take`-style implementation would succeed once and then
    /// silently start yielding nothing.
    ///
    /// # Errors
    ///
    /// Returns an error when the value cannot be duplicated — typically because
    /// it is backed by a single-shot source.
    fn try_clone(&self) -> Result<Self, Self::Error>;
}
