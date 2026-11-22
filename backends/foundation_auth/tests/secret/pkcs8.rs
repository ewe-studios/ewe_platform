//! The Ed25519 PKCS#8 layouts: version 1 (48 bytes) and version 2 (83 bytes).

use base64::Engine;
use foundation_auth::secret::{SecretError, SigningKey};

const KEY_PEM: &str = include_str!("../fixtures/test-only-ed25519-key.pem");
#[cfg(not(feature = "secret-rsa"))]
const RSA_PEM: &str = include_str!("../fixtures/rsa-pkcs8-key.pem");
const ENCRYPTED_PEM: &str = include_str!("../fixtures/encrypted-pkcs8-key.pem");

const V2_HEADER: [u8; 16] = [
    0x30, 0x51, 0x02, 0x01, 0x01, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];
const V2_TRAILER: [u8; 3] = [0x81, 0x21, 0x00];

fn pem_der(pem: &str) -> Vec<u8> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body)
        .expect("fixture PEM is base64")
}

#[test]
fn version_1_key_loads_and_round_trips() {
    let key = SigningKey::from_pkcs8_pem(KEY_PEM, None).expect("version 1 key");
    let pem = key.to_pkcs8_pem();
    let again = SigningKey::from_pkcs8_pem(&pem, None).expect("round trip");
    assert_eq!(
        key.verifying_key().as_bytes(),
        again.verifying_key().as_bytes()
    );
    assert_eq!(pem_der(&pem).len(), 48);
}

#[test]
fn version_2_key_loads() {
    let version_1 = pem_der(KEY_PEM);
    let seed = &version_1[16..48];
    let key = SigningKey::from_pkcs8_pem(KEY_PEM, None).expect("version 1 key");

    let mut der = Vec::with_capacity(83);
    der.extend_from_slice(&V2_HEADER);
    der.extend_from_slice(seed);
    der.extend_from_slice(&V2_TRAILER);
    der.extend_from_slice(key.verifying_key().as_bytes());
    assert_eq!(der.len(), 83);

    let version_2 = SigningKey::from_pkcs8_der(&der, None).expect("version 2 key");
    assert_eq!(
        version_2.verifying_key().as_bytes(),
        key.verifying_key().as_bytes()
    );
}

#[test]
fn version_2_key_with_a_wrong_public_key_fails() {
    let version_1 = pem_der(KEY_PEM);
    let seed = &version_1[16..48];
    let key = SigningKey::from_pkcs8_pem(KEY_PEM, None).expect("version 1 key");

    let mut public = key.verifying_key().as_bytes().to_vec();
    public[0] ^= 0xff;
    let mut der = Vec::with_capacity(83);
    der.extend_from_slice(&V2_HEADER);
    der.extend_from_slice(seed);
    der.extend_from_slice(&V2_TRAILER);
    der.extend_from_slice(&public);

    assert!(matches!(
        SigningKey::from_pkcs8_der(&der, None),
        Err(SecretError::BadKey(_))
    ));
}

#[cfg(not(feature = "secret-rsa"))]
#[test]
fn rsa_key_needs_the_secret_rsa_feature() {
    // The `secret-rsa` feature is not part of this build, so an RSA key is a
    // typed error, never a silent fallback.
    assert!(matches!(
        SigningKey::from_pkcs8_pem(RSA_PEM, None),
        Err(SecretError::BadKey(_))
    ));
}

#[test]
fn encrypted_key_is_rejected() {
    assert!(matches!(
        SigningKey::from_pkcs8_pem(ENCRYPTED_PEM, None),
        Err(SecretError::BadKey(_))
    ));
}
