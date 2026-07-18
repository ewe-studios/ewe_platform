-- foundation_keychain initial schema (spec-57, F008 Stage 1).
-- Grown incrementally as vault entities are implemented.

-- Folders — per-user grouping of ciphers.
CREATE TABLE IF NOT EXISTS folders (
    uuid       TEXT PRIMARY KEY,
    user_uuid  TEXT NOT NULL,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_folders_user ON folders (user_uuid);
