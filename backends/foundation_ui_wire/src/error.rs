use alloc::string::String;
use core::fmt;

/// Names the two frame kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKindName {
    Request,
    Message,
}

impl FrameKindName {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Message => "message",
        }
    }
}

/// Reports a failure to read or write a wire buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    TooLarge {
        limit: usize,
        actual: usize,
    },
    TooShort {
        actual: usize,
    },
    WrongIdentifier {
        expected: &'static str,
        found: [u8; 4],
    },
    NoKind,
    WrongKind {
        expected: FrameKindName,
        found: FrameKindName,
    },
    Invalid {
        detail: String,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit, actual } => {
                write!(
                    formatter,
                    "The buffer has {actual} bytes and exceeds the {limit}-byte limit."
                )
            }
            Self::TooShort { actual } => {
                write!(
                    formatter,
                    "The buffer has {actual} bytes and lacks a complete header."
                )
            }
            Self::WrongIdentifier { expected, found } => {
                write!(
                    formatter,
                    "Expected the {expected} identifier but found {found:?}."
                )
            }
            Self::NoKind => formatter.write_str("The frame has no kind."),
            Self::WrongKind { expected, found } => write!(
                formatter,
                "Expected a {} frame but found a {} frame.",
                expected.as_str(),
                found.as_str(),
            ),
            Self::Invalid { detail } => {
                write!(formatter, "The verifier rejected the buffer. {detail}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WireError {}
