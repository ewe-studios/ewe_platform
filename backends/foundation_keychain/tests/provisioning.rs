//! SSH key provisioning vertical — integration tests (spec-57, feature 009).
//!
//! Register app → authenticate secret → generate/list/get/delete SSH keys over a
//! real Turso store, verifying the private key is age-encrypted at rest and
//! round-trips on retrieval.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::provisioning::apps::{self, RegisterAppRequest};
use foundation_keychain::core::provisioning::ssh_keys::{self, CreateSshKeyRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

const MASTER: &str = "test-master-key-do-not-use-in-prod";

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    collect_one(execute(from_future(future), None).expect("execute")).expect("result")
}

fn fresh_ctx() -> (KeychainContext, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("keychain_test.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    let db: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    let d = Arc::clone(&db);
    drive(async move { apply_schema(d.as_ref()).await }).expect("apply schema");
    (KeychainContext::new(db), dir)
}

fn register_app(ctx: &KeychainContext, name: &str) -> (String, String) {
    let c = ctx.clone();
    let name = name.to_string();
    let resp = drive(async move {
        apps::register(&c, RegisterAppRequest { name, description: Some("test".into()) }).await
    })
    .expect("register app");
    (resp.app_id, resp.secret)
}

#[valtron_test]
fn app_register_and_authenticate() {
    let (ctx, _dir) = fresh_ctx();
    let (app_id, secret) = register_app(&ctx, "deployer");
    assert!(secret.starts_with(&format!("{app_id}.")), "secret embeds the app id");

    // The secret authenticates back to the app.
    let c = ctx.clone();
    let sec = secret.clone();
    let resolved = drive(async move { apps::authenticate(&c, &sec).await }).expect("auth");
    assert_eq!(resolved, app_id);

    // A tampered secret does not.
    let c = ctx.clone();
    let bad = format!("{app_id}.tampered");
    let res = drive(async move { apps::authenticate(&c, &bad).await });
    assert!(matches!(res, Err(AppError::Unauthorized)));
}

#[valtron_test]
fn ssh_key_generate_list_get_delete() {
    let (ctx, _dir) = fresh_ctx();
    let (app_id, _secret) = register_app(&ctx, "ci");

    // Generate an Ed25519 key — returns the private key once.
    let c = ctx.clone();
    let aid = app_id.clone();
    let created = drive(async move {
        ssh_keys::create(
            &c,
            MASTER,
            &aid,
            CreateSshKeyRequest {
                name: "deploy-key".into(),
                key_type: Some("ed25519".into()),
                comment: Some("prod".into()),
                ttl_hours: Some(720),
            },
        )
        .await
    })
    .expect("create key");
    assert_eq!(created.key_type, "ed25519");
    assert!(created.public_key.starts_with("ssh-ed25519 "), "pub: {}", created.public_key);
    let private = created.private_key.clone().expect("private returned on create");
    assert!(private.contains("BEGIN OPENSSH PRIVATE KEY"), "priv: {private}");
    assert!(created.expires_at.is_some());

    // List — public only, no private key leaked.
    let c = ctx.clone();
    let aid = app_id.clone();
    let listed = drive(async move { ssh_keys::list(&c, &aid).await }).expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].private_key.is_none(), "list must not include private key");
    assert_eq!(listed[0].public_key, created.public_key);

    // Get — decrypts the private key at rest; must match what create returned.
    let c = ctx.clone();
    let aid = app_id.clone();
    let kid = created.id.clone();
    let fetched = drive(async move { ssh_keys::get(&c, MASTER, &aid, &kid).await }).expect("get");
    assert_eq!(fetched.private_key.as_deref(), Some(private.as_str()));

    // Wrong master key can't decrypt.
    let c = ctx.clone();
    let aid = app_id.clone();
    let kid = created.id.clone();
    let wrong = drive(async move { ssh_keys::get(&c, "wrong-master", &aid, &kid).await });
    assert!(wrong.is_err(), "wrong master key must fail to decrypt");

    // Delete.
    let c = ctx.clone();
    let aid = app_id.clone();
    let kid = created.id.clone();
    drive(async move { ssh_keys::delete(&c, &aid, &kid).await }).expect("delete");
    let c = ctx.clone();
    let listed = drive(async move { ssh_keys::list(&c, &app_id).await }).expect("list2");
    assert!(listed.is_empty());
}

#[valtron_test]
fn keys_are_isolated_per_app() {
    let (ctx, _dir) = fresh_ctx();
    let (app_a, _) = register_app(&ctx, "app-a");
    let (app_b, _) = register_app(&ctx, "app-b");

    let c = ctx.clone();
    let aid = app_a.clone();
    let key = drive(async move {
        ssh_keys::create(&c, MASTER, &aid, CreateSshKeyRequest {
            name: "k".into(), key_type: None, comment: None, ttl_hours: None,
        })
        .await
    })
    .expect("create");

    // app_b cannot read app_a's key.
    let c = ctx.clone();
    let kid = key.id.clone();
    let res = drive(async move { ssh_keys::get(&c, MASTER, &app_b, &kid).await });
    assert!(matches!(res, Err(AppError::Forbidden)));
}
