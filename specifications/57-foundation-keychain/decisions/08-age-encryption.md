# Decision 08: age Encryption — Used for SSH Key At-Rest

## Context

[`age`](https://crates.io/crates/age) is a modern encryption library (X25519 + XChaCha20-Poly1305 + scrypt) that compiles to both native and `wasm32-unknown-unknown`. It supports two modes:

1. **X25519 recipients** — asymmetric (encrypt to public key, decrypt with identity)
2. **scrypt passphrase** — symmetric, derives the key from a passphrase/secret

## Decision: Use age (scrypt passphrase mode) for SSH key at-rest encryption

The SSH key provisioning feature (decision 10 / F04) stores private keys. These are encrypted at rest using age's scrypt passphrase mode — the passphrase is `KEYCHAIN_MASTER_KEY`. Same code on native and Workers.

## Other Potential Uses (Deferred)

Still deferred until the admin ops story:
1. **JWT signing key at rest** — encrypt the JWK in KV/filesystem
2. **Admin backups** — age-encrypted database dumps with multi-recipient
3. **Audit log integrity** — encrypted event logs

## Consequences

- `age` is now a core dependency (SSH key encryption), not deferred
- One `KEYCHAIN_MASTER_KEY` secret drives at-rest encryption
- scrypt passphrase mode means no keypair management
- Other at-rest encryption uses (JWT key, backups) can adopt the same pattern later
