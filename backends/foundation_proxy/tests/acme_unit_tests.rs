//! ACME helpers with no network: DNS-01 values, base64url, JWS shape and the
//! account thumbprint (moved from the inline test module of `src/acme.rs`).

use foundation_proxy::acme::{base64_url, build_jws, AcmeClient};

#[test]
fn dns01_value_is_deterministic() {
    let a = AcmeClient::dns01_value("tok1", "thumb1");
    let b = AcmeClient::dns01_value("tok1", "thumb1");
    assert_eq!(a, b);
}

#[test]
fn dns01_value_is_base64url() {
    let val = AcmeClient::dns01_value("test-token", "thumb");
    assert!(!val.contains('+'));
    assert!(!val.contains('/'));
    assert!(!val.contains('='));
}

#[test]
fn dns01_different_inputs_yield_different_outputs() {
    let a = AcmeClient::dns01_value("tok1", "thumb1");
    let b = AcmeClient::dns01_value("tok2", "thumb1");
    assert_ne!(a, b);
}

#[test]
fn base64_url_encodes_correctly() {
    let encoded = base64_url("test");
    assert!(!encoded.is_empty());
    assert!(!encoded.contains('='));
}

#[test]
fn base64_url_no_padding() {
    // Base64 URL-safe with no padding: every output should be padding-free
    for input in &["a", "ab", "abc", "abcd", "abcde"] {
        let encoded = base64_url(input);
        assert!(!encoded.contains('='), "input '{input}' produced '{encoded}' with padding");
    }
}

#[test]
fn build_jws_protected_payload() {
    let jwk = serde_json::json!({"kty": "EC", "crv": "P-256"});
    let jws = build_jws(
        "https://acme.example.com/newAccount",
        "abc123",
        "eyJ0ZXN0IjogdHJ1ZX0",
        None,
        Some(&jwk),
    );
    let protected = jws["protected"].as_str().unwrap();
    let payload = jws["payload"].as_str().unwrap();
    let sig = jws["signature"].as_str().unwrap();
    assert!(!protected.is_empty(), "protected should be non-empty base64url");
    assert!(!payload.is_empty(), "payload should be non-empty base64url");
    assert!(sig.is_empty(), "signature should be empty before signing");
    assert!(!protected.contains('='), "base64url should not have padding");
    assert!(!payload.contains('='), "base64url should not have padding");
}

#[test]
fn account_thumbprint_is_stable() {
    let jwk = r#"{"kty":"EC","crv":"P-256","x":"test","y":"test"}"#;
    let a = AcmeClient::account_thumbprint(jwk);
    let b = AcmeClient::account_thumbprint(jwk);
    assert_eq!(a, b);
    assert!(!a.contains('='));
    assert!(!a.contains('+'));
}
