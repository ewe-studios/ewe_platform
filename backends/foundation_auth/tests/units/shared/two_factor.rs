//! Two-factor helper tests.

use foundation_auth::shared::two_factor::constant_time_eq;

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq(b"hello", b"hello"));
    assert!(!constant_time_eq(b"hello", b"world"));
    assert!(!constant_time_eq(b"short", b"longer"));
}

use foundation_auth::TOTPSecret;

/// RFC 6238 appendix B: the SHA-1 test secret and the codes at the listed
/// times, cut to 6 digits (the RFC lists 8).
#[test]
fn rfc6238_sha1_test_vectors_verify_by_default() {
    let secret = TOTPSecret::from_bytes(b"12345678901234567890".to_vec());
    for (time, code) in [
        (59_u64, "287082"),
        (1_111_111_109, "081804"),
        (1_111_111_111, "050471"),
        (1_234_567_890, "005924"),
        (2_000_000_000, "279037"),
        (20_000_000_000, "353130"),
    ] {
        assert_eq!(secret.code_at(time), code, "at T = {time}");
    }
}

/// RFC 6238 appendix B: the SHA-256 test secret and codes (6 digits).
#[test]
fn rfc6238_sha256_test_vectors_verify_when_selected() {
    let secret = TOTPSecret::from_bytes(b"12345678901234567890123456789012".to_vec())
        .with_algorithm(foundation_auth::TotpAlgorithm::Sha256);
    for (time, code) in [
        (59_u64, "119246"),
        (1_111_111_109, "084774"),
        (2_000_000_000, "698825"),
    ] {
        assert_eq!(secret.code_at(time), code, "at T = {time}");
    }
}

#[test]
fn sha1_and_sha256_codes_differ() {
    let bytes = b"12345678901234567890".to_vec();
    let sha1 = TOTPSecret::from_bytes(bytes.clone());
    let sha256 =
        TOTPSecret::from_bytes(bytes).with_algorithm(foundation_auth::TotpAlgorithm::Sha256);
    assert_ne!(sha1.code_at(59), sha256.code_at(59));
}
