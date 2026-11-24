//! A state directory of today loads with no change.

use foundation_auth::keys::{KeySource, PrivateKey, PrivateKeyAlgorithm};
use foundation_core::valtron::valtron_test;

use super::common;

#[valtron_test]
async fn bootstrap_signing_key_from_a_state_directory_loads() {
    let fs = common::memory_fs();
    // The branch wrote this file as PKCS#8 PEM with mode 0600.
    common::write(
        &fs,
        "/state/bootstrap-signing-key.pem",
        common::ED25519_PKCS8,
        0o600,
    )
    .await;

    let key = PrivateKey::load(
        &KeySource::File("/state/bootstrap-signing-key.pem".to_string()),
        &fs,
    )
    .await
    .expect("the legacy key loads");
    assert_eq!(key.algorithm(), PrivateKeyAlgorithm::Ed25519);
    assert_eq!(key.public_key().len(), 32);
}
