# Decision 10: SSH Key Provisioning API

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

The secret is a one-time response — it's hashed (Argon2id) for storage. If lost, the app must be recreated.

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

### Storage

SSH keys are stored encrypted at rest. The encryption key is derived from a master key (env var) using the `age` crate (decision 08 — deferred but revisited here). Alternatively, for Cloudflare Workers, keys can be stored in KV encrypted with a key derived from a secret.

### Key Types

- **Ed25519** (default) — fast, small, modern
- **RSA 4096** — legacy compatibility
- Generated server-side using `ssh_key` crate (pure Rust, WASM-compatible)

### Security Model

- App secret is the sole credential — treat it like a password
- Keys are encrypted at rest (age or AES-GCM)
- Keys are only returned on creation or explicit retrieval — never in list responses
- Audit log records all key creation/retrieval/deletion events
- Optional TTL on keys (auto-delete after expiry)

## Consequences

- Adds a new API surface beyond the Bitwarden-compatible vault
- Requires the `ssh_key` crate dependency (pure Rust, WASM-compatible)
- At-rest encryption needed for stored private keys (decision 08 becomes relevant)
- App registration is a one-time secret — lost secrets require app recreation
