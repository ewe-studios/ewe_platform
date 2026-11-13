-- Migration: 028_create_tos_acceptances
-- Records which Terms of Service version each user accepted. A user accepts a
-- version once; a second acceptance of the same version is ignored.

CREATE TABLE IF NOT EXISTS tos_acceptances (
    user_id TEXT NOT NULL,
    tos_version TEXT NOT NULL,
    accepted_at INTEGER NOT NULL,                 -- Unix time in milliseconds
    ip_address TEXT NOT NULL DEFAULT '',          -- empty when unknown
    PRIMARY KEY (user_id, tos_version)
);

CREATE INDEX IF NOT EXISTS idx_tos_acceptances_user ON tos_acceptances(user_id, accepted_at);
