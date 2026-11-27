//! Key encryption key (KEK) rotation: every record opens with the new KEK, and
//! the old KEK is no longer needed.

use std::sync::Arc;

use foundation_auth::keystore::{
    migrate_legacy_keys, FsKeyStore, KekSealer, KeyId, KeyMaterial, KeyProtection, KeySealer,
    KeyStore, StoredPrivateKey,
};
use foundation_core::valtron::valtron_test;
use tempfile::TempDir;

use super::common::test_ctx;

const LEGACY_SIGNING_KEY: &str = include_str!("../fixtures/test-only-ed25519-key.pem");

#[valtron_test]
async fn rotation_moves_every_record_to_the_new_kek() {
    let (ctx, env) = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    std::fs::write(
        dir.path().join("bootstrap-signing-key.pem"),
        LEGACY_SIGNING_KEY,
    )
    .expect("write the legacy key");

    let store = Arc::new(FsKeyStore::new(dir.path()));
    let old: Arc<dyn KeySealer> = Arc::new(KekSealer::new("old-kek", [1; 32], env.clone()));
    let new: Arc<dyn KeySealer> = Arc::new(KekSealer::new("new-kek", [2; 32], env.clone()));
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");

    // Migrate the legacy key under the old KEK, and remember its `kid`.
    let before = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(Arc::clone(&old)),
        ctx.clone(),
    );
    migrate_legacy_keys(&store, &before).await.expect("migrate");
    let kid = before
        .signing_key(&id)
        .await
        .expect("old key")
        .verifying_key()
        .kid()
        .map(str::to_string);

    // Rotate: the current KEK is new, the old KEK can still open the record.
    let rotating = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::with_previous(Arc::clone(&new), vec![Arc::clone(&old)]),
        ctx.clone(),
    );
    assert_eq!(rotating.rotate_kek().await.expect("rotate"), 1);

    let record = store.get(&id).await.expect("get").expect("record");
    match &record.private {
        StoredPrivateKey::Sealed(sealed) => {
            assert_eq!(sealed.kek_id, "new-kek");
        }
        _ => panic!("the record is still not sealed"),
    }

    // A host with only the new KEK opens the rotated record.
    let after = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(new),
        ctx,
    );
    let key = after.signing_key(&id).await.expect("new key");
    assert_eq!(key.verifying_key().kid().map(str::to_string), kid);
    // A second rotation has nothing to do.
    assert_eq!(after.rotate_kek().await.expect("rotate again"), 0);
}
