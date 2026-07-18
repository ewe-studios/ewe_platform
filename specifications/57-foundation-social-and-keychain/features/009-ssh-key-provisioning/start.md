---
feature: "009-ssh-key-provisioning"
spec: "57-foundation-social-and-keychain"
depends: "008-keychain-vault-and-backends"
status: "pending"
---

# Feature 009: SSH Key Provisioning + App Registry

## Current state
Not started.

## Remaining work
- App registry: models, register/list/delete endpoints, Argon2id secret hashing
- SSH key management: models, CRUD endpoints for Ed25519 and RSA 4096 keys
- Key generation via `ssh_key` crate (pure Rust, WASM-compatible)
- At-rest encryption via `age` crate (scrypt passphrase mode, `KEYCHAIN_MASTER_KEY`)
- App secret auth middleware (`X-App-Secret` header / Bearer token)
- DB schema: `apps` and `ssh_keys` tables
- Cron job: expired key cleanup (hourly)
- Works on both Cloudflare Workers (WASM) and native
