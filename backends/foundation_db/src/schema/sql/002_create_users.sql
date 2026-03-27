-- Migration: 002_create_users
-- Create users table for core identity

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    username TEXT UNIQUE,
    password_hash TEXT,
    email_verified INTEGER DEFAULT 0,
    email_verified_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    metadata TEXT,

    -- Account lockout (rate limiting per user)
    failed_login_attempts INTEGER DEFAULT 0,
    locked_until INTEGER,

    -- Soft delete support
    deleted_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
