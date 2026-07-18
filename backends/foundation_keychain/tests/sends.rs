//! Sends vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! Owner CRUD + anonymous access controls (password, max-access-count, disabled)
//! over a real Turso store.

use std::sync::Arc;

use chrono::{Duration, Utc};
use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::sends;
use foundation_keychain::core::models::send::{SendCreateRequest, SendType, SendUpdateRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

const USER_A: &str = "user-aaaa";
const USER_B: &str = "user-bbbb";

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

fn text_send(name: &str) -> SendCreateRequest {
    SendCreateRequest {
        name: name.into(),
        notes: None,
        send_type: SendType::Text,
        data: Some("2.text|enc".into()),
        key: Some("2.key|enc".into()),
        password: None,
        max_access_count: None,
        deletion_date: None,
        expiration_date: None,
        disabled: Some(false),
        hide_email: Some(false),
    }
}

#[valtron_test]
fn create_list_get_and_password_not_echoed() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let mut req = text_send("Shared");
    req.password = Some("access-pw".into());
    let created = drive(async move { sends::create(&c, USER_A, req).await }).expect("create");
    assert_eq!(created.send_type, SendType::Text);
    assert_eq!(created.access_count, 0);
    assert!(created.password.is_none(), "password must never be echoed");

    let c = ctx.clone();
    assert_eq!(drive(async move { sends::list(&c, USER_A).await }).expect("list").len(), 1);

    let id = created.id.clone();
    let c = ctx.clone();
    let got = drive(async move { sends::get(&c, USER_A, &id).await }).expect("get");
    assert_eq!(got.name, "Shared");
}

#[valtron_test]
fn anonymous_access_enforces_password_and_counts() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let mut req = text_send("Protected");
    req.password = Some("secret".into());
    req.max_access_count = Some(2);
    let created = drive(async move { sends::create(&c, USER_A, req).await }).expect("create");
    let id = created.id.clone();

    // Wrong password rejected.
    let c = ctx.clone();
    let id2 = id.clone();
    let bad = drive(async move { sends::access(&c, &id2, Some("nope")).await });
    assert!(matches!(bad, Err(AppError::Unauthorized)));

    // Correct password: first two accesses succeed, counter climbs.
    let c = ctx.clone();
    let id2 = id.clone();
    let a1 = drive(async move { sends::access(&c, &id2, Some("secret")).await }).expect("access 1");
    assert_eq!(a1.access_count, 1);
    let c = ctx.clone();
    let id2 = id.clone();
    let a2 = drive(async move { sends::access(&c, &id2, Some("secret")).await }).expect("access 2");
    assert_eq!(a2.access_count, 2);

    // Third exceeds max_access_count → gone (NotFound).
    let c = ctx.clone();
    let id2 = id.clone();
    let a3 = drive(async move { sends::access(&c, &id2, Some("secret")).await });
    assert!(matches!(a3, Err(AppError::NotFound(_))));
}

#[valtron_test]
fn disabled_and_expired_sends_are_gone() {
    let (ctx, _dir) = fresh_ctx();

    // Disabled.
    let c = ctx.clone();
    let mut req = text_send("Off");
    req.disabled = Some(true);
    let off = drive(async move { sends::create(&c, USER_A, req).await }).expect("create off");
    let c = ctx.clone();
    let id = off.id.clone();
    assert!(matches!(
        drive(async move { sends::access(&c, &id, None).await }),
        Err(AppError::NotFound(_))
    ));

    // Already past deletion date.
    let c = ctx.clone();
    let mut req = text_send("Expired");
    req.deletion_date = Some((Utc::now() - Duration::hours(1)).to_rfc3339());
    let expired = drive(async move { sends::create(&c, USER_A, req).await }).expect("create expired");
    let c = ctx.clone();
    let id = expired.id.clone();
    assert!(matches!(
        drive(async move { sends::access(&c, &id, None).await }),
        Err(AppError::NotFound(_))
    ));
}

#[valtron_test]
fn update_delete_and_ownership() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let created = drive(async move { sends::create(&c, USER_A, text_send("Orig")).await })
        .expect("create");
    let id = created.id.clone();

    // Update.
    let c = ctx.clone();
    let id2 = id.clone();
    let updated = drive(async move {
        sends::update(
            &c,
            USER_A,
            &id2,
            SendUpdateRequest {
                name: Some("Renamed".into()),
                notes: None,
                data: None,
                key: None,
                password: None,
                max_access_count: Some(5),
                disabled: Some(true),
                hide_email: None,
            },
        )
        .await
    })
    .expect("update");
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.max_access_count, Some(5));
    assert!(updated.disabled);

    // USER_B can't touch it.
    let c = ctx.clone();
    let id2 = id.clone();
    assert!(matches!(
        drive(async move { sends::get(&c, USER_B, &id2).await }),
        Err(AppError::Forbidden)
    ));

    // Delete.
    let c = ctx.clone();
    let id2 = id.clone();
    drive(async move { sends::delete(&c, USER_A, &id2).await }).expect("delete");
    let c = ctx.clone();
    assert!(matches!(
        drive(async move { sends::get(&c, USER_A, &id).await }),
        Err(AppError::NotFound(_))
    ));
}
