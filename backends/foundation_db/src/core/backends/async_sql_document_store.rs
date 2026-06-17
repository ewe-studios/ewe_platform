//! Async SQL-backed `AsyncDocumentStore` (F22b).
//!
//! `AsyncSqlDocumentStore<Q: AsyncQueryStore>` is the **canonical async** document
//! store for every SQL backend — Turso, Libsql, native D1, and wasm/CF D1 (which
//! F23 reuses). It mirrors the sync [`super::sql_document_store::SqlDocumentStore`]
//! exactly: the SQL strings, params, promoted columns and `Document` projection all
//! come from the shared [`super::sql_document_store::sql`] module, so the two never
//! drift. Multi-item reads return an [`AsyncStorageItemStream`] pulled one item at a
//! time with `.next().await` — never a `Vec` — so unbounded scans can't OOM
//! (F06 OD-06-8).

use crate::core::errors::StorageResult;
use crate::core::storage_provider::{
    AsyncDocumentStore, AsyncQueryStore, AsyncQueryStream, AsyncStorageItemStream, DataValue,
    Document, PromotableDocument, StorageError,
};
use futures_lite::StreamExt;
use serde::{de::DeserializeOwned, Serialize};

use super::sql_document_store::sql;

/// Async SQL document store over any [`AsyncQueryStore`] backend.
pub struct AsyncSqlDocumentStore<Q> {
    query_store: Q,
    table: String,
}

impl<Q> AsyncSqlDocumentStore<Q> {
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

impl<Q: AsyncQueryStore> AsyncSqlDocumentStore<Q> {
    async fn insert_promoted<V: Serialize + Send + 'static>(
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
        let params = sql::insert_params(key, &doc_id, &content_json, &title, &summary, &record_type);
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

#[async_trait::async_trait(?Send)]
impl<Q: AsyncQueryStore> AsyncDocumentStore for AsyncSqlDocumentStore<Q> {
    async fn append_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        self.insert_promoted(key, doc_id, &content, None, None, None).await
    }

    async fn append_with_id_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.insert_promoted(key, doc_id.to_string(), &content, None, None, None).await
    }

    async fn append_promotable_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let doc_id = foundation_compact::ids::new_scru128_string();
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.insert_promoted(key, doc_id, &content, t, s, rt).await
    }

    async fn append_promotable_with_id_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (t, s, rt) = (content.title(), content.summary(), content.record_type());
        self.insert_promoted(key, doc_id.to_string(), &content, t, s, rt).await
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
