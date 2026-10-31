//! Handler helper tests.

#![cfg(feature = "server")]

use foundation_auth::server::handlers::core::header_name_eq;
use foundation_netio::shared::http::SimpleHeader;

#[test]
fn header_name_eq_ignores_case_and_debug_format() {
    // A known variant matches its canonical lowercase name, any casing.
    assert!(header_name_eq(&SimpleHeader::AUTHORIZATION, "authorization"));
    assert!(header_name_eq(&SimpleHeader::AUTHORIZATION, "AUTHORIZATION"));
    assert!(header_name_eq(&SimpleHeader::AUTHORIZATION, "Authorization"));

    // A custom header matches by its own name, not its `Custom(...)` Debug form.
    assert!(header_name_eq(
        &SimpleHeader::Custom("Authorization".into()),
        "authorization"
    ));

    // A hyphenated known variant matches by its wire name, not its Debug form
    // (`X_FRAME_OPTIONS` has an underscore).
    assert!(header_name_eq(&SimpleHeader::X_FRAME_OPTIONS, "x-frame-options"));
    assert!(header_name_eq(&SimpleHeader::X_FRAME_OPTIONS, "X-Frame-Options"));

    // Non-matching names are rejected.
    assert!(!header_name_eq(&SimpleHeader::AUTHORIZATION, "content-type"));
    assert!(!header_name_eq(&SimpleHeader::X_FRAME_OPTIONS, "x-frame-option"));
}
