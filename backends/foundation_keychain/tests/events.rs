//! Events audit-log tests (spec-57, F008 Stage 1).

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::events::{self, EventRequest};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::KeychainContext;
use tempfile::TempDir;

const USER: &str = "user-ev";

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

#[valtron_test]
fn collect_then_list() {
    let (ctx, _dir) = fresh_ctx();

    let c = ctx.clone();
    drive(async move {
        events::collect(
            &c,
            USER,
            vec![
                EventRequest { event_type: 1114, cipher_id: Some("cid-1".into()), date: None },
                EventRequest { event_type: 1107, cipher_id: None, date: None },
            ],
        )
        .await
    })
    .expect("collect");

    let c = ctx.clone();
    let listed = drive(async move { events::list(&c, USER).await }).expect("list");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|e| e.event_type == 1114 && e.cipher_id.as_deref() == Some("cid-1")));

    // Another user sees none.
    let c = ctx.clone();
    assert!(drive(async move { events::list(&c, "other").await }).expect("list other").is_empty());
}
