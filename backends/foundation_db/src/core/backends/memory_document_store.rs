//! In-memory `DocumentStore` implementation — for testing and development.
//!
//! Ordering is by `doc_id` (scru128, lexicographic == chronological) on every
//! scan — NOT a separate sequence counter and NOT `"mem-{seq}"` ids (which sort
//! incorrectly: `"mem-10" < "mem-2"`). This matches the SQL backend (F06 Part 0).

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncDocumentStore, Document, DocumentStore, PromotableDocument, StorageItemStream,
};
use foundation_core::valtron::Stream;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// One stored document row.
#[derive(Clone)]
struct Row {
    doc_id: String,
    content: String,
    metadata: serde_json::Value,
    title: Option<String>,
    summary: Option<String>,
    record_type: Option<String>,
}

impl Row {
    fn to_document(&self) -> Document {
        Document {
            id: self.doc_id.clone(),
            content: self.content.clone(),
            metadata: self.metadata.clone(),
            title: self.title.clone(),
            summary: self.summary.clone(),
            record_type: self.record_type.clone(),
        }
    }
}

/// In-memory document store.
pub struct MemoryDocumentStore {
    /// Map: `collection_key` → rows (kept sorted-on-read by `doc_id`).
    documents: Mutex<HashMap<String, Vec<Row>>>,
}

impl MemoryDocumentStore {
    /// Create a new empty in-memory document store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: Mutex::new(HashMap::new()),
        }
    }

    /// Insert a row (with optional promoted columns) and return the `Document` view.
    fn insert(
        &self,
        key: &str,
        doc_id: String,
        content_json: String,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> Document {
        let row = Row {
            doc_id,
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type,
        };
        let doc = row.to_document();
        self.documents
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_default()
            .push(row);
        doc
    }
}

impl Default for MemoryDocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a row's content JSON to a deserialized stream item.
fn row_to_item<V: DeserializeOwned>(content: &str) -> Stream<Result<V, StorageError>, ()> {
    match serde_json::from_str::<V>(content) {
        Ok(v) => Stream::Next(Ok(v)),
        Err(e) => Stream::Next(Err(StorageError::Deserialization(e.to_string()))),
    }
}

impl DocumentStore for MemoryDocumentStore {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(self.insert(key, doc_id, content_json, None, None, None))
    }

    fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(self.insert(key, doc_id.to_string(), content_json, None, None, None))
    }

    fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(self.insert(key, doc_id, content_json, t, s, rt))
    }

    fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(self.insert(key, doc_id.to_string(), content_json, t, s, rt))
    }

    fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let docs = self.documents.lock().unwrap();
        let mut collection = docs.get(key).cloned().unwrap_or_default();
        drop(docs);
        // Newest-first by doc_id, up to limit.
        collection.sort_by(|a, b| b.doc_id.cmp(&a.doc_id));
        collection.truncate(limit);
        Ok(collection.iter().map(Row::to_document).collect())
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let docs = self.documents.lock().unwrap();
        let mut collection: Vec<Row> = docs
            .get(key)
            .map(|v| v.iter().filter(|r| r.doc_id.as_str() >= from_id).cloned().collect())
            .unwrap_or_default();
        drop(docs);
        // Oldest-first; limit 0 = unlimited.
        collection.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
        if limit > 0 {
            collection.truncate(limit);
        }
        Ok(collection.iter().map(Row::to_document).collect())
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let docs = self.documents.lock().unwrap();
        let mut collection = docs.get(key).cloned().unwrap_or_default();
        drop(docs);

        // Newest-first by doc_id (scru128), up to limit.
        collection.sort_by(|a, b| b.doc_id.cmp(&a.doc_id));
        collection.truncate(limit);

        let iter = collection
            .into_iter()
            .map(|r| row_to_item::<V>(&r.content));
        Ok(Box::new(iter))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let docs = self.documents.lock().unwrap();
        let mut collection = docs.get(key).cloned().unwrap_or_default();
        drop(docs);

        // Oldest-first by doc_id.
        collection.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));

        let iter = collection
            .into_iter()
            .map(|r| row_to_item::<V>(&r.content));
        Ok(Box::new(iter))
    }

    fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let docs = self.documents.lock().unwrap();
        let mut collection: Vec<Row> = docs
            .get(key)
            .map(|v| {
                v.iter()
                    // Inclusive: doc_id >= from_id (OD-06-2).
                    .filter(|r| r.doc_id.as_str() >= from_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        drop(docs);

        // Oldest-first by doc_id; limit 0 = unlimited.
        collection.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
        if limit > 0 {
            collection.truncate(limit);
        }

        let iter = collection
            .into_iter()
            .map(|r| row_to_item::<V>(&r.content));
        Ok(Box::new(iter))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let mut docs = self.documents.lock().unwrap();
        if let Some(collection) = docs.get_mut(key) {
            collection.retain(|r| r.doc_id != doc_id);
        }
        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let mut docs = self.documents.lock().unwrap();
        let count = docs.remove(key).map_or(0, |v| v.len() as u64);
        Ok(count)
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let docs = self.documents.lock().unwrap();
        let count = docs.get(key).map_or(0, |v| v.len() as u64);
        Ok(count)
    }
}

/// Drain a sync item-stream into `Vec<V>`, surfacing the first item error.
fn drain<V>(stream: StorageItemStream<'_, V>) -> StorageResult<Vec<V>> {
    let mut out = Vec::new();
    for item in stream {
        match item {
            Stream::Next(Ok(v)) => out.push(v),
            Stream::Next(Err(e)) => return Err(e),
            _ => {}
        }
    }
    Ok(out)
}

/// Async `DocumentStore` over the same in-memory state — lets async backends
/// (D1/wasm, F23) and the agentic layer be exercised in tests without a live
/// Promise-based backend. Each method forwards to the sync logic (the store is
/// already in-memory), so ordering/id/promoted-column semantics are identical.
#[async_trait::async_trait(?Send)]
impl AsyncDocumentStore for MemoryDocumentStore {
    async fn append_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.append(key, content)
    }

    async fn append_with_id_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.append_with_id(key, doc_id, content)
    }

    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<Vec<V>> {
        drain(self.scan::<V>(key, limit)?)
    }

    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Vec<V>> {
        drain(self.scan_all::<V>(key)?)
    }

    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<V>> {
        drain(self.scan_from::<V>(key, from_id, limit)?)
    }

    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        self.delete(key, doc_id)
    }

    async fn delete_all_async(&self, key: &str) -> StorageResult<u64> {
        self.delete_all(key)
    }

    async fn count_async(&self, key: &str) -> StorageResult<u64> {
        self.count(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_mints_scru128_ids_and_orders_by_id() {
        let store = MemoryDocumentStore::new();
        let a = store.append("k", serde_json::json!({"n": 1})).unwrap();
        let b = store.append("k", serde_json::json!({"n": 2})).unwrap();
        // scru128 ids are 25 chars, not "mem-N".
        assert_eq!(a.id.len(), 25);
        assert!(b.id > a.id, "later append has a greater scru128 id");
    }

    #[test]
    fn scan_from_is_inclusive_and_ordered() {
        let store = MemoryDocumentStore::new();
        let ids: Vec<String> = (0..5)
            .map(|n| store.append("k", serde_json::json!({ "n": n })).unwrap().id)
            .collect();

        // scan_from the 3rd id (inclusive) → ids[2..] in order.
        let got: Vec<serde_json::Value> = store
            .scan_from::<serde_json::Value>("k", &ids[2], 0)
            .unwrap()
            .filter_map(|s| match s {
                Stream::Next(Ok(v)) => Some(v),
                _ => None,
            })
            .collect();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0]["n"], 2);
        assert_eq!(got[2]["n"], 4);

        // limit caps the result.
        let limited = store
            .scan_from::<serde_json::Value>("k", &ids[0], 2)
            .unwrap()
            .filter(|s| matches!(s, Stream::Next(Ok(_))))
            .count();
        assert_eq!(limited, 2);
    }

    #[test]
    fn append_with_id_uses_caller_id() {
        let store = MemoryDocumentStore::new();
        let id = foundation_compact::ids::new_scru128_string();
        let doc = store
            .append_with_id("k", &id, serde_json::json!({"x": 1}))
            .unwrap();
        assert_eq!(doc.id, id);
        // The supplied id anchors scan_from.
        let n = store
            .scan_from::<serde_json::Value>("k", &id, 0)
            .unwrap()
            .filter(|s| matches!(s, Stream::Next(Ok(_))))
            .count();
        assert_eq!(n, 1);
    }

    /// A record that promotes columns from its content.
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

        // Observable again through scan_documents (newest-first).
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
    fn async_document_store_matches_sync_semantics() {
        use crate::core::storage_provider::AsyncDocumentStore;
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

            // scan_from_async is inclusive and oldest-first, like the sync path.
            let got: Vec<serde_json::Value> =
                store.scan_from_async("k", &a.id, 0).await.unwrap();
            assert_eq!(got.len(), 2);
            assert_eq!(got[0]["n"], 0);
            assert_eq!(got[1]["n"], 1);

            store.delete_async("k", &a.id).await.unwrap();
            assert_eq!(store.count_async("k").await.unwrap(), 1);
        });
    }
}
