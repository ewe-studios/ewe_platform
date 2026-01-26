//! Response intro wrapper for HTTP client.
//!
//! This module provides `ResponseIntro` wrapper that holds the initial
//! response information from `IncomingResponseParts::Intro`.

use crate::wire::simple_http::{Proto, Status};

/// HTTP response intro (status line).
///
/// This wrapper holds the initial response parsing result:
/// status code, protocol version, and optional reason phrase.
///
/// Corresponds to `IncomingResponseParts::Intro(Status, Proto, Option<String>)`.
#[derive(Debug, Clone)]
pub struct ResponseIntro {
    pub status: Status,
    pub proto: Proto,
    pub reason: Option<String>,
}

impl From<(Status, Proto, Option<String>)> for ResponseIntro {
    fn from((status, proto, reason): (Status, Proto, Option<String>)) -> Self {
        Self {
            status,
            proto,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Verify ResponseIntro::from converts tuple correctly
    /// WHAT: Tests that From trait creates ResponseIntro from tuple
    #[test]
    fn test_response_intro_from_tuple() {
        let intro = ResponseIntro::from((Status::OK, Proto::HTTP11, Some("OK".to_string())));
        assert!(matches!(intro.status, Status::OK));
        assert!(matches!(intro.proto, Proto::HTTP11));
        assert_eq!(intro.reason, Some("OK".to_string()));
    }

    /// WHY: Verify ResponseIntro::from handles None reason
    /// WHAT: Tests that None reason is preserved
    #[test]
    fn test_response_intro_from_tuple_no_reason() {
        let intro = ResponseIntro::from((Status::OK, Proto::HTTP11, None));
        assert!(matches!(intro.status, Status::OK));
        assert!(matches!(intro.proto, Proto::HTTP11));
        assert_eq!(intro.reason, None);
    }

    /// WHY: Verify ResponseIntro holds all status codes
    /// WHAT: Tests various status codes
    #[test]
    fn test_response_intro_various_status() {
        let intro = ResponseIntro::from((
            Status::NotFound,
            Proto::HTTP11,
            Some("Not Found".to_string()),
        ));
        assert!(matches!(intro.status, Status::NotFound));

        let intro2 = ResponseIntro::from((Status::InternalServerError, Proto::HTTP11, None));
        assert!(matches!(intro2.status, Status::InternalServerError));
    }

    /// WHY: Verify ResponseIntro holds all protocols
    /// WHAT: Tests various protocol versions
    #[test]
    fn test_response_intro_various_proto() {
        let intro = ResponseIntro::from((Status::OK, Proto::HTTP10, None));
        assert!(matches!(intro.proto, Proto::HTTP10));

        let intro2 = ResponseIntro::from((Status::OK, Proto::HTTP20, None));
        assert!(matches!(intro2.proto, Proto::HTTP20));
    }

    /// WHY: Verify ResponseIntro fields are public
    /// WHAT: Tests that status, proto, reason can be accessed directly
    #[test]
    fn test_response_intro_public_fields() {
        let intro = ResponseIntro {
            status: Status::OK,
            proto: Proto::HTTP11,
            reason: Some("OK".to_string()),
        };
        assert!(matches!(intro.status, Status::OK));
        assert!(matches!(intro.proto, Proto::HTTP11));
        assert_eq!(intro.reason, Some("OK".to_string()));
    }
}
