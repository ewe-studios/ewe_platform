//! Storage helper tests.

#![cfg(feature = "server")]

use foundation_auth::server::storage::{int_to_u32, StorageOpError};

fn is_parse_err(result: &Result<u32, StorageOpError>) -> bool {
    matches!(result, Err(StorageOpError::Parse(_)))
}

#[test]
fn negative_counter_is_a_parse_error() {
    assert!(is_parse_err(&int_to_u32(-1, "counter")));
}

#[test]
fn counter_overflow_is_a_parse_error() {
    assert!(is_parse_err(&int_to_u32(
        i64::from(u32::MAX) + 1,
        "counter"
    )));
}

#[test]
fn int_to_u32_accepts_valid_range() {
    assert_eq!(int_to_u32(0, "counter").expect("zero"), 0);
    assert_eq!(
        int_to_u32(i64::from(u32::MAX), "counter").expect("max"),
        u32::MAX
    );
}
