# Decision 06: Argon2id Support in Native Backend

## Problem

Bitwarden clients support two KDF types: PBKDF2 (type 0) and Argon2id (type 1). OrangeVault only supports PBKDF2 server-side because Argon2id isn't available in Web Crypto. The native backend has no such limitation.

## Analysis

- **PBKDF2 only**: Simple, consistent across backends. Clients that use Argon2id client-side still send a PBKDF2-derived hash to the server. This is the Bitwarden design: heavy KDF is client-side; server verifies a hash-of-the-hash with minimal iterations (1 PBKDF2 iteration).
- **Argon2id on native only**: The native server could accept Argon2id-hashed passwords directly, giving users who register on native an additional security option. But this creates a compatibility gap: an Argon2id-registered user can't migrate to Cloudflare without a password change.
- **Argon2id everywhere**: Not possible — Web Crypto doesn't support Argon2id, and compiling argon2 to WASM is risky (64MB memory may exceed Worker limits).

## Decision: PBKDF2-only for server-side verification

Both backends use PBKDF2 for server-side password verification (1 iteration, as Vaultwarden does). The client-side KDF (PBKDF2 600K or Argon2id) is the client's responsibility — the server stores whatever hash the client sends. The native backend advertises both KDF types in `/api/config` but still verifies the client-provided hash with PBKDF2.

This means:
- Users can use Argon2id **client-side** (their device derives the master key with Argon2id)
- The server receives the client's master password hash and verifies it with **1 PBKDF2 iteration**
- No server-side Argon2id dependency needed
- Full compatibility across backends

## Consequences

- Native backend doesn't need the `argon2` crate
- Simpler dependency graph
- Consistent behavior across Cloudflare and native
- Users on native don't get "extra" server-side security — but the security model is client-side anyway
