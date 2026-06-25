//! SQL-backed `VectorStore` — stores vectors as BLOBs, queries with
//! `foundation_vectors::flat_top_k`. Works with any `QueryStore` backend
//! (Turso, libSQL, or any future SQL storage).

use std::sync::Arc;

use foundation_core::valtron::Stream;
use foundation_vectors::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;

use crate::core::storage_provider::{DataValue, QueryStore, SqlRow};

pub struct SqlVectorStore<Q: QueryStore> {
    backend: Arc<Q>,
    config: VectorStoreConfig,
}

impl<Q: QueryStore> SqlVectorStore<Q> {
    pub fn new(backend: Arc<Q>, config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        backend
            .execute_batch(include_str!("../schema/sql/023_create_vectors.sql"))
            .map_err(|e| VectorStoreError::Backend(format!("failed to create vectors table: {e}")))?;
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

fn metadata_to_json(m: &VectorMetadata) -> String {
    serde_json::to_string(m).unwrap_or_else(|_| "{}".into())
}

fn json_to_metadata(s: &str) -> VectorMetadata {
    serde_json::from_str(s).unwrap_or_default()
}

fn collect_rows(
    stream: crate::core::storage_provider::StorageItemStream<'_, SqlRow>,
) -> Result<Vec<SqlRow>, VectorStoreError> {
    let mut rows = Vec::new();
    for item in stream {
        match item {
            Stream::Next(Ok(row)) => rows.push(row),
            Stream::Next(Err(e)) => {
                return Err(VectorStoreError::Backend(format!("row error: {e}")))
            }
            _ => {}
        }
    }
    Ok(rows)
}

impl<Q: QueryStore> VectorStore for SqlVectorStore<Q> {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(&entry)?;

        let blob = vector_to_blob(&entry.vector);
        let meta_json = metadata_to_json(&entry.metadata);
        #[allow(clippy::cast_possible_wrap)]
        let dim = entry.vector.dimension() as i64;

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
            .map_err(|e| VectorStoreError::Backend(format!("insert failed: {e}")))?;

        Ok(())
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let affected = self
            .backend
            .execute(
                "DELETE FROM vectors WHERE namespace = ? AND id = ?",
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .map_err(|e| VectorStoreError::Backend(format!("delete failed: {e}")))?;

        if affected == 0 {
            return Err(VectorStoreError::NotFound { id: id.to_string() });
        }
        Ok(())
    }

    fn search(
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

        let stream = self
            .backend
            .query(
                "SELECT id, vector FROM vectors WHERE namespace = ?",
                &[DataValue::Text(namespace.to_string())],
            )
            .map_err(|e| VectorStoreError::Backend(format!("query failed: {e}")))?;

        let rows = collect_rows(stream)?;
        let mut entries: Vec<(String, Vec<f32>)> = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: String = row
                .get(0)
                .map_err(|e| VectorStoreError::Backend(format!("read id: {e}")))?;
            let blob: Vec<u8> = row
                .get(1)
                .map_err(|e| VectorStoreError::Backend(format!("read vector: {e}")))?;
            let vec = blob_to_vector(&blob);
            entries.push((id, vec.data));
        }

        let iter = entries
            .iter()
            .map(|(id, data)| (id.as_str(), data.as_slice()));
        Ok(foundation_vectors::flat_top_k(
            query,
            iter,
            k,
            self.config.metric,
        ))
    }

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        let stream = self
            .backend
            .query(
                "SELECT id, vector, metadata FROM vectors WHERE namespace = ? AND id = ?",
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .map_err(|e| VectorStoreError::Backend(format!("get failed: {e}")))?;

        let rows = collect_rows(stream)?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };

        let id: String = row
            .get(0)
            .map_err(|e| VectorStoreError::Backend(format!("read id: {e}")))?;
        let blob: Vec<u8> = row
            .get(1)
            .map_err(|e| VectorStoreError::Backend(format!("read vector: {e}")))?;
        let meta_json: String = row
            .get(2)
            .map_err(|e| VectorStoreError::Backend(format!("read metadata: {e}")))?;

        Ok(Some(VectorEntry {
            id,
            vector: blob_to_vector(&blob),
            metadata: json_to_metadata(&meta_json),
        }))
    }

    fn len(&self, namespace: &str) -> usize {
        let stream = match self.backend.query(
            "SELECT COUNT(*) FROM vectors WHERE namespace = ?",
            &[DataValue::Text(namespace.to_string())],
        ) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let rows = match collect_rows(stream) {
            Ok(r) => r,
            Err(_) => return 0,
        };
        rows.first()
            .and_then(|row| row.get::<i64>(0).ok())
            .unwrap_or(0) as usize
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
