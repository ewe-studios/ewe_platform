//! Folders vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! WHY: Prove the portable `core/api/folders` handlers work end-to-end over a
//! real `foundation_db` SQL backend (Turso), exercising the same async code path
//! the native and Workers transports will call.
//!
//! HOW: a temp-file Turso store with the initial schema applied; the async
//! handlers are driven on the valtron pool via `from_future` + `execute` +
//! `collect_one` inside `#[valtron_test]` (`KeychainContext` is `Clone`, so a
//! clone moves into each future to satisfy the `'static` bound).

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::folders;
use foundation_keychain::core::models::folder::{FolderCreateRequest, FolderUpdateRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

const USER_A: &str = "user-aaaa";
const USER_B: &str = "user-bbbb";

/// Drive a `'static + Send` future to completion on the valtron pool.
fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

/// A fresh Turso-backed context with the schema applied. The `TempDir` guard
/// must be held for the test's lifetime.
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

#[valtron_test]
fn create_then_list_and_get() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let created = drive(async move {
        folders::create(&c, USER_A, FolderCreateRequest { name: "  Personal  ".into() }).await
    })
    .expect("create");
    // name is trimmed
    assert_eq!(created.name, "Personal");
    assert!(!created.id.is_empty());

    let c = ctx.clone();
    let listed = drive(async move { folders::list(&c, USER_A).await }).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let id = created.id.clone();
    let c = ctx.clone();
    let got = drive(async move { folders::get(&c, USER_A, &id).await }).expect("get");
    assert_eq!(got.name, "Personal");
}

#[valtron_test]
fn update_renames_and_bumps_revision() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let created = drive(async move {
        folders::create(&c, USER_A, FolderCreateRequest { name: "Work".into() }).await
    })
    .expect("create");

    let id = created.id.clone();
    let c = ctx.clone();
    let updated = drive(async move {
        folders::update(&c, USER_A, &id, FolderUpdateRequest { name: "Work Renamed".into() }).await
    })
    .expect("update");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "Work Renamed");
    assert!(updated.revision_date >= created.revision_date);
}

#[valtron_test]
fn delete_removes_folder() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let created = drive(async move {
        folders::create(&c, USER_A, FolderCreateRequest { name: "Temp".into() }).await
    })
    .expect("create");

    let id = created.id.clone();
    let c = ctx.clone();
    drive(async move { folders::delete(&c, USER_A, &id).await }).expect("delete");

    let id = created.id.clone();
    let c = ctx.clone();
    let got = drive(async move { folders::get(&c, USER_A, &id).await });
    assert!(matches!(got, Err(AppError::NotFound(_))), "deleted folder must be gone");

    let c = ctx.clone();
    let listed = drive(async move { folders::list(&c, USER_A).await }).expect("list");
    assert!(listed.is_empty());
}

#[valtron_test]
fn ownership_is_enforced() {
    let (ctx, _dir) = fresh_ctx();

    // USER_A creates a folder.
    let c = ctx.clone();
    let created = drive(async move {
        folders::create(&c, USER_A, FolderCreateRequest { name: "Private".into() }).await
    })
    .expect("create");

    // USER_B may not read it.
    let id = created.id.clone();
    let c = ctx.clone();
    let got = drive(async move { folders::get(&c, USER_B, &id).await });
    assert!(matches!(got, Err(AppError::Forbidden)), "cross-user get must be Forbidden");

    // USER_B may not update it.
    let id = created.id.clone();
    let c = ctx.clone();
    let upd = drive(async move {
        folders::update(&c, USER_B, &id, FolderUpdateRequest { name: "hijack".into() }).await
    });
    assert!(matches!(upd, Err(AppError::Forbidden)), "cross-user update must be Forbidden");

    // USER_B may not delete it.
    let id = created.id.clone();
    let c = ctx.clone();
    let del = drive(async move { folders::delete(&c, USER_B, &id).await });
    assert!(matches!(del, Err(AppError::Forbidden)), "cross-user delete must be Forbidden");

    // USER_B's own list is empty; USER_A still owns it.
    let c = ctx.clone();
    assert!(drive(async move { folders::list(&c, USER_B).await }).expect("list b").is_empty());
    let c = ctx.clone();
    assert_eq!(drive(async move { folders::list(&c, USER_A).await }).expect("list a").len(), 1);
}

#[valtron_test]
fn empty_name_is_rejected() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let res = drive(async move {
        folders::create(&c, USER_A, FolderCreateRequest { name: "   ".into() }).await
    });
    assert!(matches!(res, Err(AppError::BadRequest(_))), "blank name must be BadRequest");
}

#[valtron_test]
fn get_missing_folder_is_not_found() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let res = drive(async move { folders::get(&c, USER_A, "does-not-exist").await });
    assert!(matches!(res, Err(AppError::NotFound(_))));
}
