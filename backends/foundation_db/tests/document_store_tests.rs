//! SQL-backed `DocumentStore` integration tests (F06).
//!
//! Exercises the real `SQLite` path (`TursoStorage` + migrations 020/021) so the
//! promoted-column INSERT/SELECT and scru128 `doc_id` ordering are validated
//! end-to-end, not just against the in-memory backend.

use foundation_db::traits::{DocumentStore, PromotableDocument};
use foundation_db::{SqlDocumentStore, TursoStorage};
use std::sync::Mutex;
use tempfile::TempDir;

/// Shared Valtron pool guard — initialized once, reused across tests.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// Build a migrated `SqlDocumentStore` over a fresh temp `SQLite` db.
///
/// Uses the real `init_schema` → `MigrationRunner` path (not a hand-written
/// schema), so this also proves migrations 020/021 actually apply the
/// `documents` table and its promoted columns end-to-end.
fn make_store() -> (TempDir, SqlDocumentStore<TursoStorage>) {
    init_valtron();
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("docs.db");
    let storage = TursoStorage::new(url.to_str().unwrap()).unwrap();
    storage.init_schema().unwrap();
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

/// The sync `MigrationRunner` actually applies the full set and is idempotent —
/// the bug it replaces silently applied **zero** migrations (so `documents`
/// never existed). Guards F06's "migrations registered + applied" done-when.
#[test]
fn migration_runner_applies_all_then_is_idempotent() {
    use foundation_db::{MigrationRunner, MIGRATIONS};
    init_valtron();
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("migrate.db");
    let storage = TursoStorage::new(url.to_str().unwrap()).unwrap();

    // First run applies every migration; second run applies none.
    let applied = MigrationRunner::new(MIGRATIONS).run(&storage).unwrap();
    assert_eq!(applied, MIGRATIONS.len(), "first run applies all migrations");
    let reapplied = MigrationRunner::new(MIGRATIONS).run(&storage).unwrap();
    assert_eq!(reapplied, 0, "re-run is a no-op (tracked in _migrations)");

    // The documents table + promoted columns from 020/021 are usable.
    let docs = SqlDocumentStore::new(storage);
    docs.append("k", serde_json::json!({"n": 1})).unwrap();
    assert_eq!(docs.scan_documents("k", 10).unwrap().len(), 1);
}
