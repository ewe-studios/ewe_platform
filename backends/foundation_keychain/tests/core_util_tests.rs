//! `core::util` tests (moved from `src/core/util.rs`).

use foundation_keychain::core::util::validate_icon_domain;

#[test]
fn rejects_localhost() {
    assert!(!validate_icon_domain("https://localhost/icon.png"));
    assert!(!validate_icon_domain("http://127.0.0.1:8080/favicon.ico"));
    assert!(!validate_icon_domain("https://10.0.0.1/icon"));
    assert!(!validate_icon_domain("https://192.168.1.1/icon"));
}

#[test]
fn allows_public_urls() {
    assert!(validate_icon_domain("https://example.com/icon.png"));
    assert!(validate_icon_domain("https://github.com/favicon.ico"));
}
