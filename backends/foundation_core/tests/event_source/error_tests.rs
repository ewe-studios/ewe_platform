//! Unit tests for `event_source` error module.
//!
//! Tests `EventSourceError` Display implementation.

use foundation_core::wire::event_source::EventSourceError;

#[test]
fn test_error_display() {
    assert_eq!(
        format!("{}", EventSourceError::ConnectionClosed),
        "Connection closed"
    );
    assert_eq!(
        format!("{}", EventSourceError::Timeout),
        "Connection timeout"
    );
    assert_eq!(
        format!("{}", EventSourceError::ParseError("bad data".to_string())),
        "Parse error: bad data"
    );
}
