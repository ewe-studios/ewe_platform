//! SQL-backed `DocumentStore` implementation.
//!
//! Documents are stored in a table with columns: `collection_key`, `doc_id`, content, metadata, `created_at`.
//! This works with `SQLite`, Turso, D1, and any SQL backend via `QueryStore`.
//!
//! The SQL strings + param/row helpers in the [`sql`] module are **shared** with
//! the async [`super::async_sql_document_store::AsyncSqlDocumentStore`] (F22b) so
//! the schema and queries never drift between the sync and async backends.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    Document, DocumentStore, PromotableDocument, SqlRow, StorageItemStream,
};
use foundation_core::valtron::Stream;
use serde::{de::DeserializeOwned, Serialize};

/// Shared SQL builders + row/param helpers for the sync + async SQL document stores.
pub(crate) mod sql {
    use crate::core::errors::{StorageError, StorageResult};
    use crate::core::storage_provider::{DataValue, Document, SqlRow};
    use serde::de::DeserializeOwned;

    /// Projection for `Document`-returning reads (promoted columns observable).
    pub const DOC_COLUMNS: &str = "doc_id, content, metadata, title, summary, record_type";

    /// Clamp a `usize` limit to an `i64` SQL parameter (saturating).
    pub fn limit_to_i64(limit: usize) -> i64 {
        i64::try_from(limit).unwrap_or(i64::MAX)
    }

    /// `0` means "unlimited" → SQL `-1`; otherwise the saturated limit.
    pub fn limit_or_unlimited(limit: usize) -> i64 {
        if limit == 0 {
            -1
        } else {
            limit_to_i64(limit)
        }
    }

    /// `INSERT` with all promoted columns.
    pub fn insert_sql(table: &str) -> String {
        format!(
            "INSERT INTO {table} \
             (collection_key, doc_id, content, metadata, title, summary, record_type) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
    }

    /// Params for [`insert_sql`] (metadata is always `{}`; promoted cols → NULL when absent).
    pub fn insert_params(
        key: &str,
        doc_id: &str,
        content_json: &str,
        title: Option<&str>,
        summary: Option<&str>,
        record_type: Option<&str>,
    ) -> Vec<DataValue> {
        let opt = |v: Option<&str>| v.map_or(DataValue::Null, |s| DataValue::Text(s.to_string()));
        vec![
            DataValue::Text(key.to_string()),
            DataValue::Text(doc_id.to_string()),
            DataValue::Text(content_json.to_string()),
            DataValue::Text("{}".to_string()),
            opt(title),
            opt(summary),
            opt(record_type),
        ]
    }

    /// Build a scan `SELECT`. `columns` is the projection; `from` adds the
    /// `doc_id >= ?` cursor; `desc` orders newest-first; `limited` appends `LIMIT ?`.
    pub fn scan_sql(table: &str, columns: &str, from: bool, desc: bool, limited: bool) -> String {
        let mut s = format!("SELECT {columns} FROM {table} WHERE collection_key = ?");
        if from {
            s.push_str(" AND doc_id >= ?");
        }
        s.push_str(if desc {
            " ORDER BY doc_id DESC"
        } else {
            " ORDER BY doc_id ASC"
        });
        if limited {
            s.push_str(" LIMIT ?");
        }
        s
    }

    /// Params for a scan: `key`, then optional `from_id`, then optional `limit`.
    pub fn scan_params(key: &str, from_id: Option<&str>, limit: Option<i64>) -> Vec<DataValue> {
        let mut p = vec![DataValue::Text(key.to_string())];
        if let Some(f) = from_id {
            p.push(DataValue::Text(f.to_string()));
        }
        if let Some(l) = limit {
            p.push(DataValue::Integer(l));
        }
        p
    }

    pub fn delete_sql(table: &str) -> String {
        format!("DELETE FROM {table} WHERE collection_key = ? AND doc_id = ?")
    }

    pub fn delete_all_sql(table: &str) -> String {
        format!("DELETE FROM {table} WHERE collection_key = ?")
    }

    pub fn count_sql(table: &str) -> String {
        format!("SELECT COUNT(*) as cnt FROM {table} WHERE collection_key = ?")
    }

    /// Deserialize the `content` column (index 0) of a row into `V`.
    pub fn deser_content<V: DeserializeOwned>(row: &SqlRow) -> StorageResult<V> {
        let content = row.get::<String>(0)?;
        serde_json::from_str::<V>(&content)
            .map_err(|e| StorageError::Deserialization(e.to_string()))
    }

    /// Build a `Document` from a row selecting [`DOC_COLUMNS`] (in that order).
    pub fn row_to_document(row: &SqlRow) -> StorageResult<Document> {
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
}

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
        let content_json = serde_json::to_string(content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let params = sql::insert_params(
            key,
            &doc_id,
            &content_json,
            title.as_deref(),
            summary.as_deref(),
            record_type.as_deref(),
        );
        self.query_store.execute(&sql::insert_sql(&self.table), &params)?;

        Ok(Document {
            id: doc_id,
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type,
        })
    }
}

/// Deserialize the `content` column (index 0) of a query row into a stream item.
fn content_to_item<V: DeserializeOwned>(
    s: Stream<Result<SqlRow, StorageError>, ()>,
) -> Option<Stream<Result<V, StorageError>, ()>> {
    match s {
        Stream::Next(Ok(row)) => Some(Stream::Next(sql::deser_content::<V>(&row))),
        Stream::Next(Err(e)) => Some(Stream::Next(Err(e))),
        _ => None,
    }
}

/// Collect a [`sql::DOC_COLUMNS`] stream into `Vec<Document>`.
fn collect_documents(
    rows: Stream<Result<SqlRow, StorageError>, ()>,
) -> Option<StorageResult<Document>> {
    match rows {
        Stream::Next(Ok(row)) => Some(sql::row_to_document(&row)),
        Stream::Next(Err(e)) => Some(Err(e)),
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
        let stmt = sql::scan_sql(&self.table, sql::DOC_COLUMNS, false, true, true);
        let params = sql::scan_params(key, None, Some(sql::limit_to_i64(limit)));
        let rows = self.query_store.query(&stmt, &params)?;
        rows.filter_map(collect_documents).collect()
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let stmt = sql::scan_sql(&self.table, sql::DOC_COLUMNS, true, false, true);
        let params = sql::scan_params(key, Some(from_id), Some(sql::limit_or_unlimited(limit)));
        let rows = self.query_store.query(&stmt, &params)?;
        rows.filter_map(collect_documents).collect()
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", false, true, true);
        let params = sql::scan_params(key, None, Some(sql::limit_to_i64(limit)));
        let rows = self.query_store.query(&stmt, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", false, false, false);
        let params = sql::scan_params(key, None, None);
        let rows = self.query_store.query(&stmt, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", true, false, true);
        let params = sql::scan_params(key, Some(from_id), Some(sql::limit_or_unlimited(limit)));
        let rows = self.query_store.query(&stmt, &params)?;
        Ok(Box::new(rows.filter_map(content_to_item::<V>)))
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let params = [
            crate::core::storage_provider::DataValue::Text(key.to_string()),
            crate::core::storage_provider::DataValue::Text(doc_id.to_string()),
        ];
        self.query_store.execute(&sql::delete_sql(&self.table), &params)?;
        Ok(())
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        self.query_store.execute(&sql::delete_all_sql(&self.table), &params)
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let params = [crate::core::storage_provider::DataValue::Text(key.to_string())];
        let rows = self.query_store.query(&sql::count_sql(&self.table), &params)?;
        let mut total = 0u64;
        for s in rows {
            if let Stream::Next(Ok(row)) = s {
                // Read by index (0) not alias — robust across backends.
                if let Ok(n) = row.get::<i64>(0) {
                    total = u64::try_from(n).unwrap_or(0);
                }
            }
        }
        Ok(total)
    }
}
