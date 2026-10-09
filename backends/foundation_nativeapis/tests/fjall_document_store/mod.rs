#![cfg(feature = "vfs-fjall")]

use std::sync::Arc;

use foundation_db::traits::{DocumentStore, PromotableDocument};
use foundation_nativeapis::shared::vfs::fjall_fs::InodeFs;
use foundation_nativeapis::{DurabilityWriteConfig, FjallDocumentStore, VfsFileSystem};

use foundation_core::valtron::Stream;
use serde::{Deserialize, Serialize};

fn make_store() -> FjallDocumentStore<InodeFs> {
    let fs = InodeFs::in_memory().unwrap();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig::default());
    FjallDocumentStore::open(fs, "/data".to_string(), tmpdir.path(), durability).unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestRecord {
    name: String,
    value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TypedRecord {
    name: String,
    kind: String,
}

impl PromotableDocument for TypedRecord {
    fn record_type(&self) -> Option<String> {
        Some(self.kind.clone())
    }
    fn title(&self) -> Option<String> {
        Some(self.name.clone())
    }
}

fn collect_stream<T>(stream: foundation_db::StorageItemStream<'_, T>) -> Vec<T> {
    stream
        .filter_map(|item| match item {
            Stream::Next(Ok(v)) => Some(v),
            _ => None,
        })
        .collect()
}

// --- Basic append and scan ---

#[test]
fn append_and_scan_returns_documents() {
    let store = make_store();
    let doc1 = store
        .append("col", TestRecord { name: "first".into(), value: 1 })
        .unwrap();
    let doc2 = store
        .append("col", TestRecord { name: "second".into(), value: 2 })
        .unwrap();

    assert!(!doc1.id.is_empty());
    assert!(!doc2.id.is_empty());
    assert!(doc1.id < doc2.id, "scru128 ids should be chronologically ordered");

    let results: Vec<TestRecord> = collect_stream(store.scan("col", 10).unwrap());
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "second"); // newest first
    assert_eq!(results[1].name, "first");
}

#[test]
fn scan_all_returns_oldest_first() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).unwrap();

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").unwrap());
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].name, "a");
    assert_eq!(results[1].name, "b");
    assert_eq!(results[2].name, "c");
}

#[test]
fn scan_from_returns_from_given_id() {
    let store = make_store();
    let _d1 = store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    let _d3 = store.append("col", TestRecord { name: "c".into(), value: 3 }).unwrap();

    let results: Vec<TestRecord> =
        collect_stream(store.scan_from("col", &d2.id, 0).unwrap());
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "b");
    assert_eq!(results[1].name, "c");
}

#[test]
fn scan_from_with_limit() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).unwrap();
    store.append("col", TestRecord { name: "d".into(), value: 4 }).unwrap();

    let results: Vec<TestRecord> =
        collect_stream(store.scan_from("col", &d2.id, 2).unwrap());
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "b");
    assert_eq!(results[1].name, "c");
}

// --- append_with_id ---

#[test]
fn append_with_id_uses_provided_id() {
    let store = make_store();
    let custom_id = foundation_compact::ids::new_scru128_string();
    let doc = store
        .append_with_id("col", &custom_id, TestRecord { name: "custom".into(), value: 42 })
        .unwrap();
    assert_eq!(doc.id, custom_id);

    let results: Vec<TestRecord> = collect_stream(store.scan("col", 10).unwrap());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "custom");
}

// --- Promotable documents ---

#[test]
fn append_promotable_populates_promoted_columns() {
    let store = make_store();
    let doc = store
        .append_promotable(
            "col",
            TypedRecord { name: "obs-1".into(), kind: "observation".into() },
        )
        .unwrap();

    assert_eq!(doc.title.as_deref(), Some("obs-1"));
    assert_eq!(doc.record_type.as_deref(), Some("observation"));
}

#[test]
fn append_promotable_with_id_works() {
    let store = make_store();
    let custom_id = foundation_compact::ids::new_scru128_string();
    let doc = store
        .append_promotable_with_id(
            "col",
            &custom_id,
            TypedRecord { name: "refl-1".into(), kind: "reflection".into() },
        )
        .unwrap();

    assert_eq!(doc.id, custom_id);
    assert_eq!(doc.title.as_deref(), Some("refl-1"));
    assert_eq!(doc.record_type.as_deref(), Some("reflection"));
}

// --- scan_documents ---

#[test]
fn scan_documents_returns_full_document_views() {
    let store = make_store();
    store
        .append_promotable(
            "col",
            TypedRecord { name: "item-1".into(), kind: "conversation".into() },
        )
        .unwrap();
    store
        .append_promotable(
            "col",
            TypedRecord { name: "item-2".into(), kind: "observation".into() },
        )
        .unwrap();

    let docs = store.scan_documents("col", 10).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].title.as_deref(), Some("item-2")); // newest first
    assert_eq!(docs[1].title.as_deref(), Some("item-1"));
}

#[test]
fn scan_documents_from_returns_from_id() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    let d2 = store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();

    let docs = store.scan_documents_from("col", &d2.id, 0).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, d2.id);
}

// --- Delete ---

#[test]
fn delete_removes_document_from_index() {
    let store = make_store();
    let d1 = store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();

    store.delete("col", &d1.id).unwrap();

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").unwrap());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "b");
}

#[test]
fn delete_all_removes_everything() {
    let store = make_store();
    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    store.append("col", TestRecord { name: "c".into(), value: 3 }).unwrap();

    let count = store.delete_all("col").unwrap();
    assert_eq!(count, 3);

    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").unwrap());
    assert!(results.is_empty());
}

// --- Count ---

#[test]
fn count_returns_number_of_documents() {
    let store = make_store();
    assert_eq!(store.count("col").unwrap(), 0);

    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    assert_eq!(store.count("col").unwrap(), 1);

    store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    assert_eq!(store.count("col").unwrap(), 2);
}

// --- Multiple collections ---

#[test]
fn separate_collections_are_independent() {
    let store = make_store();
    store.append("alpha", TestRecord { name: "a".into(), value: 1 }).unwrap();
    store.append("beta", TestRecord { name: "b".into(), value: 2 }).unwrap();

    let alpha: Vec<TestRecord> = collect_stream(store.scan_all("alpha").unwrap());
    let beta: Vec<TestRecord> = collect_stream(store.scan_all("beta").unwrap());

    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].name, "a");
    assert_eq!(beta.len(), 1);
    assert_eq!(beta[0].name, "b");
}

// --- Empty collection ---

#[test]
fn scan_empty_collection_returns_empty() {
    let store = make_store();
    let results: Vec<TestRecord> = collect_stream(store.scan("empty", 10).unwrap());
    assert!(results.is_empty());
}

#[test]
fn scan_all_empty_collection_returns_empty() {
    let store = make_store();
    let results: Vec<TestRecord> = collect_stream(store.scan_all("empty").unwrap());
    assert!(results.is_empty());
}

// --- Batched durability ---

#[test]
fn batched_durability_accumulates_then_flushes() {
    let fs = InodeFs::in_memory().unwrap();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig {
        batch_size_bytes: 1024,
        flush_timeout: std::time::Duration::from_secs(60),
    });
    let store = FjallDocumentStore::open(
        fs, "/data".to_string(), tmpdir.path(), durability,
    )
    .unwrap();

    store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
    store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();

    // Data should still be scannable from the batch
    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").unwrap());
    assert_eq!(results.len(), 2);

    // Explicit flush
    store.flush("col").unwrap();

    // Still readable after flush
    let results: Vec<TestRecord> = collect_stream(store.scan_all("col").unwrap());
    assert_eq!(results.len(), 2);
}

// --- Crash recovery (tail rebuild) ---

#[test]
fn tail_rebuild_recovers_unindexed_records() {
    let fs = InodeFs::in_memory().unwrap();
    let tmpdir = tempfile::tempdir().unwrap();
    let durability = Arc::new(DurabilityWriteConfig::default());

    // First: create store and add records
    {
        let store = FjallDocumentStore::open(
            fs.clone(),
            "/data".to_string(),
            tmpdir.path(),
            durability.clone(),
        )
        .unwrap();

        store.append("col", TestRecord { name: "a".into(), value: 1 }).unwrap();
        store.append("col", TestRecord { name: "b".into(), value: 2 }).unwrap();
    }

    // Simulate crash: write more data to the NDJSON file directly (bypassing index)
    let hex_key = hex::encode("col");
    let extra_path = format!("/data/{hex_key}.jsonl");
    let existing = fs.read_file(&extra_path).unwrap();
    let extra_line = serde_json::json!({
        "id": foundation_compact::ids::new_scru128_string(),
        "content": "\"orphan\"",
        "metadata": {}
    });
    let mut new_data = existing;
    new_data.extend_from_slice(serde_json::to_string(&extra_line).unwrap().as_bytes());
    new_data.push(b'\n');
    fs.write_file(&extra_path, &new_data).unwrap();

    // Re-open: tail rebuild should recover the orphan
    let store2 = FjallDocumentStore::open(
        fs,
        "/data".to_string(),
        tmpdir.path(),
        durability,
    )
    .unwrap();

    assert_eq!(store2.count("col").unwrap(), 3);
}

// --- Ordering matches other backends ---

#[test]
fn ordering_is_chronological_by_scru128() {
    let store = make_store();
    let mut ids = Vec::new();
    for i in 0..5 {
        let doc = store
            .append("col", TestRecord { name: format!("item-{i}"), value: i })
            .unwrap();
        ids.push(doc.id);
    }

    // scan_all: oldest first
    let all_docs = store.scan_documents_from("col", &ids[0], 0).unwrap();
    for (i, doc) in all_docs.iter().enumerate() {
        assert_eq!(doc.id, ids[i]);
    }

    // scan: newest first
    let newest = store.scan_documents("col", 5).unwrap();
    for (i, doc) in newest.iter().enumerate() {
        assert_eq!(doc.id, ids[4 - i]);
    }
}
