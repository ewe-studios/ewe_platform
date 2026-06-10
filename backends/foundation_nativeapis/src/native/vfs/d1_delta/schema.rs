pub const CREATE_DENTRY: &str = "\
CREATE TABLE IF NOT EXISTS d1_dentry (
    ino         INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_ino  INTEGER NOT NULL,
    file_type   TEXT NOT NULL,
    size        INTEGER,
    permissions INTEGER NOT NULL DEFAULT 493,
    owner_uid   INTEGER NOT NULL DEFAULT 0,
    owner_gid   INTEGER NOT NULL DEFAULT 0,
    checksum    TEXT,
    version     INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    symlink_target TEXT,
    UNIQUE(parent_ino, name)
);";

pub const CREATE_CHUNKS: &str = "\
CREATE TABLE IF NOT EXISTS d1_chunks (
    ino         INTEGER NOT NULL,
    chunk_idx   INTEGER NOT NULL,
    data        BLOB NOT NULL,
    PRIMARY KEY (ino, chunk_idx)
);";

pub const CREATE_WHITEOUTS: &str = "\
CREATE TABLE IF NOT EXISTS d1_whiteouts (
    path    TEXT PRIMARY KEY,
    version INTEGER NOT NULL
);";

pub const CREATE_ALL: &str = "\
CREATE TABLE IF NOT EXISTS d1_dentry (
    ino         INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_ino  INTEGER NOT NULL,
    file_type   TEXT NOT NULL,
    size        INTEGER,
    permissions INTEGER NOT NULL DEFAULT 493,
    owner_uid   INTEGER NOT NULL DEFAULT 0,
    owner_gid   INTEGER NOT NULL DEFAULT 0,
    checksum    TEXT,
    version     INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    symlink_target TEXT,
    UNIQUE(parent_ino, name)
);
CREATE TABLE IF NOT EXISTS d1_chunks (
    ino         INTEGER NOT NULL,
    chunk_idx   INTEGER NOT NULL,
    data        BLOB NOT NULL,
    PRIMARY KEY (ino, chunk_idx)
);
CREATE TABLE IF NOT EXISTS d1_whiteouts (
    path    TEXT PRIMARY KEY,
    version INTEGER NOT NULL
);
INSERT OR IGNORE INTO d1_dentry (ino, name, parent_ino, file_type, permissions, version, created_at, updated_at)
VALUES (1, '', 0, 'dir', 493, 0, 0, 0);
";
