-- Migration: 005_create_verification_tokens
-- Create verification tokens table (unified: email OTP, magic links, password reset)

CREATE TABLE IF NOT EXISTS verification_tokens (
    id TEXT PRIMARY KEY,
    identifier TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    type TEXT NOT NULL,  -- 'email_otp', 'magic_link', 'password_reset'
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    consumed_at INTEGER,
    metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_verification_tokens_token ON verification_tokens(token);
CREATE INDEX IF NOT EXISTS idx_verification_tokens_identifier ON verification_tokens(identifier);
CREATE INDEX IF NOT EXISTS idx_verification_tokens_type ON verification_tokens(type);
