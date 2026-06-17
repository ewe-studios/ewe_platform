//! In-memory `DocumentStore` / `AsyncDocumentStore` tests (F06).
//!
//! The in-memory store is pure (no valtron pool / no I/O), so these run as plain
//! `#[test]`s; the async surface is driven with `futures_lite::future::block_on`.

use foundation_core::valtron::Stream;
use foundation_db::traits::{AsyncDocumentStore, DocumentStore, PromotableDocument};
use foundation_db::MemoryDocumentStore;

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

/// Collect a sync scan stream's successful values.
fn sync_values<V>(stream: foundation_db::traits::StorageItemStream<'_, V>) -> Vec<V> {
    stream
        .filter_map(|s| match s {
            Stream::Next(Ok(v)) => Some(v),
            _ => None,
        })
        .collect()
}

#[test]
fn append_mints_scru128_ids_and_orders_by_id() {
    let store = MemoryDocumentStore::new();
    let a = store.append("k", serde_json::json!({"n": 1})).unwrap();
    let b = store.append("k", serde_json::json!({"n": 2})).unwrap();
    assert_eq!(a.id.len(), 25, "scru128 id, not \"mem-N\"");
    assert!(b.id > a.id, "later append has a greater scru128 id");
}

#[test]
fn scan_from_is_inclusive_and_ordered() {
    let store = MemoryDocumentStore::new();
    let ids: Vec<String> = (0..5)
        .map(|n| store.append("k", serde_json::json!({ "n": n })).unwrap().id)
        .collect();

    // scan_from the 3rd id (inclusive) → ids[2..] in order.
    let got = sync_values(store.scan_from::<serde_json::Value>("k", &ids[2], 0).unwrap());
    assert_eq!(got.len(), 3);
    assert_eq!(got[0]["n"], 2);
    assert_eq!(got[2]["n"], 4);

    // limit caps the result.
    let limited = sync_values(store.scan_from::<serde_json::Value>("k", &ids[0], 2).unwrap());
    assert_eq!(limited.len(), 2);
}

#[test]
fn append_with_id_uses_caller_id() {
    let store = MemoryDocumentStore::new();
    let id = foundation_compact::ids::new_scru128_string();
    let doc = store
        .append_with_id("k", &id, serde_json::json!({"x": 1}))
        .unwrap();
    assert_eq!(doc.id, id);
    let got = sync_values(store.scan_from::<serde_json::Value>("k", &id, 0).unwrap());
    assert_eq!(got.len(), 1);
}

#[test]
fn append_promotable_round_trips_columns() {
    let store = MemoryDocumentStore::new();
    let doc = store
        .append_promotable(
            "k",
            Note {
                kind: "observation".into(),
                text: "saw a cat".into(),
            },
        )
        .unwrap();
    assert_eq!(doc.record_type.as_deref(), Some("observation"));
    assert_eq!(doc.summary.as_deref(), Some("saw a cat"));
    assert_eq!(doc.title, None);

    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].record_type.as_deref(), Some("observation"));
    assert_eq!(docs[0].summary.as_deref(), Some("saw a cat"));
}

#[test]
fn plain_append_leaves_promoted_columns_null() {
    let store = MemoryDocumentStore::new();
    store.append("k", serde_json::json!({"n": 1})).unwrap();
    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs[0].record_type, None);
    assert_eq!(docs[0].title, None);
    assert_eq!(docs[0].summary, None);
}

#[test]
fn scan_documents_from_is_inclusive_and_ordered() {
    let store = MemoryDocumentStore::new();
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
    let docs = store.scan_documents_from("k", &ids[1], 0).unwrap();
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].id, ids[1]);
    assert_eq!(docs[0].summary.as_deref(), Some("t1"));
    assert_eq!(docs[2].summary.as_deref(), Some("t3"));
}

#[test]
fn async_store_streams_and_matches_sync_semantics() {
    use futures_lite::StreamExt;
    futures_lite::future::block_on(async {
        let store = MemoryDocumentStore::new();
        let a = store
            .append_async("k", serde_json::json!({"n": 0}))
            .await
            .unwrap();
        let b = store
            .append_async("k", serde_json::json!({"n": 1}))
            .await
            .unwrap();
        assert!(b.id > a.id, "async append preserves scru128 ordering");
        assert_eq!(store.count_async("k").await.unwrap(), 2);

        // scan_from_async yields a lazily-pulled stream (inclusive, oldest-first)
        // — drain it via `.next().await`, no Vec return.
        let mut stream = store
            .scan_from_async::<serde_json::Value>("k", &a.id, 0)
            .await
            .unwrap();
        let mut got = Vec::new();
        while let Some(item) = stream.next().await {
            got.push(item.unwrap());
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0]["n"], 0);
        assert_eq!(got[1]["n"], 1);

        store.delete_async("k", &a.id).await.unwrap();
        assert_eq!(store.count_async("k").await.unwrap(), 1);
    });
}
