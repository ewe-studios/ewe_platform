//! In-memory DocumentStore implementation — for testing and development.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{Document, DocumentStore, StorageItemStream};
use foundation_core::valtron::Stream;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// In-memory document store.
pub struct MemoryDocumentStore {
    /// Map: collection_key → Vec<(doc_id, content_json, metadata, sequence)>
    documents: Mutex<HashMap<String, Vec<(String, String, serde_json::Value, u64)>>>,
    /// Monotonic counter for ordering.
    sequence: AtomicU64,
}

impl MemoryDocumentStore {
    /// Create a new empty in-memory document store.
    pub fn new() -> Self {
        Self {
            documents: Mutex::new(HashMap::new()),
            sequence: AtomicU64::new(0),
        }
    }
}

impl Default for MemoryDocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentStore for MemoryDocumentStore {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let doc_id = format!("mem-{seq}");
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let metadata = serde_json::json!({"sequence": seq});

        let doc = Document {
            id: doc_id.clone(),
            content: content_json.clone(),
            metadata: metadata.clone(),
        };

        let mut docs = self.documents.lock().unwrap();
        docs.entry(key.to_string())
            .or_default()
            .push((doc_id, content_json, metadata, seq));

        Ok(doc)
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let docs = self.documents.lock().unwrap();
        let collection = docs.get(key).cloned().unwrap_or_default();
        drop(docs);

        // Newest first, up to limit
        let mut sorted: Vec<_> = collection.into_iter().collect();
        sorted.sort_by(|a, b| b.3.cmp(&a.3));
        sorted.truncate(limit);

        let iter = sorted.into_iter().map(|(_, content, _, _)| {
            match serde_json::from_str::<V>(&content) {
                Ok(v) => Stream::Next(Ok(v)),
                Err(e) => Stream::Next(Err(StorageError::Deserialization(e.to_string()))),
            }
        });
        Ok(Box::new(iter))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let docs = self.documents.lock().unwrap();
        let collection = docs.get(key).cloned().unwrap_or_default();
        drop(docs);

        // Oldest first
        let mut sorted: Vec<_> = collection.into_iter().collect();
        sorted.sort_by(|a, b| a.3.cmp(&b.3));

        let iter = sorted.into_iter().map(|(_, content, _, _)| {
            match serde_json::from_str::<V>(&content) {
                Ok(v) => Stream::Next(Ok(v)),
                Err(e) => Stream::Next(Err(StorageError::Deserialization(e.to_string()))),
            }
        });
        Ok(Box::new(iter))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let mut docs = self.documents.lock().unwrap();
        if let Some(collection) = docs.get_mut(key) {
            collection.retain(|(id, _, _, _)| id != doc_id);
        }
        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let mut docs = self.documents.lock().unwrap();
        let count = docs.remove(key).map(|v| v.len() as u64).unwrap_or(0);
        Ok(count)
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let docs = self.documents.lock().unwrap();
        let count = docs.get(key).map(|v| v.len() as u64).unwrap_or(0);
        Ok(count)
    }
}
