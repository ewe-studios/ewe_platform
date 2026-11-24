//! Key loading: every input format of the design, and every refusal.

use foundation_auth::keys::{KeyLoadError, KeySource, PrivateKey, PrivateKeyAlgorithm};
use foundation_core::valtron::valtron_test;

use super::common;

fn kid_is_derived(kid: &str) -> bool {
    kid.len() == 16
        && kid
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[test]
fn inline_pkcs8_pem_loads() {
    let key = PrivateKey::parse(common::ED25519_PKCS8.as_bytes()).expect("PKCS#8 key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
    assert!(kid_is_derived(key.kid()));
    assert_eq!(key.public_key().len(), 32);
}

#[test]
fn inline_openssh_loads() {
    let key = PrivateKey::parse(common::ED25519_OPENSSH.as_bytes()).expect("OpenSSH key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[test]
fn p256_pkcs8_loads() {
    let key = PrivateKey::parse(common::P256_PKCS8.as_bytes()).expect("P-256 PKCS#8 key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::EcdsaP256);
    assert_eq!(key.public_key().len(), 65);
}

#[test]
fn p256_sec1_loads() {
    let key = PrivateKey::parse(common::P256_SEC1.as_bytes()).expect("SEC1 key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::EcdsaP256);
    assert_eq!(key.public_key().len(), 65);
}

#[test]
fn p256_openssh_loads() {
    let key = PrivateKey::parse(common::P256_OPENSSH.as_bytes()).expect("P-256 OpenSSH key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::EcdsaP256);
    assert_eq!(key.public_key().len(), 65);
}

#[test]
fn encrypted_pkcs8_is_rejected() {
    assert!(matches!(
        PrivateKey::parse(common::ENCRYPTED_PKCS8.as_bytes()),
        Err(KeyLoadError::Encrypted)
    ));
}

#[test]
fn encrypted_openssh_is_rejected() {
    assert!(matches!(
        PrivateKey::parse(common::ENCRYPTED_OPENSSH.as_bytes()),
        Err(KeyLoadError::Encrypted)
    ));
}

#[cfg(not(feature = "secret-rsa"))]
#[test]
fn rsa_is_unsupported_without_the_feature() {
    assert!(matches!(
        PrivateKey::parse(common::RSA_PKCS8.as_bytes()),
        Err(KeyLoadError::UnsupportedFormat(_))
    ));
}

#[test]
fn garbage_is_unsupported() {
    assert!(matches!(
        PrivateKey::parse(b"not a key"),
        Err(KeyLoadError::UnsupportedFormat(_))
    ));
}

#[test]
fn a_p256_key_is_wrong_for_the_signing_role() {
    let key = PrivateKey::parse(common::P256_PKCS8.as_bytes()).expect("P-256 key");
    assert!(matches!(
        key.into_signing_key(),
        Err(KeyLoadError::WrongAlgorithm {
            expected: PrivateKeyAlgorithm::Ed25519,
            found: PrivateKeyAlgorithm::EcdsaP256,
        })
    ));
}

#[valtron_test]
async fn file_loads_over_the_injected_file_system() {
    let fs = common::memory_fs();
    common::write(&fs, "/keys/signing.pem", common::ED25519_PKCS8, 0o600).await;
    let key = PrivateKey::load(&KeySource::File("/keys/signing.pem".to_string()), &fs)
        .await
        .expect("the key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[valtron_test]
async fn missing_file_is_not_found() {
    let fs = common::memory_fs();
    assert!(matches!(
        PrivateKey::load(&KeySource::File("/keys/absent.pem".to_string()), &fs).await,
        Err(KeyLoadError::NotFound(path)) if path == "/keys/absent.pem"
    ));
}

#[cfg(unix)]
#[valtron_test]
async fn loose_permissions_are_refused() {
    let fs = common::memory_fs();
    common::write(&fs, "/keys/loose.pem", common::ED25519_PKCS8, 0o644).await;
    assert!(matches!(
        PrivateKey::load(&KeySource::File("/keys/loose.pem".to_string()), &fs).await,
        Err(KeyLoadError::LoosePermissions(path)) if path == "/keys/loose.pem"
    ));
}

#[valtron_test]
async fn inline_source_ignores_the_file_system() {
    let fs = common::memory_fs();
    let key = PrivateKey::load(&KeySource::inline(common::ED25519_PKCS8.as_bytes()), &fs)
        .await
        .expect("the inline key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}
