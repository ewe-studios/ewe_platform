//! The clear-text key files of the first version are sealed and removed.

use std::sync::Arc;

use foundation_auth::keystore::{
    migrate_legacy_keys, FsKeyStore, KekSealer, KeyId, KeyMaterial, KeyProtection, KeyStore,
    StoredPrivateKey,
};
use foundation_core::valtron::valtron_test;
use tempfile::TempDir;

use super::common::test_ctx;

const LEGACY_SIGNING_KEY: &str = include_str!("../fixtures/test-only-ed25519-key.pem");

#[valtron_test]
async fn a_legacy_plaintext_key_is_sealed_and_removed() {
    let (ctx, env) = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let legacy = dir.path().join("bootstrap-signing-key.pem");
    std::fs::write(&legacy, LEGACY_SIGNING_KEY).expect("write the legacy key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o600))
            .expect("restrict the legacy key");
    }

    let store = Arc::new(FsKeyStore::new(dir.path()));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(Arc::new(KekSealer::new("test-kek", [7; 32], env))),
        ctx,
    );
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");

    // Before the migration the record is read-only clear text.
    let before = store.get(&id).await.expect("get").expect("legacy record");
    assert!(matches!(before.private, StoredPrivateKey::Plaintext { .. }));

    let report = migrate_legacy_keys(&store, &material)
        .await
        .expect("migrate");
    assert_eq!(report.migrated, vec![id.clone()]);
    assert!(!legacy.exists(), "the clear-text file is removed");
    assert!(
        store.record_path(&id).exists(),
        "the sealed record is written"
    );

    let after = store.get(&id).await.expect("get").expect("sealed record");
    assert!(matches!(after.private, StoredPrivateKey::Sealed(_)));
    // The key still loads from the sealed record.
    material
        .signing_key(&id)
        .await
        .expect("the sealed key opens");

    let again = migrate_legacy_keys(&store, &material)
        .await
        .expect("second run");
    assert!(again.is_empty(), "a second run changes nothing");
}

#[valtron_test]
async fn a_sealed_key_is_not_migrated() {
    let (ctx, env) = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let store = Arc::new(FsKeyStore::new(dir.path()));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(Arc::new(KekSealer::new("test-kek", [7; 32], env))),
        ctx,
    );
    // Making a key writes a sealed record.
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");
    material.signing_key(&id).await.expect("make the key");

    let report = migrate_legacy_keys(&store, &material)
        .await
        .expect("migrate");
    assert!(report.is_empty());
}
