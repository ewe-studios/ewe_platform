//! SQLite vector store via **sqlite-vec** loaded as a dynamic extension (F29).
//!
//! WHY: `sqlite-vec` provides native `vec0` virtual tables with k-nearest
//! neighbour (KNN) queries on SQLite-compatible engines. The extension is
//! loaded at runtime from a bundled `.so` (written to a temp dir), so no system
//! install is needed.
//!
//! WHAT: [`SqliteVecVectorStore`] implements the async [`VectorStore`] over a
//! local libSQL/SQLite file. Each namespace is a separate `vec0` virtual table
//! (`vec0_{ns}`), so namespaces are isolated by structure.
//!
//! HOW: each `vec0` table has a text primary key (the entry id), the
//! embedding column with the store's distance metric, and an auxiliary
//! `metadata` column (JSON text). The methods await libSQL's async API
//! directly, the same as `LibsqlStore` in `foundation_db`.
//!
//! Sync boundary: [`SqliteVecVectorStore::open`] writes the bundled `.so` to a
//! temp file and loads it. `std::fs::write` and libSQL's `load_extension`
//! are sync calls. They run once, when the store opens.
//!
//! Limits of `vec0`:
//! - It has cosine and L2 distance, but no dot product. A store configured
//!   with [`DistanceMetric::Dot`] fails to open with
//!   [`VectorStoreError::Unsupported`].
//! - A namespace becomes part of a table name, so it may hold only ASCII
//!   letters, digits and `_` (at most [`MAX_NAMESPACE_LEN`] bytes).

use std::path::Path;

use libsql::{Builder, Connection};

use crate::metric::DistanceMetric;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

/// Prebuilt sqlite-vec loadable extension (linux-x86_64, v0.1.9).
/// Loaded at init time into the SQLite connection via `load_extension`.
const VEC0_SO: &[u8] = include_bytes!("../vendor/vec0.so");

/// The longest namespace name, in bytes. It keeps the table name well inside
/// SQLite's identifier rules; this is a format rule, not a tuning limit.
pub const MAX_NAMESPACE_LEN: usize = 64;

/// Serialize a vector into raw f32 LE bytes for vec0 `float[{dim}]`.
/// The table schema already knows the dimension, so no length prefix.
fn vec_blob(v: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for f in v {
        b.extend_from_slice(&f.to_le_bytes());
    }
    b
}

/// Decode a vec0 `float[{dim}]` blob (f32 LE) back to floats.
fn blob_vec(b: &[u8]) -> Result<Vec<f32>, VectorStoreError> {
    if !b.len().is_multiple_of(4) {
        return Err(VectorStoreError::Backend(format!(
            "embedding blob length {} is not a multiple of 4",
            b.len()
        )));
    }
    Ok(b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn be<E: std::fmt::Display>(ctx: &'static str) -> impl Fn(E) -> VectorStoreError {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

/// The `vec0` table of a namespace, after a check that the name is safe to
/// put in SQL.
fn table_name(namespace: &str) -> Result<String, VectorStoreError> {
    let valid = !namespace.is_empty()
        && namespace.len() <= MAX_NAMESPACE_LEN
        && namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err(VectorStoreError::Unsupported(format!(
            "namespace {namespace:?}: sqlite-vec namespaces use only ASCII letters, digits \
             and '_', 1 to {MAX_NAMESPACE_LEN} bytes"
        )));
    }
    Ok(format!("vec0_{namespace}"))
}

pub struct SqliteVecVectorStore {
    conn: Connection,
    config: VectorStoreConfig,
    // Keep the temp dir alive so the .so isn't deleted while the extension is loaded.
    #[allow(dead_code)]
    temp_dir: tempfile::TempDir,
}

impl SqliteVecVectorStore {
    /// Open (or create) a persistent vector store at `db_path`.
    ///
    /// The bundled sqlite-vec `.so` is written to a temp directory and loaded
    /// into the SQLite connection. Each namespace gets its own `vec0_{ns}`
    /// virtual table on its first insert.
    ///
    /// # Errors
    /// [`VectorStoreError::Unsupported`] for [`DistanceMetric::Dot`] (`vec0`
    /// has no dot product). [`VectorStoreError::Backend`] if the extension
    /// cannot be loaded or the database cannot be opened.
    pub async fn open<P: AsRef<Path>>(
        config: VectorStoreConfig,
        db_path: P,
    ) -> Result<Self, VectorStoreError> {
        if config.metric == DistanceMetric::Dot {
            return Err(VectorStoreError::Unsupported(
                "sqlite-vec has no dot-product distance; use Cosine or L2".into(),
            ));
        }
        let dir = tempfile::TempDir::new().map_err(be("create temp dir"))?;
        let so_path = dir.path().join("vec0.so");
        std::fs::write(&so_path, VEC0_SO).map_err(be("write extension .so"))?;

        let path_str = db_path
            .as_ref()
            .to_str()
            .ok_or_else(|| VectorStoreError::Backend("invalid db path".into()))?;
        let db = Builder::new_local(path_str)
            .build()
            .await
            .map_err(be("open db"))?;
        let conn = db.connect().map_err(be("connect"))?;

        conn.load_extension_enable()
            .map_err(be("enable load_extension"))?;
        conn.load_extension(&so_path, None)
            .map_err(be("load_extension"))?;
        conn.load_extension_disable()
            .map_err(be("disable load_extension"))?;

        Ok(Self {
            conn,
            config,
            temp_dir: dir,
        })
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

    /// The `vec0` distance metric option for the store's metric.
    fn metric_option(&self) -> &'static str {
        match self.config.metric {
            DistanceMetric::L2 => "l2",
            // `open` refuses Dot, so only Cosine reaches here.
            DistanceMetric::Cosine | DistanceMetric::Dot => "cosine",
        }
    }

    /// Map a `vec0` distance to the crate's score (higher is closer).
    fn score(&self, distance: f64) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        match self.config.metric {
            // vec0 `l2` is the plain Euclidean distance; the crate's L2 score is
            // the negative squared distance.
            DistanceMetric::L2 => -(distance * distance) as f32,
            DistanceMetric::Cosine | DistanceMetric::Dot => (1.0_f64 - distance) as f32,
        }
    }

    async fn table_exists(&self, table: &str) -> Result<bool, VectorStoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
                libsql::params![table],
            )
            .await
            .map_err(be("look up table"))?;
        Ok(rows.next().await.map_err(be("look up table"))?.is_some())
    }

    async fn ensure_table(&self, table: &str) -> Result<(), VectorStoreError> {
        let sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING vec0(\
                external_id text primary key, \
                embedding float[{dim}] distance_metric={metric}, \
                +metadata text)",
            dim = self.config.dimension,
            metric = self.metric_option(),
        );
        self.conn
            .execute(&sql, ())
            .await
            .map(|_| ())
            .map_err(be("create vec0"))
    }
}

#[async_trait::async_trait]
impl VectorStore for SqliteVecVectorStore {
    async fn insert(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let table = table_name(namespace)?;
        self.ensure_table(&table).await?;
        let meta = serde_json::to_string(&entry.metadata).map_err(be("encode metadata"))?;
        let blob = vec_blob(&entry.vector.data);
        // vec0 has no upsert: replace an existing id inside one transaction.
        let tx = self.conn.transaction().await.map_err(be("begin"))?;
        tx.execute(
            &format!("DELETE FROM {table} WHERE external_id = ?"),
            libsql::params![entry.id.clone()],
        )
        .await
        .map_err(be("replace"))?;
        tx.execute(
            &format!("INSERT INTO {table}(external_id, embedding, metadata) VALUES (?, ?, ?)"),
            libsql::params![entry.id, blob, meta],
        )
        .await
        .map_err(be("insert"))?;
        tx.commit().await.map_err(be("commit"))
    }

    async fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let table = table_name(namespace)?;
        if !self.table_exists(&table).await? {
            return Err(VectorStoreError::NotFound { id: id.to_string() });
        }
        let affected = self
            .conn
            .execute(
                &format!("DELETE FROM {table} WHERE external_id = ?"),
                libsql::params![id],
            )
            .await
            .map_err(be("delete"))?;
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
        self.validate(query.len(), false)?;
        let table = table_name(namespace)?;
        if k == 0 || !self.table_exists(&table).await? {
            return Ok(Vec::new());
        }
        let k = i64::try_from(k).map_err(be("k out of range"))?;
        let mut rows = self
            .conn
            .query(
                &format!(
                    "SELECT external_id, distance FROM {table} \
                     WHERE embedding MATCH ? AND k = ? ORDER BY distance"
                ),
                libsql::params![vec_blob(query), k],
            )
            .await
            .map_err(be("query vec0"))?;
        let mut matches = Vec::new();
        while let Some(row) = rows.next().await.map_err(be("fetch row"))? {
            let id: String = row.get(0).map_err(be("read id"))?;
            let dist: f64 = row.get(1).map_err(be("read distance"))?;
            matches.push(VectorMatch {
                id,
                score: self.score(dist),
            });
        }
        Ok(matches)
    }

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
        let table = table_name(namespace)?;
        if !self.table_exists(&table).await? {
            return Ok(None);
        }
        let mut rows = self
            .conn
            .query(
                &format!("SELECT embedding, metadata FROM {table} WHERE external_id = ?"),
                libsql::params![id],
            )
            .await
            .map_err(be("get"))?;
        let Some(row) = rows.next().await.map_err(be("fetch row"))? else {
            return Ok(None);
        };
        let blob: Vec<u8> = row.get(0).map_err(be("read embedding"))?;
        let meta: Option<String> = row.get(1).map_err(be("read metadata"))?;
        let metadata: VectorMetadata = match meta {
            Some(m) => serde_json::from_str(&m).map_err(be("decode metadata"))?,
            None => VectorMetadata::default(),
        };
        Ok(Some(VectorEntry {
            id: id.to_string(),
            vector: Vector::new(blob_vec(&blob)?),
            metadata,
        }))
    }

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError> {
        let table = table_name(namespace)?;
        if !self.table_exists(&table).await? {
            return Ok(0);
        }
        let mut rows = self
            .conn
            .query(&format!("SELECT COUNT(*) FROM {table}"), ())
            .await
            .map_err(be("count"))?;
        let Some(row) = rows.next().await.map_err(be("count"))? else {
            return Ok(0);
        };
        let n: i64 = row.get(0).map_err(be("read count"))?;
        usize::try_from(n).map_err(be("count out of range"))
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
