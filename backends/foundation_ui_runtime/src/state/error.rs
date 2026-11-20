use core::fmt;

/// Describes the first check that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    MissingSession,
    BadKey,
    BodyTooLarge { limit: usize, actual: usize },
    Malformed { detail: String },
    UnknownKey,
    BadTag,
    Expired,
    SessionMismatch,
    ComponentMismatch,
    Decrypt,
    RateLimited,
    MissingEnv { what: &'static str },
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSession => formatter.write_str("The session has no identifier."),
            Self::BadKey => {
                formatter.write_str("The state key is invalid or its identifier is in use.")
            }
            Self::BodyTooLarge { limit, actual } => {
                write!(
                    formatter,
                    "The state has {actual} body bytes and exceeds the {limit}-byte limit."
                )
            }
            Self::Malformed { detail } => {
                write!(formatter, "The state has an invalid format. {detail}")
            }
            Self::UnknownKey => formatter.write_str("The state key identifier is unknown."),
            Self::BadTag => formatter.write_str("The state tag is invalid."),
            Self::Expired => formatter.write_str("The state is outside its time window."),
            Self::SessionMismatch => {
                formatter.write_str("The state belongs to a different session.")
            }
            Self::ComponentMismatch => {
                formatter.write_str("The state has a different component or format version.")
            }
            Self::Decrypt => formatter.write_str("The state cipher operation failed."),
            Self::RateLimited => {
                formatter.write_str("The request is blocked after too many failures.")
            }
            Self::MissingEnv { what } => write!(formatter, "The context has no usable {what}."),
        }
    }
}

impl std::error::Error for StateError {}

pub(super) fn malformed(detail: impl Into<String>) -> StateError {
    StateError::Malformed {
        detail: detail.into(),
    }
}
