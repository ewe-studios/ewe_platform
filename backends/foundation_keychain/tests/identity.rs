//! Identity / login vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! Exercises `connect/token` (password + refresh grants) end-to-end over a real
//! Turso store: register → login → verify the minted access token → rotate via the
//! refresh grant → confirm the old refresh token is superseded.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::{accounts, identity};
use foundation_keychain::core::auth::verify_access_token;
use foundation_keychain::core::models::user::{KdfType, RegisterRequest, TokenRequest};
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
        master_password_hint: None,
        key: Some("2.userkey|wrapped".into()),
        kdf: Some(KdfType::Pbkdf2Sha256),
        kdf_iterations: Some(600_000),
        kdf_memory: None,
        kdf_parallelism: None,
    }
}

fn password_grant(email: &str, password: &str, device: &str) -> TokenRequest {
    TokenRequest {
        grant_type: "password".into(),
        username: Some(email.into()),
        master_password_hash: Some(password.into()),
        refresh_token: None,
        device_identifier: Some(device.into()),
        device_name: Some("Test Device".into()),
        device_type: Some(8),
    }
}

fn register(ctx: &KeychainContext, email: &str, password: &str) {
    let c = ctx.clone();
    let (email, password) = (email.to_string(), password.to_string());
    drive(async move { accounts::register(&c, register_req(&email, &password)).await })
        .expect("register");
}

#[valtron_test]
fn password_grant_issues_verifiable_tokens() {
    let (ctx, _dir) = fresh_ctx();
    register(&ctx, "alice@example.com", "master-hash");

    let c = ctx.clone();
    let login = drive(async move {
        identity::connect_token(&c, password_grant("alice@example.com", "master-hash", "dev-1")).await
    })
    .expect("login");

    assert_eq!(login.token_type, "Bearer");
    assert!(!login.access_token.is_empty());
    assert!(!login.refresh_token.is_empty());
    assert_eq!(login.kdf_iterations, 600_000);
    // The user's protected key round-trips into the unlock metadata.
    assert_eq!(login.key.as_deref(), Some("2.userkey|wrapped"));
    assert!(login.user_decryption_options.master_password_unlock.is_some());

    // The minted access token verifies and carries the right principal + sstamp.
    let authed = verify_access_token(ctx.verifier(), &login.access_token).expect("verify access");
    assert!(!authed.user_uuid.is_empty());
    assert!(authed.security_stamp.is_some());
    assert_eq!(authed.device.as_deref(), Some("dev-1"));
}

#[valtron_test]
fn wrong_password_is_rejected() {
    let (ctx, _dir) = fresh_ctx();
    register(&ctx, "bob@example.com", "right");

    let c = ctx.clone();
    let res = drive(async move {
        identity::connect_token(&c, password_grant("bob@example.com", "wrong", "dev-1")).await
    });
    assert!(matches!(res, Err(AppError::BadRequest(_))), "wrong password must be invalid_grant");
}

#[valtron_test]
fn unknown_user_is_rejected() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let res = drive(async move {
        identity::connect_token(&c, password_grant("ghost@example.com", "x", "dev-1")).await
    });
    assert!(matches!(res, Err(AppError::BadRequest(_))));
}

#[valtron_test]
fn refresh_grant_rotates_and_supersedes_old_token() {
    let (ctx, _dir) = fresh_ctx();
    register(&ctx, "carol@example.com", "pw");

    // Initial login.
    let c = ctx.clone();
    let login = drive(async move {
        identity::connect_token(&c, password_grant("carol@example.com", "pw", "dev-1")).await
    })
    .expect("login");
    let first_refresh = login.refresh_token.clone();

    // Refresh grant issues new tokens.
    let c = ctx.clone();
    let refresh_req = TokenRequest {
        grant_type: "refresh_token".into(),
        refresh_token: Some(first_refresh.clone()),
        ..Default::default()
    };
    let rotated = drive(async move { identity::connect_token(&c, refresh_req).await }).expect("refresh");
    assert!(!rotated.access_token.is_empty());
    assert_ne!(rotated.refresh_token, first_refresh, "refresh token must rotate");

    // The rotated access token verifies.
    let authed = verify_access_token(ctx.verifier(), &rotated.access_token).expect("verify rotated");
    assert_eq!(authed.device.as_deref(), Some("dev-1"));

    // The OLD refresh token is now superseded.
    let c = ctx.clone();
    let reuse_req = TokenRequest {
        grant_type: "refresh_token".into(),
        refresh_token: Some(first_refresh),
        ..Default::default()
    };
    let reuse = drive(async move { identity::connect_token(&c, reuse_req).await });
    assert!(matches!(reuse, Err(AppError::BadRequest(_))), "reused refresh token must fail");
}

#[valtron_test]
fn refresh_with_garbage_token_is_rejected() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let req = TokenRequest {
        grant_type: "refresh_token".into(),
        refresh_token: Some("not-a-jwt".into()),
        ..Default::default()
    };
    let res = drive(async move { identity::connect_token(&c, req).await });
    assert!(matches!(res, Err(AppError::BadRequest(_))));
}

#[valtron_test]
fn unsupported_grant_type_is_rejected() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let req = TokenRequest {
        grant_type: "client_credentials".into(),
        ..Default::default()
    };
    let res = drive(async move { identity::connect_token(&c, req).await });
    assert!(matches!(res, Err(AppError::BadRequest(_))));
}
