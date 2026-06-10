//! SQL schema definitions for LibsqlDelta.

/// All schema creation SQL in one batch (idempotent via IF NOT EXISTS).
pub const CREATE_ALL: &str = r#"
CREATE TABLE IF NOT EXISTS vfs_dentry (
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
);

CREATE TABLE IF NOT EXISTS vfs_chunks (
    ino         INTEGER NOT NULL,
    chunk_idx   INTEGER NOT NULL,
    data        BLOB NOT NULL,
    PRIMARY KEY (ino, chunk_idx),
    FOREIGN KEY (ino) REFERENCES vfs_dentry(ino) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS vfs_whiteouts (
    path        TEXT PRIMARY KEY,
    version_id  BLOB(16) NOT NULL
);

CREATE TABLE IF NOT EXISTS vfs_whiteout_prefixes (
    prefix      TEXT NOT NULL,
    path        TEXT NOT NULL,
    version_id  BLOB(16) NOT NULL,
    PRIMARY KEY (prefix, path)
);

CREATE INDEX IF NOT EXISTS idx_dentry_parent ON vfs_dentry(parent_ino);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dentry_parent_name ON vfs_dentry(parent_ino, name);
CREATE INDEX IF NOT EXISTS idx_chunks_ino ON vfs_chunks(ino);
CREATE INDEX IF NOT EXISTS idx_whiteouts_path ON vfs_whiteouts(path);
CREATE INDEX IF NOT EXISTS idx_whiteout_prefix ON vfs_whiteout_prefixes(prefix);

INSERT OR IGNORE INTO vfs_dentry
    (ino, name, parent_ino, file_type, size, permissions, owner_uid, owner_gid,
     version_id, created_at, updated_at, chunk_size)
VALUES
    (1, '', 1, 'dir', 0, 493, 0, 0,
     X'00000000000000000000000000000000',
     CAST(strftime('%s', 'now') * 1000 AS INTEGER),
     CAST(strftime('%s', 'now') * 1000 AS INTEGER),
     65536);
"#;

/// WAL mode pragmas.
pub const PRAGMAS: &[&str] = &[
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA foreign_keys = ON",
    "PRAGMA busy_timeout = 5000",
];
