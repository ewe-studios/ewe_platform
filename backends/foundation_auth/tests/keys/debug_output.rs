//! No key bytes appear in `Debug` output.

use foundation_auth::keys::{KeySource, PrivateKey};

use super::common;

/// The start of the base64 body of a version 1 Ed25519 PKCS#8 key. If it
/// appears in output, the DER leaked.
const DER_PREFIX_B64: &str = "MC4CAQAwBQYDK2Vw";

fn assert_no_key_material(text: &str) {
    assert!(!text.contains("BEGIN"), "a PEM marker leaked: {text}");
    assert!(!text.contains("PRIVATE"), "a PEM marker leaked: {text}");
    assert!(!text.contains(DER_PREFIX_B64), "the key DER leaked: {text}");
}

#[test]
fn private_key_debug_is_redacted() {
    let key = PrivateKey::parse(common::ED25519_PKCS8.as_bytes()).expect("key");
    assert_no_key_material(&format!("{key:?}"));
}

#[test]
fn key_source_debug_is_redacted() {
    let inline = KeySource::inline(common::ED25519_PKCS8.as_bytes());
    assert_no_key_material(&format!("{inline:?}"));
    assert!(format!("{inline:?}").contains("<redacted>"));

    let file = KeySource::File("/keys/signing.pem".to_string());
    assert_eq!(
        format!("{file:?}"),
        "KeySource::File(\"/keys/signing.pem\")"
    );
}

#[test]
fn signing_key_debug_is_redacted() {
    let key = PrivateKey::parse(common::ED25519_PKCS8.as_bytes())
        .expect("key")
        .into_signing_key()
        .expect("Ed25519 key");
    assert_no_key_material(&format!("{key:?}"));
}
