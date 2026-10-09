//! `Default` for the persistent stores: a default-constructed store must be
//! usable as-is, so generic code that asks for `S: Default` (e.g.
//! `AgentSession::build()`) works with SQL-backed stores, not just in-memory ones.

use foundation_db::traits::{DocumentStore, KeyValueStore};
use foundation_db::{SqlDocumentStore, StorageProvider, TursoStorage};
use std::sync::Mutex;

/// Shared Valtron pool guard: Turso drives its async work through valtron, so
/// the pool must exist before a store is opened.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

#[test]
fn default_turso_storage_is_a_migrated_key_value_store() {
    init_valtron();
    let storage = TursoStorage::default();

    storage.set("greeting", "hello").unwrap();
    let value: Option<String> = storage.get("greeting").unwrap();
    assert_eq!(value.as_deref(), Some("hello"));
}

#[test]
fn default_sql_document_store_has_its_documents_table() {
    init_valtron();
    let store = SqlDocumentStore::<TursoStorage>::default();

    let doc = store.append("k", serde_json::json!({"n": 1})).unwrap();
    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, doc.id);
}

#[test]
fn default_turso_stores_are_independent() {
    init_valtron();
    let a = TursoStorage::default();
    let b = TursoStorage::default();

    a.set("only_in_a", 1_u32).unwrap();
    let in_b: Option<u32> = b.get("only_in_a").unwrap();
    assert_eq!(
        in_b, None,
        "each default store is its own in-memory database"
    );
}

#[test]
fn default_storage_provider_is_in_memory() {
    let provider = StorageProvider::default();

    provider.set("k", "v").unwrap();
    let value: Option<String> = provider.get("k").unwrap();
    assert_eq!(value.as_deref(), Some("v"));
}
