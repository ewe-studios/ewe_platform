# Decision 01: Crypto — foundation_auth + Web Crypto

## Problem

The keychain needs crypto for PBKDF2 (password verification), RSA/EdDSA (JWT signing), HMAC (TOTP), and SHA-256. Options:

1. **foundation_auth for native, Web Crypto for WASM** — foundation_auth already provides `JwtSigningKey` (EdDSA/RS256/ES256) and `TOTPSecret` (HMAC-based); Web Crypto handles PBKDF2 on WASM
2. **boring everywhere (native)** — additional dependency, but foundation_auth already depends on it
3. **Pure-Rust crates** — portable but slower

## Analysis

- **foundation_auth**: Already provides `JwtSigningKey::generate_ed25519()` / `::sign_claims()` for JWT, and `TOTPSecret::generate()` / `::verify()` for TOTP. These are backed by `jwt_simple` (native) and work cross-platform.
- **Web Crypto (WASM)**: Needed for PBKDF2-HMAC-SHA256 (password verification) since foundation_auth doesn't provide a PBKDF2 primitive. Max 100K iterations (WebCrypto limit).
- **PBKDF2 (native)**: Can use the `pbkdf2` crate directly — no WebCrypto 100K limit, so 600K iterations works.

## Decision: foundation_auth for JWT + TOTP, Web Crypto/pbkdf2 for PBKDF2

- **JWT signing/verification**: `foundation_auth::shared::jwt::JwtSigningKey` on both backends
- **TOTP**: `foundation_auth::shared::two_factor::TOTPSecret` on both backends
- **PBKDF2 (password verification)**: Web Crypto on WASM (100K max), `pbkdf2` crate on native (600K OK)

## Consequences

- No custom crypto trait needed for JWT/TOTP — foundation_auth handles it
- PBKDF2 is the only crypto that needs a backend-specific implementation
- PBKDF2 outputs must match between backends for cross-backend token portability (same input → same hash)
