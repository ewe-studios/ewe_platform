//! SQL-backed document store over any [`AsyncQueryStore`].
//!
//! Documents are stored in a table with columns: `collection_key`, `doc_id`,
//! content, metadata, and the promoted `title`/`summary`/`record_type`
//! columns. This works with `SQLite`, Turso, libsql and D1.
//!
//! The SQL strings and param/row helpers live in the [`sql`] module so every
//! SQL-backed store (this one and [`super::d1r2_document_store`]) shares one
//! schema.

use crate::core::errors::StorageResult;
use crate::core::storage_provider::{
    AsyncDocumentStore, AsyncQueryStore, AsyncQueryStream, AsyncStorageItemStream, DataValue,
    Document, PromotableDocument, StorageError,
};
use futures_lite::StreamExt;
use serde::{de::DeserializeOwned, Serialize};

/// Shared SQL builders + row/param helpers for the SQL document stores.
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

/// SQL document store over any [`AsyncQueryStore`] backend (Turso, libsql,
/// native D1, Cloudflare D1 on wasm).
///
/// It implements only [`AsyncDocumentStore`]. Multi-item reads return an
/// [`AsyncStorageItemStream`] pulled one item at a time with `.next().await`,
/// never a `Vec`, so unbounded scans can't OOM (F06 OD-06-8).
pub struct SqlDocumentStore<Q> {
    query_store: Q,
    table: String,
}

/// A store over `Q::default()`, using the `documents` table. The backend must
/// already have that table: the `Default` impls of `TursoStorage` and
/// `LibsqlStore` migrate it for you.
impl<Q: Default> Default for SqlDocumentStore<Q> {
    fn default() -> Self {
        Self::new(Q::default())
    }
}

impl<Q> SqlDocumentStore<Q> {
    /// Create a new store backed by the given async query store.
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

/// Map a SQL row stream to a deserialized-`V` async item stream.
fn map_content<V: DeserializeOwned + 'static>(
    stream: AsyncQueryStream,
) -> AsyncStorageItemStream<'static, V> {
    Box::pin(stream.map(|r| r.and_then(|row| sql::deser_content::<V>(&row))))
}

impl<Q: AsyncQueryStore> SqlDocumentStore<Q> {
    async fn insert_row(
        &self,
        key: &str,
        doc_id: String,
        content_json: String,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        let params = sql::insert_params(
            key,
            &doc_id,
            &content_json,
            title.as_deref(),
            summary.as_deref(),
            record_type.as_deref(),
        );
        self.query_store
            .execute_async(&sql::insert_sql(&self.table), &params)
            .await?;
        Ok(Document {
            id: doc_id,
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type,
        })
    }

    /// Collect a `Document`-returning scan into a `Vec` (bounded reads).
    async fn collect_documents(
        &self,
        from_id: Option<&str>,
        key: &str,
        desc: bool,
        limit: i64,
    ) -> StorageResult<Vec<Document>> {
        let stmt = sql::scan_sql(&self.table, sql::DOC_COLUMNS, from_id.is_some(), desc, true);
        let params = sql::scan_params(key, from_id, Some(limit));
        let rows = self.query_store.query_async(&stmt, &params).await?.collect_all().await?;
        rows.iter().map(sql::row_to_document).collect()
    }
}

#[async_trait::async_trait]
impl<Q: AsyncQueryStore> AsyncDocumentStore for SqlDocumentStore<Q> {
    async fn append_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.insert_row(key, doc_id, json, None, None, None).await
    }

    async fn append_with_id_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.insert_row(key, doc_id.to_string(), json, None, None, None).await
    }

    async fn append_promotable_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.insert_row(key, doc_id, json, t, s, rt).await
    }

    async fn append_promotable_with_id_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        let json = serde_json::to_string(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.insert_row(key, doc_id.to_string(), json, t, s, rt).await
    }

    async fn scan_documents_async(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        self.collect_documents(None, key, true, sql::limit_to_i64(limit)).await
    }

    async fn scan_documents_from_async(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        self.collect_documents(Some(from_id), key, false, sql::limit_or_unlimited(limit)).await
    }

    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", false, true, true);
        let params = sql::scan_params(key, None, Some(sql::limit_to_i64(limit)));
        Ok(map_content(self.query_store.query_async(&stmt, &params).await?))
    }

    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", false, false, false);
        let params = sql::scan_params(key, None, None);
        Ok(map_content(self.query_store.query_async(&stmt, &params).await?))
    }

    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let stmt = sql::scan_sql(&self.table, "content", true, false, true);
        let params = sql::scan_params(key, Some(from_id), Some(sql::limit_or_unlimited(limit)));
        Ok(map_content(self.query_store.query_async(&stmt, &params).await?))
    }

    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let params = [
            DataValue::Text(key.to_string()),
            DataValue::Text(doc_id.to_string()),
        ];
        self.query_store.execute_async(&sql::delete_sql(&self.table), &params).await?;
        Ok(())
    }

    async fn delete_all_async(&self, key: &str) -> StorageResult<u64> {
        let params = [DataValue::Text(key.to_string())];
        self.query_store.execute_async(&sql::delete_all_sql(&self.table), &params).await
    }

    async fn count_async(&self, key: &str) -> StorageResult<u64> {
        let params = [DataValue::Text(key.to_string())];
        let rows = self
            .query_store
            .query_async(&sql::count_sql(&self.table), &params)
            .await?
            .collect_all()
            .await?;
        // Read by index (0) not name — async backends may not populate column
        // aliases on the returned row.
        let total = rows
            .first()
            .and_then(|row| row.get::<i64>(0).ok())
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(0);
        Ok(total)
    }
}
