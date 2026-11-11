//! Portable full-scan `VectorStore` over any async SQL store.
//!
//! WHY: `SqlVectorStore` gives every async SQL backend a vector search without
//! a native vector index. It is the fallback that works anywhere `QueryStore`
//! works, including Cloudflare D1 (fetch-then-search).
//!
//! WHAT: vectors are little-endian `f32` blobs in the `vectors` table; `search`
//! scans the namespace and ranks rows as they stream in. Memory stays at
//! O(k) (the top-k heap), not the namespace size.
//!
//! HOW: a search reads every row of the namespace and scores each one, so its
//! time is O(n·d) — the row count times the vector dimension. That is expected
//! for a portable fallback. Large namespaces should use `LibsqlVectorStore`
//! (a native index) or a dedicated vector backend instead. When one scan reads
//! more rows than [`SqlVectorStoreConfig::scan_warning_threshold`] (default
//! 100 000), the store writes one `tracing::warn!` per namespace, so a large
//! namespace is loud once and quiet after that.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use foundation_db::{DataValue, QueryStore};
use futures_lite::StreamExt;

use crate::flat::TopK;
use crate::schema;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

/// Tuning for the portable full-scan SQL store.
///
/// Rule 8: the warning threshold is configuration, not a constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlVectorStoreConfig {
    /// The most rows in one namespace that are scanned without a warning. A
    /// namespace larger than this gets one `tracing::warn!` per process.
    ///
    /// Default 100 000. The portable fallback scores every row of the
    /// namespace (O(n·d)); a `LibsqlVectorStore` (native index) or a dedicated
    /// vector backend is the right choice past this size.
    pub scan_warning_threshold: usize,
}

impl Default for SqlVectorStoreConfig {
    fn default() -> Self {
        Self {
            scan_warning_threshold: 100_000,
        }
    }
}

impl SqlVectorStoreConfig {
    /// Warn once per namespace when a scan reads more than `rows` rows.
    #[must_use]
    pub fn with_scan_warning_threshold(mut self, rows: usize) -> Self {
        self.scan_warning_threshold = rows;
        self
    }
}

pub struct SqlVectorStore<Q: QueryStore + 'static> {
    backend: Arc<Q>,
    config: VectorStoreConfig,
    scan: SqlVectorStoreConfig,
    warned: Mutex<HashSet<String>>,
}

impl<Q: QueryStore + 'static> SqlVectorStore<Q> {
    /// Open a store over `backend`, running the vector migrations. The scan
    /// warning threshold is [`SqlVectorStoreConfig::default`].
    ///
    /// # Errors
    ///
    /// Returns [`VectorStoreError::Backend`] if the migrations fail.
    pub async fn new(backend: Arc<Q>, config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        Self::with_scan_config(backend, config, SqlVectorStoreConfig::default()).await
    }

    /// Open a store over `backend` with the given scan tuning, running the
    /// vector migrations.
    ///
    /// # Errors
    ///
    /// Returns [`VectorStoreError::Backend`] if the migrations fail.
    pub async fn with_scan_config(
        backend: Arc<Q>,
        config: VectorStoreConfig,
        scan: SqlVectorStoreConfig,
    ) -> Result<Self, VectorStoreError> {
        schema::run_migrations(backend.as_ref()).await?;
        Ok(Self {
            backend,
            config,
            scan,
            warned: Mutex::new(HashSet::new()),
        })
    }

    /// Write one warning per namespace the first time a scan passes the
    /// threshold. Later scans of the same namespace stay quiet.
    fn warn_namespace(&self, namespace: &str) {
        let mut warned = self
            .warned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if warned.insert(namespace.to_string()) {
            tracing::warn!(
                namespace = namespace,
                scan_warning_threshold = self.scan.scan_warning_threshold,
                "full namespace scan passed the warning threshold; use a native index for large namespaces"
            );
        }
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
        .as_chunks::<4>()
        .0
        .iter()
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
        let mut scanned: usize = 0;
        while let Some(row) = rows.next().await {
            let row = row.map_err(backend_err("row error"))?;
            let id: String = row.get(0).map_err(backend_err("read id"))?;
            let blob: Vec<u8> = row.get(1).map_err(backend_err("read vector"))?;
            top.push(&id, &blob_to_vector(&blob).data);
            scanned = scanned.saturating_add(1);
        }
        if scanned > self.scan.scan_warning_threshold {
            self.warn_namespace(namespace);
        }
        Ok(top.finish())
    }

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
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
    let n: i64 = row
        .map_err(backend_err("row error"))?
        .get(0)
        .map_err(backend_err("read count"))?;
    usize::try_from(n).map_err(backend_err("count out of range"))
}
