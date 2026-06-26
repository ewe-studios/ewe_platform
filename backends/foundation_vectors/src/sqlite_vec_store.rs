//! SQLite vector store via **sqlite-vec** loaded as a dynamic extension (F29).
//!
//! WHY: `sqlite-vec` provides native `vec0` virtual tables with KNN queries on
//! SQLite-compatible engines. The extension is loaded at runtime from a bundled
//! `.so` (written to a temp dir), so no system install is needed.
//!
//! WHAT: [`SqliteVecVectorStore`] implements [`VectorStore`] over a local
//! libSQL/SQLite file. Each namespace is a separate `vec0` virtual table
//! (`vec0_{ns}`), giving structural namespace isolation and native ANN queries.
//!
//! HOW: on construction the extension `.so` is written to a temp dir and loaded
//! via `libsql::Connection::load_extension_enable()` + `load_extension()`. All
//! `VectorStore` methods are synchronous; the libSQL async futures are bridged
//! with `pollster::block_on` + `SendWrapper`.

use std::path::Path;

use foundation_compact::SendWrapper;
use libsql::{Builder, Connection};

use crate::store::{
    VectorEntry, VectorMatch, VectorStore, VectorStoreConfig, VectorStoreError,
};

/// Prebuilt sqlite-vec loadable extension (linux-x86_64, v0.1.9).
/// Loaded at init time into the SQLite connection via `load_extension`.
const VEC0_SO: &[u8] = include_bytes!("../vendor/vec0.so");

/// Serialize a vector into raw f32 LE bytes for vec0 `float[{dim}]`.
/// The table schema already knows the dimension, so no length prefix.
fn vec_blob(v: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for f in v {
        b.extend_from_slice(&f.to_le_bytes());
    }
    b
}

/// vec0 uses cosine distance; convert to similarity score.
fn dist_to_score(d: f64) -> f32 {
    (1.0_f64 - d) as f32
}

fn table_name(namespace: &str) -> String {
    format!("vec0_{namespace}")
}

fn be<E: std::fmt::Display>(ctx: &'static str) -> impl Fn(E) -> VectorStoreError {
    move |e| VectorStoreError::Backend(format!("{ctx}: {e}"))
}

/// Sync bridge: block_on a `!Send` libSQL future via `SendWrapper`.
fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    pollster::block_on(SendWrapper::new(f))
}

/// vec0 requires integer rowid + optional external_id for string ids.
/// Schema: `vec0(embedding float[{dim}], external_id text)`
fn ensure_table(conn: &Connection, table: &str, dim: usize) -> Result<(), VectorStoreError> {
    block(conn.execute(
        &format!("CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING vec0(embedding float[{dim}], external_id text[km=km])"),
        (),
    ))
    .map(|_| ())
    .map_err(be("create vec0"))
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
    /// On first open, the bundled sqlite-vec `.so` is written to a temp
    /// directory and loaded into the SQLite connection via
    /// `load_extension_enable`/`load_extension`.  Each namespace gets its own
    /// `vec0_{ns}` virtual table.
    ///
    /// # Errors
    /// Returns [`VectorStoreError::Backend`] if the extension cannot be loaded
    /// or the database cannot be opened.
    pub fn open<P: AsRef<Path>>(config: VectorStoreConfig, db_path: P) -> Result<Self, VectorStoreError> {
        let dir = tempfile::TempDir::new().map_err(be("create temp dir"))?;
        let so_path = dir.path().join("vec0.so");
        std::fs::write(&so_path, VEC0_SO).map_err(be("write extension .so"))?;

        let path_str = db_path.as_ref().to_str().ok_or_else(|| {
            VectorStoreError::Backend("invalid db path".into())
        })?;
        let db = block(async move {
            Builder::new_local(path_str).build().await
        })
        .map_err(be("open db"))?;
        let conn = db.connect().map_err(be("connect"))?;

        conn.load_extension_enable().map_err(be("enable load_extension"))?;
        conn.load_extension(&so_path, None).map_err(be("load_extension"))?;
        conn.load_extension_disable().ok();

        Ok(Self { conn, config, temp_dir: dir })
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

impl VectorStore for SqliteVecVectorStore {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let table = table_name(namespace);
        ensure_table(&self.conn, &table, self.config.dimension)?;
        let blob = vec_blob(&entry.vector.data);
        block(
            self.conn.execute(
                &format!("INSERT INTO {table}(embedding, external_id) VALUES (?, ?)"),
                libsql::params![blob, entry.id],
            ),
        )
        .map(|_| ())
        .map_err(be("insert"))
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let table = table_name(namespace);
        let affected = block(
            self.conn.execute(
                &format!("DELETE FROM {table} WHERE external_id = ?"),
                libsql::params![id],
            ),
        )
        .map_err(be("delete"))?;
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
        let table = table_name(namespace);
        ensure_table(&self.conn, &table, self.config.dimension)?;
        let q = vec_blob(query);
        let mut stmt = block(
            self.conn.prepare(&format!(
                "SELECT external_id, distance FROM {table} WHERE embedding MATCH ? ORDER BY distance LIMIT ?"
            )),
        )
        .map_err(be("prepare vec0"))?;
        let mut matches = Vec::new();
        let mut rows = block(stmt.query(libsql::params![q, k as i64]))
            .map_err(be("query vec0"))?;
        while let Some(row) = block(rows.next()).map_err(be("fetch row"))? {
            let id: String = row.get(0).map_err(be("read id"))?;
            let dist: f64 = row.get(1).map_err(be("read distance"))?;
            matches.push(VectorMatch { id, score: dist_to_score(dist) });
        }
        Ok(matches)
    }

    fn get(&self, namespace: &str, _id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        // vec0 doesn't expose the raw vector via SELECT; this is primarily
        // a search store.  A metadata sidecar table could be added if needed.
        let _ = namespace;
        Ok(None)
    }

    fn len(&self, namespace: &str) -> usize {
        let table = table_name(namespace);
        match block(
            self.conn.query(
                &format!("SELECT COUNT(*) FROM {table}"),
                (),
            ),
        ) {
            Ok(mut rows) => {
                if let Ok(Some(row)) = block(rows.next()) {
                    if let Ok(n) = row.get::<i64>(0) {
                        return usize::try_from(n).unwrap_or(0);
                    }
                }
            }
            Err(_) => {}
        }
        0
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
