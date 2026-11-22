//! RSA operator keys (native + feature `secret-rsa`).

use foundation_auth::keys::{KeySource, PrivateKey, PrivateKeyAlgorithm};
use foundation_auth::secret::SigningAlgorithm;
use foundation_core::valtron::valtron_test;

use super::common;

#[test]
fn rsa_pkcs8_loads() {
    let key = PrivateKey::parse(common::RSA_PKCS8.as_bytes()).expect("RSA PKCS#8 key");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Rsa);
    // The public half is DER `RSAPublicKey`, which starts with a SEQUENCE.
    assert_eq!(key.public_key().first(), Some(&0x30));
    assert!(key.public_key().len() >= 256);
    assert_eq!(key.kid().len(), 16);
}

#[test]
fn rsa_key_becomes_a_signing_key() {
    let key = PrivateKey::parse(common::RSA_PKCS8.as_bytes()).expect("RSA PKCS#8 key");
    let kid = key.kid().to_string();
    let signing = key.into_signing_key().expect("an RSA signing key");
    assert_eq!(signing.algorithm(), SigningAlgorithm::RsaPssSha256);
    assert_eq!(signing.verifying_key().kid(), Some(kid.as_str()));
}

#[valtron_test]
async fn rsa_inline_key_loads() {
    let fs = common::memory_fs();
    let key = PrivateKey::load(&KeySource::inline(common::RSA_PKCS8.as_bytes()), &fs)
        .await
        .expect("the inline RSA key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Rsa);
}

#[valtron_test]
async fn rsa_file_loads_over_the_injected_file_system() {
    let fs = common::memory_fs();
    common::write(&fs, "/keys/rsa.pem", common::RSA_PKCS8, 0o600).await;
    let key = PrivateKey::load(&KeySource::File("/keys/rsa.pem".to_string()), &fs)
        .await
        .expect("the RSA key file loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Rsa);
}
