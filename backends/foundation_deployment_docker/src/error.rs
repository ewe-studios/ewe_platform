//! Error type for the Docker client.
//!
//! WHY: Docker API calls fail in a few distinct ways (transport down, non-2xx
//! response with a `{"message": ...}` body, JSON parse failure, missing socket).
//! Callers need to distinguish these to decide whether to retry, surface, or
//! treat as "not found".
//!
//! WHAT: [`DockerError`] — a `derive_more`-based enum wrapping the generated
//! `ApiError` plus Docker-specific cases.
//!
//! HOW: `From<ApiError>` maps generated request failures; `Display`/`Error` via
//! `derive_more`.

use crate::generated::shared::ApiError;

/// Errors returned by [`crate::client::DockerClient`] operations.
///
/// WHY: Provides a single, matchable error surface over the generated API and
/// the hand-written transport/setup layer.
#[derive(Debug, derive_more::Display)]
pub enum DockerError {
    /// The Docker daemon returned a non-2xx status.
    #[display("docker API error (status {status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Message extracted from the daemon's `{"message": ...}` body, if any.
        message: String,
    },

    /// The request could not be built or sent (transport-level failure).
    #[display("docker transport error: {_0}")]
    Transport(String),

    /// A response body could not be parsed as the expected JSON shape.
    #[display("docker JSON parse error: {_0}")]
    JsonParse(String),

    /// An image build failed. The daemon reports these **inside a 200 response**
    /// — a `{"error": …}` object part-way down the progress stream — so this is
    /// not an [`DockerError::Api`] status failure.
    #[display("docker build failed: {_0}")]
    BuildFailed(String),

    /// The Docker daemon socket could not be resolved or is unavailable.
    #[display("docker unavailable: {_0}")]
    Unavailable(String),

    /// The daemon is reachable but shares no API version with this client: its
    /// oldest supported version is newer than the newest one the client speaks.
    #[display(
        "docker API version mismatch: client speaks up to {client_max}, \
         daemon accepts {server_min}..={server_max}"
    )]
    UnsupportedApiVersion {
        /// The newest API version this client speaks.
        client_max: String,
        /// The daemon's `MinAPIVersion`.
        server_min: String,
        /// The daemon's `ApiVersion`.
        server_max: String,
    },
}

impl std::error::Error for DockerError {}

impl DockerError {
    /// `true` when no daemon could be talked to at all: the socket or host is
    /// missing, refused the connection, or the transport is misconfigured.
    ///
    /// A daemon that answered — with any HTTP status, or a version mismatch —
    /// was reachable, so those are never "unreachable".
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Transport(_) | Self::Unavailable(_))
    }
}

impl From<ApiError> for DockerError {
    /// Map a generated [`ApiError`] into a [`DockerError`], extracting the
    /// daemon's `{"message": ...}` body for `HttpStatus` responses.
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::HttpStatus { code, body, .. } => {
                let message = body
                    .as_deref()
                    .and_then(|b| {
                        serde_json::from_str::<serde_json::Value>(b)
                            .ok()
                            .and_then(|v| {
                                v.get("message")
                                    .and_then(|m| m.as_str())
                                    .map(str::to_string)
                            })
                    })
                    .or(body)
                    .unwrap_or_else(|| "(no body)".to_string());
                DockerError::Api { status: code, message }
            }
            ApiError::RequestBuildFailed(m) | ApiError::RequestSendFailed(m) => {
                DockerError::Transport(m)
            }
            ApiError::ParseFailed(m) => DockerError::JsonParse(m),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(code: u16, body: Option<&str>) -> ApiError {
        ApiError::HttpStatus {
            code,
            headers: foundation_netio::shared::http::SimpleHeaders::new(),
            body: body.map(str::to_string),
        }
    }

    #[test]
    fn a_4xx_is_an_api_error_not_an_unreachable_daemon() {
        // What an older daemon answers a too-new API version with.
        let e = DockerError::from(status(
            400,
            Some(r#"{"message":"client version 1.53 is too new. Maximum supported API version is 1.48"}"#),
        ));
        match &e {
            DockerError::Api { status, message } => {
                assert_eq!(*status, 400);
                assert!(message.contains("too new"), "message: {message}");
            }
            other => panic!("expected Api, got {other:?}"),
        }
        assert!(!e.is_unreachable());
    }

    #[test]
    fn an_error_status_without_a_body_is_still_an_api_error() {
        let e = DockerError::from(status(400, None));
        assert!(matches!(e, DockerError::Api { status: 400, ref message } if message == "(no body)"));
        assert!(!e.is_unreachable());
    }

    #[test]
    fn a_5xx_is_an_api_error() {
        let e = DockerError::from(status(500, Some("plain text")));
        assert!(matches!(e, DockerError::Api { status: 500, ref message } if message == "plain text"));
        assert!(!e.is_unreachable());
    }

    #[test]
    fn a_failed_send_is_a_transport_error_and_unreachable() {
        let e = DockerError::from(ApiError::RequestSendFailed("connection refused".to_string()));
        assert!(matches!(e, DockerError::Transport(_)));
        assert!(e.is_unreachable());
        assert!(DockerError::Unavailable("no socket".to_string()).is_unreachable());
    }

    #[test]
    fn a_version_mismatch_is_not_unreachable() {
        let e = DockerError::UnsupportedApiVersion {
            client_max: "1.53".into(),
            server_min: "1.55".into(),
            server_max: "1.60".into(),
        };
        assert!(!e.is_unreachable());
        assert!(e.to_string().contains("1.55..=1.60"), "{e}");
    }
}
