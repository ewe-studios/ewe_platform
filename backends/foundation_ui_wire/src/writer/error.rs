use core::fmt;

use crate::{vocab::VocabError, WireError};

use super::ColumnarError;

/// Gives the reason that a frame could not be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    Vocab {
        message_index: usize,
        error: VocabError,
    },
    Columnar {
        message_index: usize,
        error: ColumnarError,
    },
    MissingState {
        message_index: usize,
    },
    StateTooLarge {
        message_index: usize,
        limit: usize,
        actual: usize,
    },
    EmptyMessage {
        message_index: usize,
    },
    TooLarge {
        limit: usize,
        actual: usize,
    },
    /// A column value vector or validity bitmap has the wrong length.
    ColumnLength {
        column: usize,
        expected: u64,
        actual: u64,
    },
    UnknownVocabularyVersion {
        version: u32,
    },
    Wire(WireError),
}

impl From<WireError> for WriteError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vocab {
                message_index,
                error,
            } => {
                write!(
                    formatter,
                    "Message {message_index} has an invalid vocabulary value. {error}"
                )
            }
            Self::Columnar {
                message_index,
                error,
            } => {
                write!(
                    formatter,
                    "Message {message_index} has an invalid columnar batch. {error}"
                )
            }
            Self::MissingState { message_index } => {
                write!(formatter, "Message {message_index} has no state bytes.")
            }
            Self::StateTooLarge {
                message_index,
                limit,
                actual,
            } => {
                write!(formatter, "Message {message_index} has {actual} state bytes and exceeds the {limit}-byte limit.")
            }
            Self::EmptyMessage { message_index } => {
                write!(formatter, "Message {message_index} has no payload.")
            }
            Self::TooLarge { limit, actual } => {
                write!(
                    formatter,
                    "The frame has {actual} bytes and exceeds the {limit}-byte limit."
                )
            }
            Self::ColumnLength {
                column,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "Column {column} has length {actual}. The expected length is {expected}."
                )
            }
            Self::UnknownVocabularyVersion { version } => {
                write!(formatter, "The vocabulary version {version} is unknown.")
            }
            Self::Wire(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Vocab { error, .. } => Some(error),
            Self::Columnar { error, .. } => Some(error),
            Self::Wire(error) => Some(error),
            _ => None,
        }
    }
}
