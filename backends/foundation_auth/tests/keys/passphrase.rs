//! Passphrase-protected key files: OpenSSH `bcrypt-pbkdf` and encrypted
//! PKCS#8.

use foundation_auth::keys::{
    KeyLoadError, KeySource, PassphraseSource, PrivateKey, PrivateKeyAlgorithm,
};
use foundation_auth::keystore::EnvironmentVariableKek;
use foundation_core::valtron::valtron_test;

use super::common;

#[test]
fn an_encrypted_openssh_key_loads_with_its_passphrase() {
    let key = PrivateKey::parse_with_passphrase(
        common::ENCRYPTED_ED25519_OPENSSH.as_bytes(),
        common::PASSPHRASE,
    )
    .expect("the encrypted OpenSSH key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[test]
fn an_encrypted_pkcs8_key_loads_with_its_passphrase() {
    let key = PrivateKey::parse_with_passphrase(
        common::ENCRYPTED_ED25519_PKCS8.as_bytes(),
        common::PASSPHRASE,
    )
    .expect("the encrypted PKCS#8 key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[test]
fn an_encrypted_key_fails_without_a_passphrase() {
    assert!(matches!(
        PrivateKey::parse(common::ENCRYPTED_ED25519_OPENSSH.as_bytes()),
        Err(KeyLoadError::Encrypted)
    ));
    assert!(matches!(
        PrivateKey::parse(common::ENCRYPTED_ED25519_PKCS8.as_bytes()),
        Err(KeyLoadError::Encrypted)
    ));
}

#[test]
fn an_encrypted_key_fails_with_a_wrong_passphrase() {
    assert!(matches!(
        PrivateKey::parse_with_passphrase(
            common::ENCRYPTED_ED25519_OPENSSH.as_bytes(),
            "the wrong passphrase"
        ),
        Err(KeyLoadError::BadPassphrase)
    ));
    assert!(matches!(
        PrivateKey::parse_with_passphrase(
            common::ENCRYPTED_ED25519_PKCS8.as_bytes(),
            "the wrong passphrase"
        ),
        Err(KeyLoadError::BadPassphrase)
    ));
}

#[valtron_test]
async fn an_encrypted_key_loads_over_the_injected_file_system() {
    let fs = common::memory_fs();
    common::write(
        &fs,
        "/keys/encrypted.pem",
        common::ENCRYPTED_ED25519_PKCS8,
        0o600,
    )
    .await;
    let key = PrivateKey::load_with_passphrase(
        &KeySource::File("/keys/encrypted.pem".to_string()),
        &fs,
        common::PASSPHRASE,
    )
    .await
    .expect("the encrypted key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[test]
fn the_passphrase_comes_from_a_kek_source() {
    let name = "EWE_TEST_KEY_FILE_PASSPHRASE";
    std::env::set_var(name, common::PASSPHRASE);
    let source = EnvironmentVariableKek::new(name, true);
    let passphrase = source.passphrase().expect("the passphrase");
    let key =
        PrivateKey::parse_with_passphrase(common::ENCRYPTED_ED25519_PKCS8.as_bytes(), &passphrase)
            .expect("the encrypted key loads with the source passphrase");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
}

#[test]
fn a_passphrase_source_without_permission_is_refused() {
    let source = EnvironmentVariableKek::new("EWE_TEST_KEY_FILE_NOT_ALLOWED", false);
    assert!(matches!(
        source.passphrase(),
        Err(KeyLoadError::PassphraseUnavailable(_))
    ));
}
