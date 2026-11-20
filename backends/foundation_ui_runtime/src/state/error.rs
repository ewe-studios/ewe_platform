use core::fmt;

/// A failure to sign state or verify the supplied bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    MissingSigner,
    BadKey,
    InvalidLifetime,
    BodyTooLarge { limit: usize, actual: usize },
    Malformed { detail: String },
    UnknownKey,
    BadTag,
    Expired,
    MissingEnv { what: &'static str },
    Encryption,
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSigner => formatter.write_str("The context has no state signer."),
            Self::BadKey => {
                formatter.write_str("The state key identifier is zero or already in use.")
            }
            Self::InvalidLifetime => formatter.write_str("The state lifetime must be positive."),
            Self::BodyTooLarge { limit, actual } => write!(
                formatter,
                "The state has {actual} body bytes and exceeds the {limit}-byte limit."
            ),
            Self::Malformed { detail } => {
                write!(formatter, "The state has an invalid format. {detail}")
            }
            Self::UnknownKey => formatter.write_str("The state key identifier is unknown."),
            Self::BadTag => formatter.write_str("The state failed authentication."),
            Self::Expired => formatter.write_str("The state is outside its time window."),
            Self::MissingEnv { what } => write!(formatter, "The context has no usable {what}."),
            Self::Encryption => formatter.write_str("The state encryption failed."),
        }
    }
}

impl std::error::Error for StateError {}
