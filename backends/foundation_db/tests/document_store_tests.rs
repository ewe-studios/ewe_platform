//! SQL-backed `DocumentStore` integration tests (F06).
//!
//! Exercises the real `SQLite` path (`TursoStorage` + migrations 020/021) so the
//! promoted-column INSERT/SELECT and scru128 `doc_id` ordering are validated
//! end-to-end, not just against the in-memory backend.

use foundation_db::traits::{DocumentStore, PromotableDocument, QueryStore};
use foundation_db::{SqlDocumentStore, TursoStorage};
use std::sync::Mutex;
use tempfile::TempDir;

/// `documents` schema = migration 020 + 021 (promoted columns). Created directly
/// so the test doesn't depend on the migration runner's stream-tracking path.
const DOCUMENTS_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_key TEXT NOT NULL,
    doc_id TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    title TEXT,
    summary TEXT,
    record_type TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_collection_doc_id ON documents(collection_key, doc_id);
CREATE INDEX IF NOT EXISTS idx_documents_collection_type ON documents(collection_key, record_type);
";

/// Shared Valtron pool guard — initialized once, reused across tests.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// Build a migrated `SqlDocumentStore` over a fresh temp `SQLite` db.
fn make_store() -> (TempDir, SqlDocumentStore<TursoStorage>) {
    init_valtron();
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("docs.db");
    let storage = TursoStorage::new(url.to_str().unwrap()).unwrap();
    storage.execute_batch(DOCUMENTS_SCHEMA).unwrap();
    (dir, SqlDocumentStore::new(storage))
}

/// A record that promotes `record_type`/`summary` from its content.
#[derive(serde::Serialize)]
struct Note {
    kind: String,
    text: String,
}
impl PromotableDocument for Note {
    fn record_type(&self) -> Option<String> {
        Some(self.kind.clone())
    }
    fn summary(&self) -> Option<String> {
        Some(self.text.clone())
    }
}

#[test]
fn append_promotable_round_trips_through_sqlite() {
    let (_dir, store) = make_store();
    let doc = store
        .append_promotable(
            "k",
            Note {
                kind: "observation".into(),
                text: "saw a cat".into(),
            },
        )
        .unwrap();
    assert_eq!(doc.id.len(), 25, "scru128 doc_id");
    assert_eq!(doc.record_type.as_deref(), Some("observation"));
    assert_eq!(doc.summary.as_deref(), Some("saw a cat"));
    assert_eq!(doc.title, None);

    // Re-read through scan_documents — promoted columns survive the round-trip.
    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, doc.id);
    assert_eq!(docs[0].record_type.as_deref(), Some("observation"));
    assert_eq!(docs[0].summary.as_deref(), Some("saw a cat"));
    assert_eq!(docs[0].title, None);
}

#[test]
fn plain_append_leaves_promoted_columns_null() {
    let (_dir, store) = make_store();
    store.append("k", serde_json::json!({"n": 1})).unwrap();
    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].record_type, None);
    assert_eq!(docs[0].title, None);
    assert_eq!(docs[0].summary, None);
}

#[test]
fn scan_documents_from_is_inclusive_and_ordered_by_doc_id() {
    let (_dir, store) = make_store();
    let ids: Vec<String> = (0..4)
        .map(|n| {
            store
                .append_promotable(
                    "k",
                    Note {
                        kind: "n".into(),
                        text: format!("t{n}"),
                    },
                )
                .unwrap()
                .id
        })
        .collect();

    // Inclusive from the 2nd id, oldest-first.
    let docs = store.scan_documents_from("k", &ids[1], 0).unwrap();
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].id, ids[1]);
    assert_eq!(docs[0].summary.as_deref(), Some("t1"));
    assert_eq!(docs[2].summary.as_deref(), Some("t3"));

    // limit caps results.
    let limited = store.scan_documents_from("k", &ids[0], 2).unwrap();
    assert_eq!(limited.len(), 2);
}

#[test]
fn scan_documents_is_newest_first() {
    let (_dir, store) = make_store();
    let mut ids = Vec::new();
    for n in 0..3 {
        ids.push(
            store
                .append("k", serde_json::json!({ "n": n }))
                .unwrap()
                .id,
        );
    }
    let docs = store.scan_documents("k", 10).unwrap();
    let got: Vec<&String> = docs.iter().map(|d| &d.id).collect();
    assert_eq!(got, vec![&ids[2], &ids[1], &ids[0]]);
}
