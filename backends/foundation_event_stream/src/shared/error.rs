//! The subscription error codes.
//!
//! Every code maps to one Hypertext Transfer Protocol (HTTP) status in the adapter. The codes and
//! their status codes follow protocol section 6 and 7.

use core::fmt;

/// The machine-readable subscription error code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriptionErrorCode {
    /// The request body or a field is not valid.
    InvalidRequest,
    /// The subscription does not exist.
    SubscriptionNotFound,
    /// The subscription exists with a different configuration.
    SubscriptionAlreadyExists,
    /// The webhook uniform resource locator (URL) is not safe to call.
    WebhookUrlRejected,
    /// The bearer token is not valid for this subscription.
    TokenInvalid,
    /// The bearer token is valid but its lifetime ended.
    TokenExpired,
    /// The wake generation or the wake identifier is stale.
    Fenced,
    /// Another worker holds the lease.
    AlreadyClaimed,
    /// The subscription has no pending work.
    NoPendingWork,
    /// An acknowledged offset is unknown, behind the cursor or past the tail.
    InvalidOffset,
}

impl SubscriptionErrorCode {
    /// The wire text of the code.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::SubscriptionNotFound => "SUBSCRIPTION_NOT_FOUND",
            Self::SubscriptionAlreadyExists => "SUBSCRIPTION_ALREADY_EXISTS",
            Self::WebhookUrlRejected => "WEBHOOK_URL_REJECTED",
            Self::TokenInvalid => "TOKEN_INVALID",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::Fenced => "FENCED",
            Self::AlreadyClaimed => "ALREADY_CLAIMED",
            Self::NoPendingWork => "NO_PENDING_WORK",
            Self::InvalidOffset => "INVALID_OFFSET",
        }
    }

    /// The Hypertext Transfer Protocol (HTTP) status of the code.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::InvalidRequest | Self::WebhookUrlRejected => 400,
            Self::SubscriptionNotFound => 404,
            Self::TokenInvalid | Self::TokenExpired => 401,
            Self::SubscriptionAlreadyExists
            | Self::Fenced
            | Self::AlreadyClaimed
            | Self::NoPendingWork
            | Self::InvalidOffset => 409,
        }
    }
}

/// A refused subscription operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionError {
    /// The machine-readable code.
    pub code: SubscriptionErrorCode,
    /// The human-readable message.
    pub message: String,
    /// The worker that holds the lease, for [`SubscriptionErrorCode::AlreadyClaimed`].
    pub current_holder: Option<String>,
    /// The current generation, for [`SubscriptionErrorCode::AlreadyClaimed`].
    pub generation: Option<u64>,
}

impl SubscriptionError {
    /// Build an error from a code and a message.
    #[must_use]
    pub fn new(code: SubscriptionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            current_holder: None,
            generation: None,
        }
    }

    /// Build an [`SubscriptionErrorCode::InvalidRequest`] error.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(SubscriptionErrorCode::InvalidRequest, message)
    }

    /// Build an [`SubscriptionErrorCode::SubscriptionNotFound`] error.
    #[must_use]
    pub fn not_found() -> Self {
        Self::new(
            SubscriptionErrorCode::SubscriptionNotFound,
            "Subscription not found",
        )
    }

    /// Build an [`SubscriptionErrorCode::Fenced`] error.
    #[must_use]
    pub fn fenced() -> Self {
        Self::new(SubscriptionErrorCode::Fenced, "Wake generation is stale")
    }

    /// Attach the current lease holder and generation.
    #[must_use]
    pub fn with_holder(mut self, holder: impl Into<String>, generation: u64) -> Self {
        self.current_holder = Some(holder.into());
        self.generation = Some(generation);
        self
    }

    /// The Hypertext Transfer Protocol (HTTP) status of this error.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.code.status()
    }
}

impl fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl core::error::Error for SubscriptionError {}
