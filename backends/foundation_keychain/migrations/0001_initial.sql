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

-- Users — account + server-side password verifier + client KDF params.
CREATE TABLE IF NOT EXISTS vault_accounts (
    uuid                   TEXT PRIMARY KEY,
    email                  TEXT NOT NULL UNIQUE,
    name                   TEXT,
    password_hash          TEXT NOT NULL,   -- base64(server PBKDF2 of client master_password_hash)
    salt                   TEXT NOT NULL,   -- base64(per-user random salt)
    password_iterations    INTEGER NOT NULL,
    security_stamp         TEXT NOT NULL,   -- rotates on password change; invalidates tokens
    akey                   TEXT,            -- protected symmetric key ("Key")
    client_kdf_type        INTEGER NOT NULL,
    client_kdf_iter        INTEGER NOT NULL,
    client_kdf_memory      INTEGER,
    client_kdf_parallelism INTEGER,
    master_password_hint   TEXT,
    email_verified         INTEGER NOT NULL DEFAULT 0,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_vault_accounts_email ON vault_accounts (email);

-- Devices — one per client login; holds the current refresh token for rotation.
CREATE TABLE IF NOT EXISTS vault_devices (
    uuid           TEXT PRIMARY KEY,
    user_uuid      TEXT NOT NULL,
    identifier     TEXT NOT NULL,   -- client-provided device identifier
    name           TEXT,
    atype          INTEGER NOT NULL DEFAULT 0,
    refresh_token  TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_vault_devices_user_ident
    ON vault_devices (user_uuid, identifier);

-- Ciphers — the vault items (login/card/identity/note). Client-encrypted content
-- is stored opaquely as `data` (JSON); metadata columns support listing + sync.
CREATE TABLE IF NOT EXISTS ciphers (
    uuid              TEXT PRIMARY KEY,
    user_uuid         TEXT NOT NULL,
    organization_uuid TEXT,
    atype             INTEGER NOT NULL,
    folder_uuid       TEXT,
    favorite          INTEGER NOT NULL DEFAULT 0,
    name              TEXT NOT NULL,
    data              TEXT NOT NULL,
    deleted_at        TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ciphers_user ON ciphers (user_uuid);

-- Sends — time-limited, access-counted secure sharing.
CREATE TABLE IF NOT EXISTS sends (
    uuid             TEXT PRIMARY KEY,
    user_uuid        TEXT NOT NULL,
    atype            INTEGER NOT NULL,
    name             TEXT NOT NULL,
    notes            TEXT,
    data             TEXT,
    akey             TEXT,
    password         TEXT,
    max_access_count INTEGER,
    access_count     INTEGER NOT NULL DEFAULT 0,
    disabled         INTEGER NOT NULL DEFAULT 0,
    hide_email       INTEGER NOT NULL DEFAULT 0,
    expiration_date  TEXT,
    deletion_date    TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sends_user ON sends (user_uuid);

-- Two-factor (TOTP) — one row per user; `secret` is base32, enabled gates login.
CREATE TABLE IF NOT EXISTS vault_two_factor (
    user_uuid     TEXT PRIMARY KEY,
    secret        TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 0,
    recovery_code TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- Events — lightweight audit log (client posts; owner lists).
CREATE TABLE IF NOT EXISTS vault_events (
    uuid          TEXT PRIMARY KEY,
    user_uuid     TEXT NOT NULL,
    atype         INTEGER NOT NULL,
    cipher_uuid   TEXT,
    event_date    TEXT NOT NULL,
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vault_events_user ON vault_events (user_uuid);

-- Organizations + memberships + collections (core org model).
CREATE TABLE IF NOT EXISTS vault_organizations (
    uuid          TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    billing_email TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1,
    akey          TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- status: 0 = invited, 1 = accepted, 2 = confirmed
CREATE TABLE IF NOT EXISTS vault_org_memberships (
    uuid       TEXT PRIMARY KEY,
    org_uuid   TEXT NOT NULL,
    user_uuid  TEXT,
    email      TEXT NOT NULL,
    atype      INTEGER NOT NULL,
    status     INTEGER NOT NULL DEFAULT 0,
    akey       TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memberships_org ON vault_org_memberships (org_uuid);
CREATE INDEX IF NOT EXISTS idx_memberships_user ON vault_org_memberships (user_uuid);

CREATE TABLE IF NOT EXISTS vault_collections (
    uuid       TEXT PRIMARY KEY,
    org_uuid   TEXT NOT NULL,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_collections_org ON vault_collections (org_uuid);

-- App registry (feature 009) — apps that provision credentials; secret is Argon2id-hashed.
CREATE TABLE IF NOT EXISTS vault_apps (
    uuid         TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    secret_hash  TEXT NOT NULL,
    secret_salt  TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

-- SSH keys (feature 009) — generated per app; private key is age-encrypted at rest.
CREATE TABLE IF NOT EXISTS vault_ssh_keys (
    uuid                  TEXT PRIMARY KEY,
    app_uuid              TEXT NOT NULL,
    name                  TEXT NOT NULL,
    key_type              TEXT NOT NULL,
    public_key            TEXT NOT NULL,
    private_key_encrypted TEXT NOT NULL,
    comment               TEXT,
    expires_at            TEXT,
    created_at            TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ssh_keys_app ON vault_ssh_keys (app_uuid);
