//! URI parsing error types.

use std::fmt;

/// Error returned when URI parsing fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidUri {
    message: String,
}

impl InvalidUri {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InvalidUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI: {}", self.message)
    }
}

impl std::error::Error for InvalidUri {}

/// Error returned when building a URI from parts fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidUriParts {
    message: String,
}

impl InvalidUriParts {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InvalidUriParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI parts: {}", self.message)
    }
}

impl std::error::Error for InvalidUriParts {}
