//! D1+R2 document store — transparent blob offload (F23).
//!
//! `D1R2DocumentStore` implements `AsyncDocumentStore` with D1 as the ordered
//! SQL backend and R2 for large document blobs. Documents smaller than the
//! SQLite page size (4 KB) stay inline in D1; larger ones are offloaded to R2
//! with the D1 row retaining promoted columns and the R2 key for read-through.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncDocumentStore, AsyncStorageItemStream, DataValue, Document, PromotableDocument, SqlRow,
};
use crate::wasm::wasm_storage::d1_wasm::D1WasmStorage;
use crate::wasm::wasm_storage::r2_wasm::R2WasmStorage;
use serde::{de::DeserializeOwned, Serialize};

const R2_OFFLOAD_THRESHOLD: usize = 4096;

const DOC_COLUMNS_R2: &str = "doc_id, content, metadata, title, summary, record_type, r2_key";

pub struct D1R2DocumentStore {
    d1: D1WasmStorage,
    r2: R2WasmStorage,
    table: String,
}

unsafe impl Send for D1R2DocumentStore {}
unsafe impl Sync for D1R2DocumentStore {}

impl D1R2DocumentStore {
    #[must_use]
    pub fn new(d1: D1WasmStorage, r2: R2WasmStorage) -> Self {
        Self {
            d1,
            r2,
            table: "documents".to_string(),
        }
    }

    #[must_use]
    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }

    fn r2_object_key(collection: &str, doc_id: &str) -> String {
        format!("doc/{collection}/{doc_id}")
    }

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
            let rk = Self::r2_object_key(key, &doc_id);
            self.r2
                .put_blob_async(&rk, content_json.as_bytes())
                .await?;
            Some(rk)
        } else {
            None
        };

        let stored_content = if r2_key.is_some() {
            String::new()
        } else {
            content_json.clone()
        };

        let opt =
            |v: Option<&str>| v.map_or(DataValue::Null, |s| DataValue::Text(s.to_string()));
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

        self.d1.execute_async(&sql, &params).await?;

        Ok(Document {
            id: doc_id,
            content: content_json,
            metadata: serde_json::json!({}),
            title,
            summary,
            record_type,
        })
    }

    async fn resolve_content(
        &self,
        inline: &str,
        r2_key: Option<&str>,
    ) -> StorageResult<String> {
        match r2_key {
            Some(rk) => {
                let blob = self
                    .r2
                    .get_blob_async(rk)
                    .await?
                    .ok_or_else(|| {
                        StorageError::Backend(format!("R2 blob not found: {rk}"))
                    })?;
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

        let content = self
            .resolve_content(&inline_content, r2_key.as_deref())
            .await?;

        Ok(Document {
            id: doc_id,
            content,
            metadata,
            title,
            summary,
            record_type,
        })
    }

    fn scan_sql(&self, columns: &str, from: bool, desc: bool, limited: bool) -> String {
        let mut s = format!(
            "SELECT {columns} FROM {} WHERE collection_key = ?",
            self.table
        );
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

    async fn collect_documents(
        &self,
        from_id: Option<&str>,
        key: &str,
        desc: bool,
        limit: i64,
    ) -> StorageResult<Vec<Document>> {
        let stmt = self.scan_sql(DOC_COLUMNS_R2, from_id.is_some(), desc, true);
        let params = Self::scan_params(key, from_id, Some(limit));
        let rows = self.d1.query_async(&stmt, &params).await?;

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
        let rows = self.d1.query_async(&stmt, &params).await?;

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
impl AsyncDocumentStore for D1R2DocumentStore {
    async fn append_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        foundation_compact::SendWrapper::new(async move {
            let doc_id = foundation_compact::ids::new_scru128_string();
            let json = serde_json::to_string(&content)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.insert_row(key, doc_id, json, None, None, None).await
        })
        .await
    }

    async fn append_with_id_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        foundation_compact::SendWrapper::new(async move {
            let json = serde_json::to_string(&content)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.insert_row(key, doc_id.to_string(), json, None, None, None)
                .await
        })
        .await
    }

    async fn append_promotable_async<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        foundation_compact::SendWrapper::new(async move {
            let doc_id = foundation_compact::ids::new_scru128_string();
            let (t, s, rt) = (content.title(), content.summary(), content.record_type());
            let json = serde_json::to_string(&content)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.insert_row(key, doc_id, json, t, s, rt).await
        })
        .await
    }

    async fn append_promotable_with_id_async<
        V: Serialize + PromotableDocument + Send + 'static,
    >(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        foundation_compact::SendWrapper::new(async move {
            let (t, s, rt) = (content.title(), content.summary(), content.record_type());
            let json = serde_json::to_string(&content)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.insert_row(key, doc_id.to_string(), json, t, s, rt)
                .await
        })
        .await
    }

    async fn scan_documents_async(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        foundation_compact::SendWrapper::new(async move {
            self.collect_documents(None, key, true, Self::limit_to_i64(limit))
                .await
        })
        .await
    }

    async fn scan_documents_from_async(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        foundation_compact::SendWrapper::new(async move {
            self.collect_documents(
                Some(from_id),
                key,
                false,
                Self::limit_or_unlimited(limit),
            )
            .await
        })
        .await
    }

    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = foundation_compact::SendWrapper::new(async move {
            self.scan_content::<V>(None, key, true, Some(Self::limit_to_i64(limit)))
                .await
        })
        .await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = foundation_compact::SendWrapper::new(async move {
            self.scan_content::<V>(None, key, false, None).await
        })
        .await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let items = foundation_compact::SendWrapper::new(async move {
            self.scan_content::<V>(
                Some(from_id),
                key,
                false,
                Some(Self::limit_or_unlimited(limit)),
            )
            .await
        })
        .await?;
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move {
            let check_sql = format!(
                "SELECT r2_key FROM {} WHERE collection_key = ? AND doc_id = ?",
                self.table
            );
            let params = [
                DataValue::Text(key.to_string()),
                DataValue::Text(doc_id.to_string()),
            ];
            let rows = self.d1.query_async(&check_sql, &params).await?;

            if let Some(row) = rows.first() {
                if let Ok(Some(rk)) = row.get::<Option<String>>(0) {
                    self.r2.delete_blob_async(&rk).await?;
                }
            }

            let delete_sql = format!(
                "DELETE FROM {} WHERE collection_key = ? AND doc_id = ?",
                self.table
            );
            self.d1.execute_async(&delete_sql, &params).await?;
            Ok(())
        })
        .await
    }

    async fn delete_all_async(&self, key: &str) -> StorageResult<u64> {
        foundation_compact::SendWrapper::new(async move {
            let select_sql = format!(
                "SELECT r2_key FROM {} WHERE collection_key = ? AND r2_key IS NOT NULL",
                self.table
            );
            let key_param = [DataValue::Text(key.to_string())];
            let rows = self.d1.query_async(&select_sql, &key_param).await?;

            for row in &rows {
                if let Ok(rk) = row.get::<String>(0) {
                    self.r2.delete_blob_async(&rk).await?;
                }
            }

            let delete_sql =
                format!("DELETE FROM {} WHERE collection_key = ?", self.table);
            self.d1
                .execute_async(&delete_sql, &[DataValue::Text(key.to_string())])
                .await
        })
        .await
    }

    async fn count_async(&self, key: &str) -> StorageResult<u64> {
        foundation_compact::SendWrapper::new(async move {
            let sql =
                format!("SELECT COUNT(*) as cnt FROM {} WHERE collection_key = ?", self.table);
            let rows = self
                .d1
                .query_async(&sql, &[DataValue::Text(key.to_string())])
                .await?;
            let total = rows
                .first()
                .and_then(|row| row.get::<i64>(0).ok())
                .and_then(|n| u64::try_from(n).ok())
                .unwrap_or(0);
            Ok(total)
        })
        .await
    }
}
