//! In-memory `DocumentStore` implementation — for testing and development.
//!
//! Ordering is by `doc_id` (scru128, lexicographic == chronological) on every
//! scan — NOT a separate sequence counter and NOT `"mem-{seq}"` ids (which sort
//! incorrectly: `"mem-10" < "mem-2"`). This matches the SQL backend (F06 Part 0).
//!
//! The real read/write logic lives in the neutral private helpers (`put`,
//! `collect_rows`, `remove_*`, `count_rows`); the sync [`DocumentStore`] and the
//! async [`AsyncDocumentStore`] impls both delegate to them, so neither wraps the
//! other. (The general house pattern — real logic in the async impl, sync wraps
//! it via valtron — applies to the I/O-backed backends like Turso; this in-memory
//! store has no I/O to invert and is not cheaply shareable across worker threads,
//! so it uses shared helpers instead.)

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncDocumentStore, AsyncStorageItemStream, Document, DocumentStore, PromotableDocument,
    StorageItemStream,
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

    // ---- neutral real logic (shared by the sync + async trait impls) ----

    /// Serialize `content`, store a row (with optional promoted columns), and
    /// return the `Document` view.
    fn put<V: Serialize>(
        &self,
        key: &str,
        doc_id: String,
        content: &V,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        let content_json = serde_json::to_string(content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
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
        Ok(doc)
    }

    /// Snapshot the matching rows, ordered + filtered + limited — the single read
    /// path behind every scan (sync and async).
    ///
    /// - `newest_first`: order by `doc_id` DESC (true) or ASC (false).
    /// - `from_id`: keep only rows with `doc_id >= from_id` (inclusive, OD-06-2).
    /// - `limit`: `Some(n)` caps the result; `None` is unlimited.
    fn collect_rows(
        &self,
        key: &str,
        newest_first: bool,
        from_id: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<Row> {
        let docs = self.documents.lock().unwrap();
        let mut rows: Vec<Row> = docs
            .get(key)
            .map(|v| {
                v.iter()
                    .filter(|r| from_id.is_none_or(|f| r.doc_id.as_str() >= f))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        drop(docs);

        if newest_first {
            rows.sort_by(|a, b| b.doc_id.cmp(&a.doc_id));
        } else {
            rows.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
        }
        if let Some(n) = limit {
            rows.truncate(n);
        }
        rows
    }

    fn remove_one(&self, key: &str, doc_id: &str) {
        let mut docs = self.documents.lock().unwrap();
        if let Some(collection) = docs.get_mut(key) {
            collection.retain(|r| r.doc_id != doc_id);
        }
    }

    fn remove_all(&self, key: &str) -> u64 {
        let mut docs = self.documents.lock().unwrap();
        docs.remove(key).map_or(0, |v| v.len() as u64)
    }

    fn count_rows(&self, key: &str) -> u64 {
        let docs = self.documents.lock().unwrap();
        docs.get(key).map_or(0, |v| v.len() as u64)
    }
}

impl Default for MemoryDocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

/// `0` means "unlimited" → `None`; otherwise `Some(limit)` (OD-06-2 / matches SQL).
fn limit_opt(limit: usize) -> Option<usize> {
    (limit > 0).then_some(limit)
}

/// Replay snapshotted rows as the sync [`StorageItemStream`] (a valtron-`Stream`
/// iterator of deserialized values).
fn rows_to_sync<V: DeserializeOwned + Send + 'static>(
    rows: Vec<Row>,
) -> StorageItemStream<'static, V> {
    Box::new(rows.into_iter().map(|r| match serde_json::from_str::<V>(&r.content) {
        Ok(v) => Stream::Next(Ok(v)),
        Err(e) => Stream::Next(Err(StorageError::Deserialization(e.to_string()))),
    }))
}

/// Replay snapshotted rows as the async [`AsyncStorageItemStream`] — pulled one
/// item at a time via `.next().await` (same shape as `list_keys_async`).
fn rows_to_async<V: DeserializeOwned + Send + 'static>(
    rows: Vec<Row>,
) -> AsyncStorageItemStream<'static, V> {
    let items: Vec<StorageResult<V>> = rows
        .into_iter()
        .map(|r| {
            serde_json::from_str::<V>(&r.content)
                .map_err(|e| StorageError::Deserialization(e.to_string()))
        })
        .collect();
    Box::pin(futures_lite::stream::iter(items))
}

impl DocumentStore for MemoryDocumentStore {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        self.put(key, doc_id, &content, None, None, None)
    }

    fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.put(key, doc_id.to_string(), &content, None, None, None)
    }

    fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.put(key, doc_id, &content, t, s, rt)
    }

    fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.put(key, doc_id.to_string(), &content, t, s, rt)
    }

    fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        Ok(self
            .collect_rows(key, true, None, Some(limit))
            .iter()
            .map(Row::to_document)
            .collect())
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        Ok(self
            .collect_rows(key, false, Some(from_id), limit_opt(limit))
            .iter()
            .map(Row::to_document)
            .collect())
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        Ok(rows_to_sync(self.collect_rows(key, true, None, Some(limit))))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        Ok(rows_to_sync(self.collect_rows(key, false, None, None)))
    }

    fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        Ok(rows_to_sync(self.collect_rows(
            key,
            false,
            Some(from_id),
            limit_opt(limit),
        )))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        self.remove_one(key, doc_id);
        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        Ok(self.remove_all(key))
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        Ok(self.count_rows(key))
    }
}

/// Async `DocumentStore` over the same in-memory state — lets the async backends
/// (D1/wasm, F23) and the agentic layer be exercised in tests without a live
/// Promise-based backend. Scans return a lazily-pulled [`AsyncStorageItemStream`]
/// (never a `Vec`); every method delegates to the same neutral helpers the sync
/// impl uses, so ordering/id/promoted-column semantics are identical.
#[async_trait::async_trait(?Send)]
impl AsyncDocumentStore for MemoryDocumentStore {
    async fn append_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        self.put(key, doc_id, &content, None, None, None)
    }

    async fn append_with_id_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.put(key, doc_id.to_string(), &content, None, None, None)
    }

    async fn append_promotable_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.put(key, doc_id, &content, t, s, rt)
    }

    async fn append_promotable_with_id_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.put(key, doc_id.to_string(), &content, t, s, rt)
    }

    async fn scan_documents_async(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        Ok(self
            .collect_rows(key, true, None, Some(limit))
            .iter()
            .map(Row::to_document)
            .collect())
    }

    async fn scan_documents_from_async(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        Ok(self
            .collect_rows(key, false, Some(from_id), limit_opt(limit))
            .iter()
            .map(Row::to_document)
            .collect())
    }

    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        Ok(rows_to_async(self.collect_rows(key, true, None, Some(limit))))
    }

    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        Ok(rows_to_async(self.collect_rows(key, false, None, None)))
    }

    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        Ok(rows_to_async(self.collect_rows(
            key,
            false,
            Some(from_id),
            limit_opt(limit),
        )))
    }

    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        self.remove_one(key, doc_id);
        Ok(())
    }

    async fn delete_all_async(&self, key: &str) -> StorageResult<u64> {
        Ok(self.remove_all(key))
    }

    async fn count_async(&self, key: &str) -> StorageResult<u64> {
        Ok(self.count_rows(key))
    }
}
