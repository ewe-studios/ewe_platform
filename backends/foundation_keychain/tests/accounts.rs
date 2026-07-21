//! Accounts vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! Exercises register → prelogin → profile → master-password verification over a
//! real Turso store, including the server-side PBKDF2 verifier (`foundation_auth`
//! crypto per decision 01) and user-enumeration-safe prelogin defaults.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::accounts;
use foundation_keychain::core::models::user::{KdfType, RegisterRequest, UpdateProfileRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
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

fn register_req(email: &str, password_hash: &str) -> RegisterRequest {
    RegisterRequest {
        email: email.into(),
        name: Some("Test User".into()),
        master_password_hash: password_hash.into(),
        master_password_hint: Some("the usual".into()),
        key: Some("2.abc|def".into()),
        kdf: Some(KdfType::Pbkdf2Sha256),
        kdf_iterations: Some(600_000),
        kdf_memory: None,
        kdf_parallelism: None,
    }
}

#[valtron_test]
fn register_creates_user_and_prelogin_echoes_kdf() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let account = drive(async move {
        accounts::register(&c, register_req("Alice@Example.com ", "client-hash-abc")).await
    })
    .expect("register");
    assert_eq!(account.email, "alice@example.com"); // normalized
    assert!(!account.id.is_empty());
    assert!(!account.security_stamp.is_empty());

    // Prelogin for a known user echoes their KDF params.
    let c = ctx.clone();
    let pre = drive(async move { accounts::prelogin(&c, "alice@example.com").await }).expect("prelogin");
    assert_eq!(pre.kdf, KdfType::Pbkdf2Sha256);
    assert_eq!(pre.kdf_iterations, 600_000);
}

#[valtron_test]
fn duplicate_email_conflicts() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    drive(async move { accounts::register(&c, register_req("bob@example.com", "h")).await })
        .expect("first register");

    // Same email (different case) must conflict.
    let c = ctx.clone();
    let dup = drive(async move { accounts::register(&c, register_req("BOB@example.com", "h2")).await });
    assert!(matches!(dup, Err(AppError::Conflict(_))), "duplicate email must conflict");
}

#[valtron_test]
fn master_password_verifies() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let account = drive(async move {
        accounts::register(&c, register_req("carol@example.com", "correct-hash")).await
    })
    .expect("register");
    let id = account.id.clone();

    // Correct password verifies.
    let c = ctx.clone();
    let id2 = id.clone();
    let ok = drive(async move { accounts::verify_master_password(&c, &id2, "correct-hash").await })
        .expect("verify ok");
    assert!(ok, "correct password must verify");

    // Wrong password does not.
    let c = ctx.clone();
    let bad = drive(async move { accounts::verify_master_password(&c, &id, "wrong-hash").await })
        .expect("verify runs");
    assert!(!bad, "wrong password must not verify");
}

#[valtron_test]
fn prelogin_unknown_email_returns_defaults() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let pre = drive(async move { accounts::prelogin(&c, "nobody@example.com").await }).expect("prelogin");
    // Defaults leak nothing about existence.
    assert_eq!(pre.kdf, KdfType::Pbkdf2Sha256);
    assert_eq!(pre.kdf_iterations, 600_000);
}

#[valtron_test]
fn profile_read_and_update() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let account = drive(async move {
        accounts::register(&c, register_req("dave@example.com", "h")).await
    })
    .expect("register");
    let id = account.id.clone();

    let c = ctx.clone();
    let id2 = id.clone();
    let profile = drive(async move { accounts::get_profile(&c, &id2).await }).expect("profile");
    assert_eq!(profile.email, "dave@example.com");
    assert_eq!(profile.name.as_deref(), Some("Test User"));

    let c = ctx.clone();
    let id3 = id.clone();
    let updated = drive(async move {
        accounts::update_profile(
            &c,
            &id3,
            UpdateProfileRequest { name: Some("Renamed".into()), master_password_hint: None },
        )
        .await
    })
    .expect("update");
    assert_eq!(updated.name.as_deref(), Some("Renamed"));

    // Persisted.
    let c = ctx.clone();
    let reread = drive(async move { accounts::get_profile(&c, &id).await }).expect("reread");
    assert_eq!(reread.name.as_deref(), Some("Renamed"));
}

#[valtron_test]
fn register_rejects_invalid_email() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let res = drive(async move { accounts::register(&c, register_req("not-an-email", "h")).await });
    assert!(matches!(res, Err(AppError::BadRequest(_))));
}

#[valtron_test]
fn verify_unknown_user_is_unauthorized() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let res = drive(async move { accounts::verify_master_password(&c, "ghost", "h").await });
    assert!(matches!(res, Err(AppError::Unauthorized)));
}
