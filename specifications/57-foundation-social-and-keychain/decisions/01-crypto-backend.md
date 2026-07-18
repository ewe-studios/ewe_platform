# Decision 01: Crypto — foundation_auth (JwtSigningKey, TOTP, PBKDF2)

**Status:** Resolved (2026-07-18)

## Problem

The keychain needs crypto for PBKDF2 (password verification), RSA/EdDSA (JWT signing), HMAC (TOTP), and SHA-256.

## Analysis

`foundation_auth` now provides:
- `JwtSigningKey` (EdDSA/RS256/ES256) — JWT signing/verification
- `TOTPSecret` — TOTP generation/verification (RFC 6238, HMAC-based)
- `shared::pbkdf2` — PBKDF2-HMAC-SHA256 derive/verify (`pbkdf2` crate native, Web Crypto WASM)

All three are cross-platform: native uses Rust crates, WASM uses Web Crypto API.

## Decision: All crypto via foundation_auth

- **JWT signing/verification**: `foundation_auth::shared::jwt::JwtSigningKey`
- **TOTP**: `foundation_auth::shared::two_factor::TOTPSecret`
- **PBKDF2**: `foundation_auth::shared::pbkdf2::{pbkdf2_derive, pbkdf2_verify, SERVER_PASSWORD_ITERATIONS}`

foundation_keychain imports these directly — no crypto code of its own.

## Consequences

- foundation_keychain has zero crypto implementation code
- PBKDF2 outputs are identical between backends (same algorithm, same inputs → same hash)
- WebCrypto 100K iteration limit is baked into `SERVER_PASSWORD_ITERATIONS` constant in foundation_auth
- Adding a new crypto primitive goes into foundation_auth, not keychain
