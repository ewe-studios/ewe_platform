//! Portable full-scan `VectorStore` over any async SQL store.
//!
//! Vectors are little-endian `f32` blobs in the `vectors` table; search scans
//! the namespace and ranks rows as they stream in (O(k) memory).

use std::sync::Arc;

use foundation_db::{QueryStore, DataValue};
use futures_lite::StreamExt;

use crate::flat::TopK;
use crate::schema;
use crate::store::{
    VectorStore, VectorEntry, VectorMatch, VectorMetadata, VectorStoreConfig,
    VectorStoreError,
};
use crate::vector::Vector;

pub struct SqlVectorStore<Q: QueryStore + 'static> {
    backend: Arc<Q>,
    config: VectorStoreConfig,
}

impl<Q: QueryStore + 'static> SqlVectorStore<Q> {
    /// Open a store over `backend`, running the vector migrations.
    ///
    /// # Errors
    ///
    /// Returns [`VectorStoreError::Backend`] if the migrations fail.
    pub async fn new(backend: Arc<Q>, config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        schema::run_migrations(backend.as_ref()).await?;
        Ok(Self { backend, config })
    }

    fn validate(&self, entry: &VectorEntry) -> Result<(), VectorStoreError> {
        if entry.vector.dimension() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: entry.vector.dimension(),
            });
        }
        if entry.vector.is_zero() {
            return Err(VectorStoreError::ZeroVector);
        }
        Ok(())
    }
}

fn vector_to_blob(v: &Vector) -> Vec<u8> {
    let floats = v.as_slice();
    let mut bytes = Vec::with_capacity(floats.len() * 4);
    for &f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

fn blob_to_vector(blob: &[u8]) -> Vector {
    let floats: Vec<f32> = blob
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Vector::new(floats)
}

fn metadata_to_json(m: &VectorMetadata) -> Result<String, VectorStoreError> {
    serde_json::to_string(m).map_err(|e| VectorStoreError::Backend(format!("encode metadata: {e}")))
}

fn json_to_metadata(s: &str) -> Result<VectorMetadata, VectorStoreError> {
    serde_json::from_str(s).map_err(|e| VectorStoreError::Backend(format!("decode metadata: {e}")))
}

fn backend_err<E: core::fmt::Display>(ctx: &'static str) -> impl Fn(E) -> VectorStoreError {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

#[async_trait::async_trait]
impl<Q: QueryStore + 'static> VectorStore for SqlVectorStore<Q> {
    async fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(&entry)?;
        let blob = vector_to_blob(&entry.vector);
        let meta_json = metadata_to_json(&entry.metadata)?;
        let dim = i64::try_from(entry.vector.dimension())
            .map_err(|_| VectorStoreError::Backend("dimension does not fit in i64".into()))?;
        self.backend
            .execute(
                "INSERT OR REPLACE INTO vectors (id, namespace, dimension, vector, metadata) \
                 VALUES (?, ?, ?, ?, ?)",
                &[
                    DataValue::Text(entry.id),
                    DataValue::Text(namespace.to_string()),
                    DataValue::Integer(dim),
                    DataValue::Blob(blob),
                    DataValue::Text(meta_json),
                ],
            )
            .await
            .map_err(backend_err("insert failed"))?;
        Ok(())
    }

    async fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let affected = self
            .backend
            .execute(
                "DELETE FROM vectors WHERE namespace = ? AND id = ?",
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .await
            .map_err(backend_err("delete failed"))?;
        if affected == 0 {
            return Err(VectorStoreError::NotFound { id: id.to_string() });
        }
        Ok(())
    }

    async fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        if query.len() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: query.len(),
            });
        }
        let mut rows = self
            .backend
            .query(
                "SELECT id, vector FROM vectors WHERE namespace = ?",
                &[DataValue::Text(namespace.to_string())],
            )
            .await
            .map_err(backend_err("query failed"))?;
        // Rank rows as they arrive: memory stays at k, not the namespace size.
        let mut top = TopK::new(query, k, self.config.metric);
        while let Some(row) = rows.next().await {
            let row = row.map_err(backend_err("row error"))?;
            let id: String = row.get(0).map_err(backend_err("read id"))?;
            let blob: Vec<u8> = row.get(1).map_err(backend_err("read vector"))?;
            top.push(&id, &blob_to_vector(&blob).data);
        }
        Ok(top.finish())
    }

    async fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        let mut rows = self
            .backend
            .query(
                "SELECT id, vector, metadata FROM vectors WHERE namespace = ? AND id = ?",
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .await
            .map_err(backend_err("get failed"))?;
        let Some(row) = rows.next().await else {
            return Ok(None);
        };
        let row = row.map_err(backend_err("row error"))?;
        let id: String = row.get(0).map_err(backend_err("read id"))?;
        let blob: Vec<u8> = row.get(1).map_err(backend_err("read vector"))?;
        let meta_json: String = row.get(2).map_err(backend_err("read metadata"))?;
        Ok(Some(VectorEntry {
            id,
            vector: blob_to_vector(&blob),
            metadata: json_to_metadata(&meta_json)?,
        }))
    }

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError> {
        count_rows(
            self.backend.as_ref(),
            "SELECT COUNT(*) FROM vectors WHERE namespace = ?",
            namespace,
        )
        .await
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

/// Run a `SELECT COUNT(*) … WHERE namespace = ?` and read the count.
pub(crate) async fn count_rows<Q: QueryStore + ?Sized>(
    backend: &Q,
    sql: &str,
    namespace: &str,
) -> Result<usize, VectorStoreError> {
    let mut rows = backend
        .query(sql, &[DataValue::Text(namespace.to_string())])
        .await
        .map_err(backend_err("count failed"))?;
    let Some(row) = rows.next().await else {
        return Ok(0);
    };
    let n: i64 = row.map_err(backend_err("row error"))?.get(0).map_err(backend_err("read count"))?;
    usize::try_from(n).map_err(backend_err("count out of range"))
}
