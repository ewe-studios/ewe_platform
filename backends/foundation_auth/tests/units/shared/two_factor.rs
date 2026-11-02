//! Two-factor helper tests.

use foundation_auth::shared::two_factor::constant_time_eq;

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq(b"hello", b"hello"));
    assert!(!constant_time_eq(b"hello", b"world"));
    assert!(!constant_time_eq(b"short", b"longer"));
}
