//! SQL-backed `DocumentStore` implementation.
//!
//! Documents are stored in a table with columns: `collection_key`, `doc_id`, content, metadata, `created_at`.
//! This works with `SQLite`, Turso, D1, and any SQL backend via `QueryStore`.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    Document, DocumentStore, PromotableDocument, SqlRow, StorageItemStream,
};
use foundation_core::valtron::Stream;
use serde::{de::DeserializeOwned, Serialize};

/// SQL-backed document store that uses [`crate::core::storage_provider::QueryStore`] for persistence.
pub struct SqlDocumentStore<Q> {
    query_store: Q,
    table: String,
}

impl<Q> SqlDocumentStore<Q> {
    /// Create a new `SqlDocumentStore` backed by the given `QueryStore`.
    pub fn new(query_store: Q) -> Self {
        Self {
            query_store,
            table: "documents".to_string(),
        }
    }

    /// Set the table name (default: "documents").
    #[must_use]
    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }
}

/// Clamp a `usize` limit to an `i64` SQL parameter (saturating).
fn limit_to_i64(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}

/// `0` means "unlimited" → SQL `-1`; otherwise the saturated limit.
fn limit_or_unlimited(limit: usize) -> i64 {
    if limit == 0 {
        -1
    } else {
        limit_to_i64(limit)
    }
}

impl<Q: crate::core::storage_provider::QueryStore> SqlDocumentStore<Q> {
    /// Shared INSERT path for `append`/`append_with_id`.
    fn insert<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: String,
        content: &V,
    ) -> StorageResult<Document> {
        self.insert_promoted(key, doc_id, content, None, None, None)
    }

    /// INSERT path that also writes the promoted searchable columns
    /// (`title`/`summary`/`record_type`). The JSON `content` blob stays the
    /// source of truth; promoted columns are nullable mirrors (F06 Part 2).
    fn insert_promoted<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: String,
        content: &V,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        use crate::core::storage_provider::DataValue;
        let content_json = serde_json::to_string(content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let metadata = serde_json::json!({});
        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let table = &self.table;

        let sql = format!(
            "INSERT INTO {table} \
             (collection_key, doc_id, content, metadata, title, summary, record_type) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        );
        let opt = |v: &Option<String>| match v {
            Some(s) => DataValue::Text(s.clone()),
            None => DataValue::Null,
        };
        let params = [
            DataValue::Text(key.to_string()),
            DataValue::Text(doc_id.clone()),
            DataValue::Text(content_json.clone()),
            DataValue::Text(metadata_json),
            opt(&title),
            opt(&summary),
            opt(&record_type),
        ];
        self.query_store.execute(&sql, &params)?;

        Ok(Document {
            id: doc_id,
            content: content_json,
            metadata,
            title,
            summary,
            record_type,
        })
    }
}

/// Build a `Document` from a row selecting
/// `doc_id, content, metadata, title, summary, record_type` (in that order).
fn row_to_document(row: &SqlRow) -> StorageResult<Document> {
    let metadata = row
        .get::<String>(2)
        .ok()
        .and_then(|m| serde_json::from_str(&m).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(Document {
        id: row.get::<String>(0)?,
        content: row.get::<String>(1)?,
        metadata,
        // Option<String> distinguishes a NULL column from an empty string,
        // which `String::from_data_value` would coerce NULL to.
        title: row.get::<Option<String>>(3)?,
        summary: row.get::<Option<String>>(4)?,
        record_type: row.get::<Option<String>>(5)?,
    })
}

/// Collect a `SELECT doc_id, content, metadata, title, summary, record_type` stream
/// into `Vec<Document>`.
fn collect_documents(
    rows: Stream<Result<SqlRow, StorageError>, ()>,
) -> Option<StorageResult<Document>> {
    match rows {
        Stream::Next(Ok(row)) => Some(row_to_document(&row)),
        Stream::Next(Err(e)) => Some(Err(e)),
        _ => None,
    }
}

/// Deserialize the `content` column (index 0) of a query row into `V`.
fn content_to_item<V: DeserializeOwned>(
    s: Stream<Result<SqlRow, StorageError>, ()>,
) -> Option<Stream<Result<V, StorageError>, ()>> {
    match s {
        Stream::Next(Ok(row)) => match row.get::<String>(0) {
            Ok(content) => match serde_json::from_str::<V>(&content) {
                Ok(v) => Some(Stream::Next(Ok(v))),
                Err(e) => Some(Stream::Next(Err(StorageError::Deserialization(e.to_string())))),
            },
            Err(e) => Some(Stream::Next(Err(e))),
        },
        Stream::Next(Err(e)) => Some(Stream::Next(Err(e))),
        _ => None,
    }
}

impl<Q: crate::core::storage_provider::QueryStore> DocumentStore for SqlDocumentStore<Q> {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        self.insert(key, doc_id, &content)
    }

    fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.insert(key, doc_id.to_string(), &content)
    }

    fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.insert_promoted(key, doc_id, &content, t, s, rt)
    }

    fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.insert_promoted(key, doc_id.to_string(), &content, t, s, rt)
    }

    fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let table = &self.table;
        let sql = format!(
            "SELECT doc_id, content, metadata, title, summary, record_type \
             FROM {table} WHERE collection_key = ? ORDER BY doc_id DESC LIMIT ?"
        );
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Integer(limit_to_i64(limit)),
        ];
        let rows = self.query_store.query(&sql, &params)?;
        rows.filter_map(collect_documents).collect()
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let table = &self.table;
        let sql = format!(
            "SELECT doc_id, content, metadata, title, summary, record_type \
             FROM {table} WHERE collection_key = ? AND doc_id >= ? \
             ORDER BY doc_id ASC LIMIT ?"
        );
        let sql_limit = limit_or_unlimited(limit);
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(from_id.to_string()),
            crate::core::storage_provider::DataValue::Integer(sql_limit),
        ];
        let rows = self.query_store.query(&sql, &params)?;
        rows.filter_map(collect_documents).collect()
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let table = &self.table;
        // Order by doc_id (scru128) — strictly time-ordered, unlike the coarse
        // 1-second `created_at` (F06 Part 0 / OD-06-4).
        let sql = format!(
            "SELECT content FROM {table} WHERE collection_key = ? ORDER BY doc_id DESC LIMIT ?"
        );
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Integer(limit_to_i64(limit)),
        ];

        let rows = self.query_store.query(&sql, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let table = &self.table;
        let sql = format!(
            "SELECT content FROM {table} WHERE collection_key = ? ORDER BY doc_id ASC"
        );
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];

        let rows = self.query_store.query(&sql, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let table = &self.table;
        // doc_id >= from_id (inclusive — OD-06-2), oldest-first; served by the
        // (collection_key, doc_id) unique index. limit 0 = unlimited (-1 in SQL).
        let sql = format!(
            "SELECT content FROM {table} WHERE collection_key = ? AND doc_id >= ? \
             ORDER BY doc_id ASC LIMIT ?"
        );
        let sql_limit = limit_or_unlimited(limit);
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(from_id.to_string()),
            crate::core::storage_provider::DataValue::Integer(sql_limit),
        ];

        let rows = self.query_store.query(&sql, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let table = &self.table;
        let sql = format!(
            "DELETE FROM {table} WHERE collection_key = ? AND doc_id = ?"
        );
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(doc_id.to_string()),
        ];
        self.query_store.execute(&sql, &params)?;
        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let table = &self.table;
        let sql = format!("DELETE FROM {table} WHERE collection_key = ?");
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        self.query_store.execute(&sql, &params)
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let table = &self.table;
        let sql = format!("SELECT COUNT(*) as cnt FROM {table} WHERE collection_key = ?");
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        let rows = self.query_store.query(&sql, &params)?;
        let mut total = 0u64;
        for s in rows {
            if let Stream::Next(Ok(row)) = s {
                if let Ok(n) = row.get_by_name::<i64>("cnt") {
                    total = u64::try_from(n).unwrap_or(0);
                }
            }
        }
        Ok(total)
    }
}
