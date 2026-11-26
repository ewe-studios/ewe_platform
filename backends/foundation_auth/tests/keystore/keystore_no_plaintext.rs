//! No store writes a private key in clear text.

use foundation_auth::keystore::{
    EnvKeyStore, KeyId, KeyKind, KeyMaterial, KeyProtection, KeyStore, KeyStoreError,
    MemoryKeyStore,
};
use foundation_core::valtron::valtron_test;

use super::common::{dummy_blob, plaintext_record, test_ctx};

#[valtron_test]
async fn a_clear_text_record_is_refused_by_every_writable_store() {
    let memory = MemoryKeyStore::new();
    assert!(matches!(
        memory
            .create(&plaintext_record("ca", KeyKind::CertificateAuthority))
            .await,
        Err(KeyStoreError::PlaintextRefused(_))
    ));

    let (_ctx, _env) = test_ctx();
    let env_store = EnvKeyStore::new(vec![]);
    assert!(matches!(
        env_store
            .create(&plaintext_record("ca", KeyKind::CertificateAuthority))
            .await,
        Err(KeyStoreError::ReadOnly)
    ));
}

#[valtron_test]
async fn a_clear_text_record_does_not_open() {
    let (ctx, env) = test_ctx();
    let clear = plaintext_record("ca", KeyKind::CertificateAuthority);
    let store = EnvKeyStore::new(vec![clear.clone()]);
    let material = KeyMaterial::new(
        std::sync::Arc::new(store),
        KeyProtection::new(std::sync::Arc::new(
            foundation_auth::keystore::KekSealer::new("test-kek", [7; 32], env),
        )),
        ctx,
    );
    assert!(matches!(
        material.open_signing_key(&clear).await,
        Err(KeyStoreError::PlaintextRefused(_))
    ));
}

/// A sealed record is accepted, so the refusal is about the clear text only.
#[valtron_test]
async fn a_sealed_record_is_accepted() {
    let store = MemoryKeyStore::new();
    assert!(matches!(
        store
            .create(&super::common::sealed_record(
                "ca",
                KeyKind::CertificateAuthority,
                dummy_blob()
            ))
            .await,
        Ok(foundation_auth::keystore::CreateOutcome::Created)
    ));
    let id = KeyId::new("ca").expect("valid id");
    assert!(store.get(&id).await.expect("get").is_some());
}
