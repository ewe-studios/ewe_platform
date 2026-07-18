//! Ciphers vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! CRUD + soft-delete/restore + ownership over a real Turso store, verifying the
//! client-encrypted content round-trips through the `data` JSON column.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::ciphers;
use foundation_keychain::core::models::cipher::{
    CipherCreateRequest, CipherType, CipherUpdateRequest, LoginData,
};
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

fn login_req(name: &str, username: &str) -> CipherCreateRequest {
    CipherCreateRequest {
        cipher_type: CipherType::Login,
        name: name.into(),
        notes: Some("enc-notes".into()),
        fields: None,
        login: Some(LoginData {
            uri: Some("2.uri|enc".into()),
            uris: None,
            username: Some(username.into()),
            password: Some("2.pw|enc".into()),
            totp: None,
        }),
        card: None,
        identity: None,
        secure_note: None,
        data: Some("2.data|enc".into()),
        favorite: Some(false),
        organization_id: None,
        collection_ids: None,
    }
}

#[valtron_test]
fn create_list_get_roundtrips_content() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    let created = drive(async move {
        ciphers::create(&c, USER_A, login_req("2.name|enc", "2.user|enc")).await
    })
    .expect("create");
    assert_eq!(created.cipher_type, CipherType::Login);
    assert_eq!(created.login.as_ref().unwrap().username.as_deref(), Some("2.user|enc"));
    assert!(created.deleted_date.is_none());

    let c = ctx.clone();
    let listed = drive(async move { ciphers::list(&c, USER_A).await }).expect("list");
    assert_eq!(listed.len(), 1);

    let id = created.id.clone();
    let c = ctx.clone();
    let got = drive(async move { ciphers::get(&c, USER_A, &id).await }).expect("get");
    assert_eq!(got.name, "2.name|enc");
    assert_eq!(got.notes.as_deref(), Some("enc-notes"));
    assert_eq!(got.login.unwrap().password.as_deref(), Some("2.pw|enc"));
}

#[valtron_test]
fn update_changes_content_and_favorite() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let created = drive(async move { ciphers::create(&c, USER_A, login_req("n", "u")).await })
        .expect("create");
    let id = created.id.clone();

    let c = ctx.clone();
    let updated = drive(async move {
        ciphers::update(
            &c,
            USER_A,
            &id,
            CipherUpdateRequest {
                name: Some("2.renamed|enc".into()),
                notes: None,
                fields: None,
                login: Some(LoginData {
                    uri: None,
                    uris: None,
                    username: Some("2.newuser|enc".into()),
                    password: Some("2.newpw|enc".into()),
                    totp: None,
                }),
                card: None,
                identity: None,
                secure_note: None,
                data: None,
                favorite: Some(true),
                folder_id: None,
            },
        )
        .await
    })
    .expect("update");
    assert_eq!(updated.name, "2.renamed|enc");
    assert!(updated.favorite);
    assert_eq!(updated.login.unwrap().username.as_deref(), Some("2.newuser|enc"));
    assert!(updated.revision_date >= created.revision_date);
}

#[valtron_test]
fn soft_delete_then_restore() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let created = drive(async move { ciphers::create(&c, USER_A, login_req("n", "u")).await })
        .expect("create");
    let id = created.id.clone();

    let c = ctx.clone();
    let id2 = id.clone();
    let trashed = drive(async move { ciphers::soft_delete(&c, USER_A, &id2).await }).expect("soft");
    assert!(trashed.deleted_date.is_some(), "soft-deleted cipher has deleted_date");

    let c = ctx.clone();
    let id3 = id.clone();
    let restored = drive(async move { ciphers::restore(&c, USER_A, &id3).await }).expect("restore");
    assert!(restored.deleted_date.is_none(), "restored cipher clears deleted_date");
}

#[valtron_test]
fn hard_delete_removes() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let created = drive(async move { ciphers::create(&c, USER_A, login_req("n", "u")).await })
        .expect("create");
    let id = created.id.clone();

    let c = ctx.clone();
    let id2 = id.clone();
    drive(async move { ciphers::delete(&c, USER_A, &id2).await }).expect("delete");

    let c = ctx.clone();
    let got = drive(async move { ciphers::get(&c, USER_A, &id).await });
    assert!(matches!(got, Err(AppError::NotFound(_))));
}

#[valtron_test]
fn ownership_is_enforced() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let created = drive(async move { ciphers::create(&c, USER_A, login_req("n", "u")).await })
        .expect("create");
    let id = created.id.clone();

    let c = ctx.clone();
    let id2 = id.clone();
    let got = drive(async move { ciphers::get(&c, USER_B, &id2).await });
    assert!(matches!(got, Err(AppError::Forbidden)));

    let c = ctx.clone();
    let del = drive(async move { ciphers::delete(&c, USER_B, &id).await });
    assert!(matches!(del, Err(AppError::Forbidden)));
}

#[valtron_test]
fn empty_name_rejected() {
    let (ctx, _dir) = fresh_ctx();
    let c = ctx.clone();
    let res = drive(async move { ciphers::create(&c, USER_A, login_req("   ", "u")).await });
    assert!(matches!(res, Err(AppError::BadRequest(_))));
}
