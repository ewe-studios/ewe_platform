//! D1+R2 document store — transparent blob offload (F23).
//!
//! `D1R2DocumentStore<Q: AsyncQueryStore, B: AsyncBlobStore>` implements
//! [`AsyncDocumentStore`] with a SQL backend (`Q`) as the ordered index and a
//! blob backend (`B`) for large document payloads. Documents whose serialized
//! JSON exceeds the SQLite page size (4 KB) are offloaded to the blob store; the
//! SQL row keeps the promoted columns plus the blob key (`r2_key`) for
//! transparent read-through. Smaller documents stay inline in the SQL `content`
//! column.
//!
//! WHY: On Cloudflare, D1 (SQLite) is the strictly-ordered backend but big
//! Message-API records (F08) don't fit a SQLite row well; R2 (object storage)
//! holds the heavy bytes while D1 keeps the index. This is target-agnostic — the
//! same logic serves wasm/CF (`D1WasmStorage`+`R2WasmStorage`) and native
//! (`D1Store`+`R2Store`, e.g. against a wrangler/miniflare worker for tests).
//!
//! WHAT: `D1R2DocumentStore::new(query_store, blob_store)` over any
//! [`AsyncQueryStore`] + [`AsyncBlobStore`]. Reuses the F06 `documents` schema
//! plus the migration-022 `r2_key` column.
//!
//! HOW: `insert_row` serializes once, offloads to the blob store past the 4 KB
//! threshold, and writes the row (placeholder `content` + `r2_key` when
//! offloaded). Reads resolve `r2_key` back through the blob store. Scans go
//! through the trait's [`AsyncQueryStream`] and `collect_all` for bounded reads,
//! mirroring [`super::async_sql_document_store::AsyncSqlDocumentStore`].

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncDocumentStore, AsyncQueryStore, AsyncStorageItemStream, DataValue,
    Document, PromotableDocument, SqlRow,
};
use serde::{de::DeserializeOwned, Serialize};

/// Documents serializing to more than this many bytes are offloaded to the blob
/// store (SQLite default page size).
pub const R2_OFFLOAD_THRESHOLD: usize = 4096;

/// Columns read back for a full `Document` projection, including `r2_key`.
const DOC_COLUMNS_R2: &str = "doc_id, content, metadata, title, summary, record_type, r2_key";

/// Transparent D1+R2 document store over any async SQL + blob backends.
pub struct D1R2DocumentStore<Q, B> {
    query_store: Q,
    blob_store: B,
    table: String,
}

impl<Q, B> D1R2DocumentStore<Q, B> {
    /// Create a new store backed by the given async query + blob stores.
    pub fn new(query_store: Q, blob_store: B) -> Self {
        Self {
            query_store,
            blob_store,
            table: "documents".to_string(),
        }
    }

    /// Set the table name (default: "documents").
    #[must_use]
    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }

    /// Blob-store key for a document's offloaded payload.
    fn blob_key(collection: &str, doc_id: &str) -> String {
        format!("doc/{collection}/{doc_id}")
    }

    fn limit_to_i64(limit: usize) -> i64 {
        i64::try_from(limit).unwrap_or(i64::MAX)
    }

    fn limit_or_unlimited(limit: usize) -> i64 {
        if limit == 0 {
            -1
        } else {
            Self::limit_to_i64(limit)
        }
    }

    fn scan_sql(&self, columns: &str, from: bool, desc: bool, limited: bool) -> String {
        let mut s = format!("SELECT {columns} FROM {} WHERE collection_key = ?", self.table);
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

    fn scan_params(key: &str, from_id: Option<&str>, limit: Option<i64>) -> Vec<DataValue> {
        let mut p = vec![DataValue::Text(key.to_string())];
        if let Some(f) = from_id {
            p.push(DataValue::Text(f.to_string()));
        }
        if let Some(l) = limit {
            p.push(DataValue::Integer(l));
        }
        p
    }
}

impl<Q: AsyncQueryStore, B: AsyncBlobStore> D1R2DocumentStore<Q, B> {
    async fn insert_row(
        &self,
        key: &str,
        doc_id: String,
        content_json: String,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> StorageResult<Document> {
        let r2_key = if content_json.len() > R2_OFFLOAD_THRESHOLD {
            let rk = Self::blob_key(key, &doc_id);
            self.blob_store.put_blob_async(&rk, content_json.as_bytes()).await?;
            Some(rk)
        } else {
            None
        };

        // Offloaded rows keep a placeholder `content`; inline rows keep the JSON.
        let stored_content = if r2_key.is_some() {
            String::new()
        } else {
            content_json.clone()
        };

        let opt = |v: Option<&str>| v.map_or(DataValue::Null, |s| DataValue::Text(s.to_string()));
        let params = vec![
            DataValue::Text(key.to_string()),
            DataValue::Text(doc_id.clone()),
            DataValue::Text(stored_content),
            DataValue::Text("{}".to_string()),
            opt(title.as_deref()),
            opt(summary.as_deref()),
            opt(record_type.as_deref()),
            r2_key
                .as_ref()
                .map_or(DataValue::Null, |k| DataValue::Text(k.clone())),
        ];

        let sql = format!(
            "INSERT INTO {} \
             (collection_key, doc_id, content, metadata, title, summary, record_type, r2_key) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            self.table
        );
        self.query_store.execute_async(&sql, &params).await?;

        Ok(Document {
            id: doc_id,
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type,
        })
    }

    /// Resolve a row's content, fetching from the blob store when offloaded.
    async fn resolve_content(&self, inline: &str, r2_key: Option<&str>) -> StorageResult<String> {
        match r2_key {
            Some(rk) => {
                let blob = self
                    .blob_store
                    .get_blob_async(rk)
                    .await?
                    .ok_or_else(|| StorageError::Backend(format!("R2 blob not found: {rk}")))?;
                String::from_utf8(blob)
                    .map_err(|e| StorageError::Backend(format!("R2 blob not UTF-8: {e}")))
            }
            None => Ok(inline.to_string()),
        }
    }

    async fn row_to_document(&self, row: &SqlRow) -> StorageResult<Document> {
        let doc_id = row.get::<String>(0)?;
        let inline_content = row.get::<String>(1)?;
        let metadata = row
            .get::<String>(2)
            .ok()
            .and_then(|m| serde_json::from_str(&m).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let title = row.get::<Option<String>>(3)?;
        let summary = row.get::<Option<String>>(4)?;
        let record_type = row.get::<Option<String>>(5)?;
        let r2_key = row.get::<Option<String>>(6)?;

        let content = self.resolve_content(&inline_content, r2_key.as_deref()).await?;
        Ok(Document {
            id: doc_id,
            content,
            metadata,
            title,
            summary,
            record_type,
        })
    }

    async fn collect_documents(
        &self,
        from_id: Option<&str>,
        key: &str,
        desc: bool,
        limit: i64,
    ) -> StorageResult<Vec<Document>> {
        let stmt = self.scan_sql(DOC_COLUMNS_R2, from_id.is_some(), desc, true);
        let params = Self::scan_params(key, from_id, Some(limit));
        let rows = self.query_store.query_async(&stmt, &params).await?.collect_all().await?;

        let mut docs = Vec::with_capacity(rows.len());
        for row in &rows {
            docs.push(self.row_to_document(row).await?);
        }
        Ok(docs)
    }

    async fn scan_content<V: DeserializeOwned>(
        &self,
        from_id: Option<&str>,
        key: &str,
        desc: bool,
        limit: Option<i64>,
    ) -> StorageResult<Vec<StorageResult<V>>> {
        let stmt = self.scan_sql("content, r2_key", from_id.is_some(), desc, limit.is_some());
        let params = Self::scan_params(key, from_id, limit);
        let rows = self.query_store.query_async(&stmt, &params).await?.collect_all().await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            let inline = row.get::<String>(0)?;
            let r2_key = row.get::<Option<String>>(1)?;
            let actual = self.resolve_content(&inline, r2_key.as_deref()).await?;
            items.push(
                serde_json::from_str::<V>(&actual)
                    .map_err(|e| StorageError::Deserialization(e.to_string())),
            );
        }
        Ok(items)
    }
}

#[async_trait::async_trait]
impl<Q: AsyncQueryStore, B: AsyncBlobStore> AsyncDocumentStore for D1R2DocumentStore<Q, B> {
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
        self.collect_documents(None, key, true, Self::limit_to_i64(limit)).await
    }

    async fn scan_documents_from_async(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        self.collect_documents(Some(from_id), key, false, Self::limit_or_unlimited(limit)).await
    }

    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = self
            .scan_content::<V>(None, key, true, Some(Self::limit_to_i64(limit)))
            .await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = self.scan_content::<V>(None, key, false, None).await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = self
            .scan_content::<V>(Some(from_id), key, false, Some(Self::limit_or_unlimited(limit)))
            .await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let check_sql = format!(
            "SELECT r2_key FROM {} WHERE collection_key = ? AND doc_id = ?",
            self.table
        );
        let params = [
            DataValue::Text(key.to_string()),
            DataValue::Text(doc_id.to_string()),
        ];
        let rows = self.query_store.query_async(&check_sql, &params).await?.collect_all().await?;
        if let Some(row) = rows.first() {
            if let Ok(Some(rk)) = row.get::<Option<String>>(0) {
                self.blob_store.delete_blob_async(&rk).await?;
            }
        }

        let delete_sql = format!(
            "DELETE FROM {} WHERE collection_key = ? AND doc_id = ?",
            self.table
        );
        self.query_store.execute_async(&delete_sql, &params).await?;
        Ok(())
    }

    async fn delete_all_async(&self, key: &str) -> StorageResult<u64> {
        let select_sql = format!(
            "SELECT r2_key FROM {} WHERE collection_key = ? AND r2_key IS NOT NULL",
            self.table
        );
        let key_param = [DataValue::Text(key.to_string())];
        let rows = self.query_store.query_async(&select_sql, &key_param).await?.collect_all().await?;
        for row in &rows {
            if let Ok(rk) = row.get::<String>(0) {
                self.blob_store.delete_blob_async(&rk).await?;
            }
        }

        let delete_sql = format!("DELETE FROM {} WHERE collection_key = ?", self.table);
        self.query_store
            .execute_async(&delete_sql, &[DataValue::Text(key.to_string())])
            .await
    }

    async fn count_async(&self, key: &str) -> StorageResult<u64> {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE collection_key = ?",
            self.table
        );
        let rows = self
            .query_store
            .query_async(&sql, &[DataValue::Text(key.to_string())])
            .await?
            .collect_all()
            .await?;
        let total = rows
            .first()
            .and_then(|row| row.get::<i64>(0).ok())
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(0);
        Ok(total)
    }
}
