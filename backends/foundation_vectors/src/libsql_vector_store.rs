//! Native DiskANN `VectorStore` over libSQL's built-in vector index (F29).
//!
//! WHY: libSQL ships a native **DiskANN** ANN index (`libsql_vector_idx` +
//! `vector_top_k`), so nearest-neighbour search is sub-linear on the database
//! side instead of a full scan. This is the fast native path; the generic
//! [`super::sql_store::SqlVectorStore`] is the portable full-scan fallback for
//! backends without native vectors (the default `turso` crate has only scalar
//! `vector_distance_*`).
//!
//! WHAT: [`LibsqlVectorStore`] over any [`QueryStore`] whose backend understands
//! libSQL vector SQL (i.e. `foundation_db::LibsqlStore`). Implements
//! [`VectorStore`] with dimension + namespace enforcement.
//!
//! HOW: vectors live in a `FLOAT32({dim})` column with a `libsql_vector_idx`
//! index. `vector_top_k('idx', q, k')` is a table-valued function over the
//! **whole** index — it can't filter by namespace (OD-29-4) — so search
//! **over-fetches** `k' = k * multiplier` candidates, joins them back to recover
//! `namespace`/`id`/embedding, filters to the target namespace, and re-ranks the
//! survivors exactly with [`flat_top_k`] under the store's configured metric (the
//! native index orders by its own metric; the re-rank guarantees the requested
//! one). `NamespaceStrategy::OverFetch { multiplier }` (default 4) trades recall
//! for fewer rows; small namespaces in a large index may need a larger multiplier.

use std::sync::Arc;

use foundation_core::valtron::Stream;
use foundation_db::traits::{DataValue, QueryStore, SqlRow};

use crate::flat::flat_top_k;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

const TABLE: &str = "libsql_vectors";
const INDEX: &str = "libsql_vectors_idx";

/// How to reconcile namespace filtering with the whole-index `vector_top_k` TVF.
#[derive(Debug, Clone, Copy)]
pub enum NamespaceStrategy {
    /// Fetch `k * multiplier` candidates from the index, then filter by namespace
    /// and re-rank. Risk: a namespace that is a small fraction of the index may
    /// under-return; raise the multiplier to compensate.
    OverFetch { multiplier: usize },
}

impl Default for NamespaceStrategy {
    fn default() -> Self {
        Self::OverFetch { multiplier: 4 }
    }
}

pub struct LibsqlVectorStore<Q: QueryStore> {
    backend: Arc<Q>,
    config: VectorStoreConfig,
    strategy: NamespaceStrategy,
}

impl<Q: QueryStore> LibsqlVectorStore<Q> {
    /// Open a store over `backend`, creating the vector table + DiskANN index.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] if the schema cannot be created
    /// (e.g. the backend doesn't support libSQL vector SQL).
    pub fn new(backend: Arc<Q>, config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        let create = format!(
            "CREATE TABLE IF NOT EXISTS {TABLE} (\
                rowid INTEGER PRIMARY KEY AUTOINCREMENT, \
                id TEXT NOT NULL, namespace TEXT NOT NULL, \
                embedding FLOAT32({dim}), \
                metadata TEXT NOT NULL DEFAULT '{{}}', \
                UNIQUE(namespace, id));",
            dim = config.dimension
        );
        backend
            .execute_batch(&create)
            .map_err(backend_err("create table"))?;
        backend
            .execute_batch(&format!(
                "CREATE INDEX IF NOT EXISTS {INDEX} ON {TABLE}(libsql_vector_idx(embedding));"
            ))
            .map_err(backend_err("create index"))?;
        Ok(Self {
            backend,
            config,
            strategy: NamespaceStrategy::default(),
        })
    }

    /// Override the namespace strategy (default: over-fetch ×4).
    #[must_use]
    pub fn with_namespace_strategy(mut self, strategy: NamespaceStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    fn validate(&self, dim: usize, zero: bool) -> Result<(), VectorStoreError> {
        if dim != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: dim,
            });
        }
        if zero {
            return Err(VectorStoreError::ZeroVector);
        }
        Ok(())
    }
}

fn backend_err<E: core::fmt::Display>(ctx: &'static str) -> impl Fn(E) -> VectorStoreError {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

/// libSQL's `vector('[..]')` accepts a JSON-array string.
fn vec_to_str(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 8 + 2);
    s.push('[');
    for (i, f) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&f.to_string());
    }
    s.push(']');
    s
}

/// Parse `vector_extract`'s `[a,b,c]` output back to floats.
fn parse_vec_str(s: &str) -> Vec<f32> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|p| p.trim().parse::<f32>().ok())
        .collect()
}

fn collect_rows(
    stream: foundation_db::traits::StorageItemStream<'_, SqlRow>,
) -> Result<Vec<SqlRow>, VectorStoreError> {
    let mut rows = Vec::new();
    for item in stream {
        match item {
            Stream::Next(Ok(row)) => rows.push(row),
            Stream::Next(Err(e)) => return Err(VectorStoreError::Backend(format!("row: {e}"))),
            _ => {}
        }
    }
    Ok(rows)
}

impl<Q: QueryStore> VectorStore for LibsqlVectorStore<Q> {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let meta = serde_json::to_string(&entry.metadata).unwrap_or_else(|_| "{}".into());
        self.backend
            .execute(
                &format!(
                    "INSERT OR REPLACE INTO {TABLE}(id, namespace, embedding, metadata) \
                     VALUES (?, ?, vector(?), ?)"
                ),
                &[
                    DataValue::Text(entry.id),
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(vec_to_str(&entry.vector.data)),
                    DataValue::Text(meta),
                ],
            )
            .map_err(backend_err("insert"))?;
        Ok(())
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let affected = self
            .backend
            .execute(
                &format!("DELETE FROM {TABLE} WHERE namespace = ? AND id = ?"),
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .map_err(backend_err("delete"))?;
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
        self.validate(query.len(), false)?;
        if k == 0 {
            return Ok(Vec::new());
        }
        let NamespaceStrategy::OverFetch { multiplier } = self.strategy;
        #[allow(clippy::cast_possible_wrap)]
        let fetch = (k.saturating_mul(multiplier.max(1))) as i64;

        // DiskANN candidate retrieval (whole index), then namespace filter.
        let stream = self
            .backend
            .query(
                &format!(
                    "SELECT t.id, vector_extract(t.embedding) \
                     FROM vector_top_k('{INDEX}', vector(?), ?) AS knn \
                     JOIN {TABLE} t ON t.rowid = knn.id \
                     WHERE t.namespace = ?"
                ),
                &[
                    DataValue::Text(vec_to_str(query)),
                    DataValue::Integer(fetch),
                    DataValue::Text(namespace.to_string()),
                ],
            )
            .map_err(backend_err("vector_top_k"))?;
        let rows = collect_rows(stream)?;

        let mut candidates: Vec<(String, Vec<f32>)> = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: String = row.get(0).map_err(backend_err("read id"))?;
            let vec_str: String = row.get(1).map_err(backend_err("read embedding"))?;
            candidates.push((id, parse_vec_str(&vec_str)));
        }
        // Exact re-rank under the configured metric (the index orders by its own).
        let iter = candidates.iter().map(|(id, v)| (id.as_str(), v.as_slice()));
        Ok(flat_top_k(query, iter, k, self.config.metric))
    }

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        let stream = self
            .backend
            .query(
                &format!(
                    "SELECT vector_extract(embedding), metadata FROM {TABLE} \
                     WHERE namespace = ? AND id = ?"
                ),
                &[
                    DataValue::Text(namespace.to_string()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .map_err(backend_err("get"))?;
        let rows = collect_rows(stream)?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let vec_str: String = row.get(0).map_err(backend_err("read embedding"))?;
        let meta_str: String = row.get(1).map_err(backend_err("read metadata"))?;
        let metadata: VectorMetadata = serde_json::from_str(&meta_str).unwrap_or_default();
        Ok(Some(VectorEntry {
            id: id.to_string(),
            vector: Vector::new(parse_vec_str(&vec_str)),
            metadata,
        }))
    }

    fn len(&self, namespace: &str) -> usize {
        let Ok(stream) = self.backend.query(
            &format!("SELECT COUNT(*) FROM {TABLE} WHERE namespace = ?"),
            &[DataValue::Text(namespace.to_string())],
        ) else {
            return 0;
        };
        let Ok(rows) = collect_rows(stream) else {
            return 0;
        };
        rows.first()
            .and_then(|r| r.get::<i64>(0).ok())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0)
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
