# Decision 10: SSH Key Provisioning API

**Status:** Resolved (2026-07-18)

## Problem

Applications need SSH keys for authentication (git, SSH access, etc.) but managing key generation, storage, and rotation is a cross-cutting concern. Running this as part of the keychain service means:

- Apps can request SSH keys via API — no local persistence needed
- Keys can be retrieved on-demand using an app secret
- Key rotation is centralized
- Works on Cloudflare Workers or native

## Design

### App Registration

An app registers once and receives a secret. That secret is used for all subsequent credential requests.

```
POST /api/apps/register
Body: { "name": "my-service", "description": "..." }
→ 200 { "app_id": "...", "secret": "...", "created_at": "..." }
```

The secret is a one-time response — it's hashed (Argon2id via `foundation_auth::password_hash`) for storage. If lost, the app must be recreated.

### Credential Provisioning

Authenticated by app secret (Bearer token or `X-App-Secret` header):

```
POST /api/credentials/ssh-keys
Body: { "name": "deploy-key", "key_type": "ed25519", "comment": "prod deploy" }
→ 200 { "id": "...", "name": "deploy-key", "public_key": "ssh-ed25519 AAAA...", "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----...", "created_at": "..." }

GET /api/credentials/ssh-keys
→ 200 { "data": [{ "id": "...", "name": "...", "public_key": "...", "created_at": "..." }, ...] }

GET /api/credentials/ssh-keys/:id
→ 200 { "id": "...", "public_key": "...", "private_key": "...", "created_at": "..." }

DELETE /api/credentials/ssh-keys/:id
→ 204
```

### At-Rest Encryption — age (both native and WASM)

Private keys are encrypted before storage using the `age` crate in **passphrase (scrypt) mode**. The passphrase is `KEYCHAIN_MASTER_KEY` (an env var / secret). age's `scrypt::Recipient` derives the symmetric encryption key from the passphrase — no separate keypair to manage.

```rust
use age::scrypt;

// Encrypt (on key creation):
let recipient = scrypt::Recipient::new(master_secret.into());
let encrypted = age::encrypt(&recipient, private_key_bytes)?;
store.private_key_encrypted = base64_encode(&encrypted);

// Decrypt (on retrieval):
let identity = scrypt::Identity::new(master_secret.into());
let private_key_bytes = age::decrypt(&identity, &base64_decode(&stored.private_key_encrypted)?)?;
```

age compiles to both `x86_64` (native) and `wasm32-unknown-unknown` (Cloudflare Workers). On WASM, the `web-sys` feature calibrates the scrypt work factor. Same code, both backends — no backend-specific encryption.

### Key Types

- **Ed25519** (default) — fast, small, modern
- **RSA 4096** — legacy compatibility
- Generated server-side using `ssh_key` crate (pure Rust, WASM-compatible)

### Security Model

- App secret is the sole credential — treat it like a password
- Private keys encrypted at rest with age (scrypt passphrase from `KEYCHAIN_MASTER_KEY`)
- Keys are only returned on creation or explicit retrieval — never in list responses
- Audit log records all key creation/retrieval/deletion events
- Optional TTL on keys (auto-delete after expiry via cron)

## Consequences

- Adds a new API surface beyond the Bitwarden-compatible vault
- Dependencies: `ssh_key` (key generation) + `age` (at-rest encryption) — both WASM-compatible
- One `KEYCHAIN_MASTER_KEY` secret drives at-rest encryption — no keypair management
- App registration is a one-time secret — lost secrets require app recreation
- Same encryption code on native and Workers — age passphrase mode works on both
