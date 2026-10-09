//! Two-factor (TOTP) vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! get-authenticator → enable (with a real generated code) → verify at login →
//! recovery-code path → disable, over a real Turso store.

use std::sync::Arc;

use base32::Alphabet;
use foundation_auth::shared::two_factor::TOTPSecret;
use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::two_factor;
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

const USER: &str = "user-2fa";
const ALPHABET: Alphabet = Alphabet::Rfc4648 { padding: false };

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

/// The current valid TOTP code for a base32 secret (mirrors what an authenticator app shows).
fn current_code(key_base32: &str) -> String {
    let bytes = base32::decode(ALPHABET, key_base32).expect("decode secret");
    TOTPSecret::from_bytes(bytes).now()
}

#[valtron_test]
fn full_totp_lifecycle() {
    let (ctx, _dir) = fresh_ctx();

    // Fetch a stable secret; not enabled yet.
    let c = ctx.clone();
    let auth = drive(async move { two_factor::get_authenticator(&c, USER).await }).expect("get");
    assert!(!auth.enabled);
    assert!(!auth.key.is_empty());
    let key = auth.key.clone();

    // Fetching again returns the SAME secret (idempotent).
    let c = ctx.clone();
    let again = drive(async move { two_factor::get_authenticator(&c, USER).await }).expect("get2");
    assert_eq!(again.key, key);

    // A wrong code cannot enable.
    let c = ctx.clone();
    let bad = drive(async move { two_factor::enable_authenticator(&c, USER, "000000").await });
    assert!(matches!(bad, Err(AppError::BadRequest(_))));

    // The current code enables it and yields a recovery code.
    let code = current_code(&key);
    let c = ctx.clone();
    let enabled =
        drive(async move { two_factor::enable_authenticator(&c, USER, &code).await }).expect("enable");
    assert!(enabled.enabled);
    assert!(!enabled.recovery_code.is_empty());
    let recovery = enabled.recovery_code.clone();

    // Now it is enabled and a fresh code verifies at login.
    let c = ctx.clone();
    assert!(drive(async move { two_factor::is_enabled(&c, USER).await }).expect("enabled?"));
    let code = current_code(&key);
    let c = ctx.clone();
    assert!(drive(async move { two_factor::verify(&c, USER, &code).await }).expect("verify totp"));

    // The recovery code also verifies; a random one does not.
    let c = ctx.clone();
    let rec = recovery.clone();
    assert!(drive(async move { two_factor::verify(&c, USER, &rec).await }).expect("verify recovery"));
    let c = ctx.clone();
    assert!(!drive(async move { two_factor::verify(&c, USER, "not-a-code").await }).expect("verify bad"));

    // Disable with a current code; afterwards it is off.
    let code = current_code(&key);
    let c = ctx.clone();
    drive(async move { two_factor::disable_authenticator(&c, USER, &code).await }).expect("disable");
    let c = ctx.clone();
    assert!(!drive(async move { two_factor::is_enabled(&c, USER).await }).expect("disabled?"));
}
