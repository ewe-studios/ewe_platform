//! Sync vault vertical — integration test (spec-57, F008 Stage 1).
//!
//! Verifies `GET /api/sync` assembles the profile + the user's folders + ciphers
//! into one payload, over a real Turso store.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::{accounts, ciphers, folders, sync};
use foundation_keychain::core::models::cipher::{CipherCreateRequest, CipherType};
use foundation_keychain::core::models::folder::FolderCreateRequest;
use foundation_keychain::core::models::user::{KdfType, RegisterRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::KeychainContext;
use tempfile::TempDir;

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

fn fresh_ctx() -> (KeychainContext, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("keychain_test.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    let db: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    let db_for_schema = Arc::clone(&db);
    drive(async move { apply_schema(db_for_schema.as_ref()).await }).expect("apply schema");
    (KeychainContext::new(db), dir)
}

fn note_cipher(name: &str) -> CipherCreateRequest {
    CipherCreateRequest {
        cipher_type: CipherType::SecureNote,
        name: name.into(),
        notes: Some("2.n|enc".into()),
        fields: None,
        login: None,
        card: None,
        identity: None,
        secure_note: None,
        data: Some("2.d|enc".into()),
        favorite: Some(false),
        organization_id: None,
        collection_ids: None,
    }
}

#[valtron_test]
fn sync_returns_profile_folders_and_ciphers() {
    let (ctx, _dir) = fresh_ctx();

    // Register a user.
    let c = ctx.clone();
    let account = drive(async move {
        accounts::register(
            &c,
            RegisterRequest {
                email: "sync@example.com".into(),
                name: Some("Sync User".into()),
                master_password_hash: "h".into(),
                master_password_hint: None,
                key: Some("2.key|enc".into()),
                kdf: Some(KdfType::Pbkdf2Sha256),
                kdf_iterations: Some(600_000),
                kdf_memory: None,
                kdf_parallelism: None,
            },
        )
        .await
    })
    .expect("register");
    let uid = account.id.clone();

    // Two folders + two ciphers.
    for name in ["Personal", "Work"] {
        let c = ctx.clone();
        let uid = uid.clone();
        drive(async move { folders::create(&c, &uid, FolderCreateRequest { name: name.into() }).await })
            .expect("folder");
    }
    for name in ["2.a|enc", "2.b|enc"] {
        let c = ctx.clone();
        let uid = uid.clone();
        drive(async move { ciphers::create(&c, &uid, note_cipher(name)).await }).expect("cipher");
    }

    // Sync returns everything.
    let c = ctx.clone();
    let uid2 = uid.clone();
    let snapshot = drive(async move { sync::sync(&c, &uid2).await }).expect("sync");

    assert_eq!(snapshot.profile.as_ref().unwrap().email, "sync@example.com");
    assert_eq!(snapshot.folders.len(), 2);
    assert_eq!(snapshot.ciphers.len(), 2);
    assert!(snapshot.sends.is_empty());
    assert!(snapshot.collections.is_empty());
    assert!(snapshot.domains.is_some());
}
