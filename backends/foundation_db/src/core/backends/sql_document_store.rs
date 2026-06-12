//! SQL-backed DocumentStore implementation.
//!
//! Documents are stored in a table with columns: collection_key, doc_id, content, metadata, created_at.
//! This works with SQLite, Turso, D1, and any SQL backend via QueryStore.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{Document, DocumentStore, StorageItemStream};
use foundation_core::valtron::Stream;
use serde::{de::DeserializeOwned, Serialize};

/// SQL-backed document store that uses [`crate::core::storage_provider::QueryStore`] for persistence.
pub struct SqlDocumentStore<Q> {
    query_store: Q,
    table: String,
}

impl<Q> SqlDocumentStore<Q> {
    /// Create a new SqlDocumentStore backed by the given QueryStore.
    pub fn new(query_store: Q) -> Self {
        Self {
            query_store,
            table: "documents".to_string(),
        }
    }

    /// Set the table name (default: "documents").
    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }
}

impl<Q: crate::core::storage_provider::QueryStore> DocumentStore for SqlDocumentStore<Q> {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<StorageItemStream<'_, Document>> {
        let doc_id = foundation_rng::new_scru128_string();
        let content_json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let metadata = serde_json::json!({});
        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let table = &self.table;

        let sql = format!(
            "INSERT INTO {} (collection_key, doc_id, content, metadata) VALUES (?, ?, ?, ?)",
            table
        );

        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(doc_id.clone()),
            crate::core::storage_provider::DataValue::Text(content_json.clone()),
            crate::core::storage_provider::DataValue::Text(metadata_json),
        ];

        self.query_store.execute(&sql, &params)?;

        let doc = Document {
            id: doc_id,
            content: content_json,
            metadata,
        };
        let stream = std::iter::once(Stream::Next(Ok(doc)));
        Ok(Box::new(stream))
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let table = &self.table;
        let sql = format!(
            "SELECT content FROM {} WHERE collection_key = ? ORDER BY created_at DESC LIMIT ?",
            table
        );
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Integer(limit as i64),
        ];

        let rows = self.query_store.query(&sql, &params)?;
        let iter = rows.filter_map(|s| match s {
            Stream::Next(Ok(row)) => match row.get::<String>(0) {
                Ok(content) => match serde_json::from_str::<V>(&content) {
                    Ok(v) => Some(Stream::Next(Ok(v))),
                    Err(e) => Some(Stream::Next(Err(StorageError::Deserialization(
                        e.to_string(),
                    )))),
                },
                Err(e) => Some(Stream::Next(Err(e))),
            },
            Stream::Next(Err(e)) => Some(Stream::Next(Err(e))),
            _ => None,
        });
        Ok(Box::new(iter))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let table = &self.table;
        let sql = format!(
            "SELECT content FROM {} WHERE collection_key = ? ORDER BY created_at ASC",
            table
        );
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];

        let rows = self.query_store.query(&sql, &params)?;
        let iter = rows.filter_map(|s| match s {
            Stream::Next(Ok(row)) => match row.get::<String>(0) {
                Ok(content) => match serde_json::from_str::<V>(&content) {
                    Ok(v) => Some(Stream::Next(Ok(v))),
                    Err(e) => Some(Stream::Next(Err(StorageError::Deserialization(
                        e.to_string(),
                    )))),
                },
                Err(e) => Some(Stream::Next(Err(e))),
            },
            Stream::Next(Err(e)) => Some(Stream::Next(Err(e))),
            _ => None,
        });
        Ok(Box::new(iter))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let table = &self.table;
        let sql = format!(
            "DELETE FROM {} WHERE collection_key = ? AND doc_id = ?",
            table
        );
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(doc_id.to_string()),
        ];
        self.query_store.execute(&sql, &params)?;
        let stream = std::iter::once(Stream::Next(Ok(())));
        Ok(Box::new(stream))
    }

    fn delete_all(&self, key: &str) -> StorageResult<StorageItemStream<'_, u64>> {
        let table = &self.table;
        let sql = format!("DELETE FROM {} WHERE collection_key = ?", table);
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        let result = self.query_store.execute(&sql, &params)?;
        Ok(Box::new(result))
    }

    fn count(&self, key: &str) -> StorageResult<StorageItemStream<'_, u64>> {
        let table = &self.table;
        let sql = format!("SELECT COUNT(*) as cnt FROM {} WHERE collection_key = ?", table);
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        let rows = self.query_store.query(&sql, &params)?;
        let iter = rows.filter_map(|s| match s {
            Stream::Next(Ok(row)) => match row.get_by_name::<i64>("cnt") {
                Ok(n) => Some(Stream::Next(Ok(n as u64))),
                Err(e) => Some(Stream::Next(Err(e))),
            },
            Stream::Next(Err(e)) => Some(Stream::Next(Err(e))),
            _ => None,
        });
        Ok(Box::new(iter))
    }
}
