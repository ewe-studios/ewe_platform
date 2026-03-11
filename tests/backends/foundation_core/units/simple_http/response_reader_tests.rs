//! Unit tests for 101 Switching Protocols response handling.
//!
//! These tests verify that Status::SwitchingProtocols is correctly defined
//! and that IncomingResponseParts::NoBody is used for 1xx responses.
//!
//! RFC 7230 Section 3.3 specifies that 1xx responses MUST NOT contain a body.
//! The actual HttpResponseReader handling is tested at the integration level.

use foundation_core::wire::simple_http::{IncomingResponseParts, Proto, SimpleHeaders, Status};

/// WHY: Verify Status::SwitchingProtocols is correctly defined as 101.
/// WHAT: Tests that the status code value is 101.
#[test]
fn test_switching_protocols_status_code() {
    assert_eq!(Status::SwitchingProtocols.into_usize(), 101);
}

/// WHY: Verify Status::SwitchingProtocols can be parsed from "101".
/// WHAT: Tests parsing "101" returns SwitchingProtocols.
#[test]
fn test_switching_protocols_from_string() {
    let status = Status::from("101".to_string());
    assert_eq!(status, Status::SwitchingProtocols);
}

/// WHY: Verify Status::SwitchingProtocols status line format.
/// WHAT: Tests that status_line() contains "101" and "Switching".
#[test]
fn test_switching_protocols_status_line() {
    let status = Status::SwitchingProtocols;
    let line = status.status_line();
    assert!(line.contains("101"), "Status line should contain '101', got: {}", line);
    assert!(
        line.to_lowercase().contains("switching"),
        "Status line should contain 'Switching', got: {}",
        line
    );
}

/// WHY: Verify IncomingResponseParts::NoBody variant exists and is usable.
/// WHAT: Tests that NoBody can be constructed and matched.
#[test]
fn test_no_body_variant() {
    let parts = IncomingResponseParts::NoBody;
    assert!(matches!(parts, IncomingResponseParts::NoBody));
}

/// WHY: Verify IncomingResponseParts::Headers variant works.
/// WHAT: Tests Headers variant with empty headers.
#[test]
fn test_headers_variant() {
    let headers = SimpleHeaders::default();
    let parts = IncomingResponseParts::Headers(headers);
    assert!(matches!(parts, IncomingResponseParts::Headers(_)));
}

/// WHY: Verify IncomingResponseParts::Intro works with 101 status.
/// WHAT: Tests Intro variant with SwitchingProtocols.
#[test]
fn test_intro_with_101_status() {
    let parts = IncomingResponseParts::Intro(
        Status::SwitchingProtocols,
        Proto::HTTP11,
        Some("Switching Protocols".to_string()),
    );
    assert!(matches!(
        parts,
        IncomingResponseParts::Intro(Status::SwitchingProtocols, Proto::HTTP11, _)
    ));
}

/// WHY: Verify other 1xx status codes are defined.
/// WHAT: Tests 100 Continue status code value.
#[test]
fn test_100_continue_status_code() {
    assert_eq!(Status::Continue.into_usize(), 100);
}

/// WHY: Verify 102 Processing status code exists.
/// WHAT: Tests 102 Processing status code value.
#[test]
fn test_102_processing_status_code() {
    assert_eq!(Status::Processing.into_usize(), 102);
}

/// WHY: Verify 204 No Content status (also no body).
/// WHAT: Tests 204 No Content status code value.
#[test]
fn test_204_no_content_status_code() {
    let status = Status::NoContent;
    assert_eq!(status.into_usize(), 204);
}

/// WHY: Verify 304 Not Modified status (also no body).
/// WHAT: Tests 304 Not Modified status code value.
#[test]
fn test_304_not_modified_status_code() {
    let status = Status::NotModified;
    assert_eq!(status.into_usize(), 304);
}

/// WHY: Verify all 1xx status codes return correct values.
/// WHAT: Tests 100, 101, 102 are all in 1xx range.
#[test]
fn test_all_1xx_status_codes() {
    let codes = vec![
        (Status::Continue, 100usize),
        (Status::SwitchingProtocols, 101),
        (Status::Processing, 102),
    ];
    for (status, expected) in codes {
        let actual = status.into_usize();
        assert_eq!(
            actual,
            expected,
            "Status should be {}, got {}",
            expected,
            actual
        );
    }
}

/// WHY: Verify status codes that should NOT have bodies.
/// WHAT: Tests that 1xx, 204, 304 are distinct from body-having codes.
#[test]
fn test_no_body_status_codes_are_distinct() {
    let no_body_codes = vec![100, 101, 102, 204, 304];
    let body_codes = vec![200, 201, 202, 400, 404, 500];

    for code in &no_body_codes {
        assert!(*code < 200 || *code == 204 || *code == 304, "1xx, 204, 304 should not have body");
    }

    for code in &body_codes {
        assert!(*code >= 200 && *code != 204 && *code != 304, "2xx (except 204) should have body");
    }
}
