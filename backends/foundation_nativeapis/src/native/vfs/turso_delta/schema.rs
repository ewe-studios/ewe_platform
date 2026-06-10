//! SQL schema definitions and migration logic for TursoDelta.

use libsql::Connection;

/// Schema version for migration tracking.
pub const SCHEMA_VERSION: i64 = 1;

/// Create the dentry table with SCRU128 version support (schema v2).
pub const CREATE_DENTRY: &str = r#"
CREATE TABLE IF NOT EXISTS turso_dentry (
    ino           INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    parent_ino    INTEGER NOT NULL,
    file_type     TEXT NOT NULL CHECK (file_type IN ('file', 'dir', 'symlink')),
    size          INTEGER NOT NULL DEFAULT 0,
    permissions   INTEGER NOT NULL DEFAULT 493,
    owner_uid     INTEGER NOT NULL DEFAULT 0,
    owner_gid     INTEGER NOT NULL DEFAULT 0,
    checksum      BLOB,
    version_id    BLOB(16) NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    symlink_target TEXT,
    chunk_size    INTEGER NOT NULL DEFAULT 65536,
    UNIQUE(parent_ino, name)
)"#;

/// Create the chunks table.
pub const CREATE_CHUNKS: &str = r#"
CREATE TABLE IF NOT EXISTS turso_chunks (
    ino         INTEGER NOT NULL,
    chunk_idx   INTEGER NOT NULL,
    data        BLOB NOT NULL,
    PRIMARY KEY (ino, chunk_idx),
    FOREIGN KEY (ino) REFERENCES turso_dentry(ino) ON DELETE CASCADE
)"#;

/// Create the whiteouts table with SCRU128 version.
pub const CREATE_WHITEOUTS: &str = r#"
CREATE TABLE IF NOT EXISTS turso_whiteouts (
    path        TEXT PRIMARY KEY,
    version_id  BLOB(16) NOT NULL
)"#;

/// Create the hierarchical whiteout prefix index table.
pub const CREATE_WHITEOUT_PREFIXES: &str = r#"
CREATE TABLE IF NOT EXISTS turso_whiteout_prefixes (
    prefix      TEXT NOT NULL,
    path        TEXT NOT NULL,
    version_id  BLOB(16) NOT NULL,
    PRIMARY KEY (prefix, path)
)"#;

/// Create the schema version tracking table.
pub const CREATE_META: &str = r#"
CREATE TABLE IF NOT EXISTS turso_vfs_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
)"#;

/// Create indexes for common query patterns.
pub const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_dentry_parent ON turso_dentry(parent_ino)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_dentry_parent_name ON turso_dentry(parent_ino, name)",
    "CREATE INDEX IF NOT EXISTS idx_chunks_ino ON turso_chunks(ino)",
    "CREATE INDEX IF NOT EXISTS idx_whiteouts_path ON turso_whiteouts(path)",
    "CREATE INDEX IF NOT EXISTS idx_whiteout_prefix ON turso_whiteout_prefixes(prefix)",
];

/// Insert the root directory bootstrap entry.
/// ino = 1, parent_ino = 1 (self-referencing), name = '' (empty sentinel).
pub const INSERT_ROOT_DENTRY: &str = r#"
INSERT OR IGNORE INTO turso_dentry
    (ino, name, parent_ino, file_type, size, permissions, owner_uid, owner_gid,
     version_id, created_at, updated_at, chunk_size)
VALUES
    (1, '', 1, 'dir', 0, 493, 0, 0,
     X'00000000000000000000000000000000',
     CAST(strftime('%s', 'now') * 1000 AS INTEGER),
     CAST(strftime('%s', 'now') * 1000 AS INTEGER),
     65536)"#;

/// Insert the schema version.
pub const INSERT_SCHEMA_VERSION: &str = r#"
INSERT OR REPLACE INTO turso_vfs_meta (key, value) VALUES ('schema_version', ?)"#;

/// Query the current schema version.
pub const QUERY_SCHEMA_VERSION: &str =
    "SELECT value FROM turso_vfs_meta WHERE key = 'schema_version'";

/// Enable WAL mode and set pragmas.
pub const PRAGMAS: &[&str] = &[
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA foreign_keys = ON",
    "PRAGMA busy_timeout = 5000",
];

/// WAL checkpoint pragma.
pub const WAL_CHECKPOINT: &str = "PRAGMA wal_checkpoint(TRUNCATE)";

/// Run all schema migrations. Idempotent (CREATE IF NOT EXISTS).
pub fn run_migrations(conn: &Connection) -> crate::shared::vfs::error::VfsResult<()> {
    use crate::shared::vfs::error::{VfsError, VfsResult};

    // Ensure meta table exists first
    conn.execute(CREATE_META, ())
        .map_err(|e| VfsError::Io { source: e.into() })?;

    // Check current schema version
    let current_version: Option<i64> = conn
        .query(QUERY_SCHEMA_VERSION, ())
        .map_err(|e| VfsError::Io { source: e.into() })?
        .next()
        .map_err(|e| VfsError::Io { source: e.into() })?
        .and_then(|row| row.get_value(0).ok())
        .and_then(|v| match v {
            libsql::Value::Integer(i) => Some(i),
            libsql::Value::Text(s) => s.parse::<i64>().ok(),
            _ => None,
        });

    let target = SCHEMA_VERSION;

    if current_version.is_none() || current_version.unwrap() < target {
        // Run all table creations (IF NOT EXISTS makes them safe)
        conn.execute(CREATE_DENTRY, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;
        conn.execute(CREATE_CHUNKS, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;
        conn.execute(CREATE_WHITEOUTS, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;
        conn.execute(CREATE_WHITEOUT_PREFIXES, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        // Create indexes
        for idx_sql in CREATE_INDEXES {
            conn.execute(idx_sql, ())
                .map_err(|e| VfsError::Io { source: e.into() })?;
        }

        // Insert root dentry (safe due to OR IGNORE on ino=1)
        conn.execute(INSERT_ROOT_DENTRY, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        // Update schema version
        conn.execute(INSERT_SCHEMA_VERSION, (target,))
            .map_err(|e| VfsError::Io { source: e.into() })?;
    }

    Ok(())
}

/// Enable WAL mode and set pragmas.
pub fn enable_wal(conn: &Connection) -> crate::shared::vfs::error::VfsResult<()> {
    use crate::shared::vfs::error::VfsError;

    for pragma in PRAGMAS {
        conn.execute(pragma, ())
            .map_err(|e| VfsError::Io { source: e.into() })?;
    }
    Ok(())
}

/// Run WAL checkpoint.
pub fn wal_checkpoint(conn: &Connection) -> crate::shared::vfs::error::VfsResult<()> {
    use crate::shared::vfs::error::VfsError;

    conn.execute(WAL_CHECKPOINT, ())
        .map_err(|e| VfsError::Io { source: e.into() })?;
    Ok(())
}
