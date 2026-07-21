# Decision 06: Password Hashing — PBKDF2 (both) + Argon2id (native)

**Status:** Resolved (2026-07-18)

## Problem

Bitwarden clients support two KDF types: PBKDF2 (type 0) and Argon2id (type 1). The server must verify passwords hashed by either. WebCrypto (WASM) only supports PBKDF2.

## Analysis

- **PBKDF2**: Available everywhere — `pbkdf2` crate (native), Web Crypto (WASM). Server uses 100K iterations for re-hashing the client-derived hash.
- **Argon2id**: Only available natively (`argon2` crate). Not in WebCrypto. Bitwarden defaults: 64MB memory, 4 parallelism, 3 iterations.

## Decision: Both in foundation_auth::password_hash

`foundation_auth::shared::password_hash` provides:
- `pbkdf2_derive` / `pbkdf2_verify` — both native (sync) and WASM (async via WebCrypto)
- `argon2id_derive` / `argon2id_verify` — native only (feature-gated)
- `verify_password(kdf_type, ...)` — dispatches based on KDF type
- `KDF_TYPE_PBKDF2 = 0`, `KDF_TYPE_ARGON2ID = 1` — Bitwarden wire format constants

On WASM, `verify_password` with Argon2id returns an error — clients using Argon2id must use a native server.

## Consequences

- Native server supports both KDF types — clients can use either
- WASM server supports PBKDF2 only — clear error if Argon2id client connects
- foundation_keychain uses `foundation_auth::password_hash` — no crypto code of its own
