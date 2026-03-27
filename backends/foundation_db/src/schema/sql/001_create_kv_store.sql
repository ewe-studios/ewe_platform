-- Migration: 001_create_kv_store
-- Create key-value store table

CREATE TABLE IF NOT EXISTS kv_store (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
    updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_kv_store_key ON kv_store(key);
