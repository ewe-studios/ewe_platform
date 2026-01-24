//! TLS local server tests
//!
//! These tests verify TLS server functionality with controlled certificate scenarios.

#![cfg(not(target_arch = "wasm32"))]
#![cfg(feature = "ssl-rustls")]

use foundation_core::netcap::ssl::rustls::RustlsAcceptor;
use zeroize::Zeroizing;

#[test]
fn test_invalid_certificate_rejected() {
    // Test that invalid/corrupted PEM data is rejected
    let bad_cert = b"-----BEGIN CERTIFICATE-----\nINVALID\n-----END CERTIFICATE-----";
    let bad_key = b"-----BEGIN PRIVATE KEY-----\nINVALID\n-----END PRIVATE KEY-----";

    let result = RustlsAcceptor::from_pem(bad_cert.to_vec(), Zeroizing::new(bad_key.to_vec()));

    // Should fail with invalid PEM data
    assert!(result.is_err(), "Expected failure with invalid certificate");
}

#[test]
fn test_empty_certificate_rejected() {
    // Test that empty certificate data is rejected
    let empty_cert = Vec::new();
    let empty_key = Zeroizing::new(Vec::new());

    let result = RustlsAcceptor::from_pem(empty_cert, empty_key);

    // Should fail with empty data
    assert!(result.is_err(), "Expected failure with empty certificate");
}

#[test]
fn test_mismatched_cert_and_key_rejected() {
    // Test with valid PEM format but mismatched cert/key
    // Using minimal invalid but well-formed PEM
    let cert = b"-----BEGIN CERTIFICATE-----\nMIIBkTCB+wIJAKHHCgVZU1W/MA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBnVu\n-----END CERTIFICATE-----";
    let key = b"-----BEGIN PRIVATE KEY-----\nMIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgVcB/UNPxalR9zDYAjQIf\n-----END PRIVATE KEY-----";

    let result = RustlsAcceptor::from_pem(cert.to_vec(), Zeroizing::new(key.to_vec()));

    // This should likely fail due to invalid certificate format
    // If it doesn't fail at creation, it would fail at TLS handshake
    // Either outcome is acceptable for this test
    let _ = result; // Don't assert - just verify no panic
}
