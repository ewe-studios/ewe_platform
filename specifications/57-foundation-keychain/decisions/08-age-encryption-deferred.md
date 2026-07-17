# Decision 08: age Encryption — Deferred

## Context

[`age`](https://crates.io/crates/age) is a modern, WASM-compatible encryption library (X25519 + XChaCha20-Poly1305 + scrypt) with streaming, multi-recipient, and passphrase support.

## Potential Uses in Keychain

1. **JWT signing key at rest** — encrypt the JWK in KV/filesystem with a passphrase from `KEYCHAIN_SECRET` env var
2. **Admin backups** — age-encrypted database dumps with multi-recipient (admin SSH key + recovery key)
3. **Send file defense-in-depth** — server-side encryption layer on top of client-side encryption
4. **Audit log integrity** — encrypted event logs that even DB admins can't read

## Decision: Defer

Not needed for F00–F03 (getting the Bitwarden API working). The first three features are about API compatibility, not at-rest encryption hardening. Zero-knowledge means vault data is already client-encrypted.

## When to Revisit

When building the admin ops story: backups, key rotation, audit log hardening, multi-instance key distribution. The highest-ROI target is JWT signing key storage — a one-line change from plaintext JWK to `age::encrypt(secret, jwk_bytes)`.
