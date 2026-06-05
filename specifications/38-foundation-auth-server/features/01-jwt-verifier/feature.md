# Feature 01: JWT Verifier

## Description

Add cryptographic JWT signature verification to `JwtManager`. Currently `JwtToken::from_token()` decodes claims via base64 without verifying the signature — any attacker could forge a token. `JwtVerifier` uses the `jwt-simple` crate (already in Cargo.toml) to verify EdDSA (Ed25519), RS256, and ES256 signatures with full claim validation.

## Module

`backends/foundation_auth/src/shared/jwt.rs` — extend existing module

## API Surface

```rust
/// Algorithm supported for JWT verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtAlgorithm {
    EdDSA,   // Ed25519
    RS256,   // RSA with SHA-256
    ES256,   // ECDSA P-256 with SHA-256
}

/// Configuration for JWT verification.
pub struct JwtVerifierConfig {
    pub allowed_algorithms: Vec<JwtAlgorithm>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    /// If Some, tokens signed with this key ID are accepted.
    pub key_id: Option<String>,
    /// Public key in PEM format or raw bytes (Ed25519).
    pub public_key: PublicKeySource,
}

/// JWT verifier — validates signatures and claims.
pub struct JwtVerifier {
    // jwt-simple Validators for each supported algorithm
    validators: Vec<(JwtAlgorithm, jwt_simple::prelude::JWTClaims)>,
    config: JwtVerifierConfig,
}

impl JwtVerifier {
    /// Create from configuration.
    pub fn from_config(config: JwtVerifierConfig) -> Result<Self, JwtError>;

    /// Verify a JWT token, returning parsed claims on success.
    pub fn verify(&self, token: &str) -> Result<VerifiedClaims, JwtError>;

    /// Verify with explicit key ID override (used with JWKS).
    pub fn verify_with_key(&self, token: &str, kid: &str) -> Result<VerifiedClaims, JwtError>;
}

/// Claims extracted after successful verification.
pub struct VerifiedClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: chrono::DateTime<chrono::Utc>,
    pub iat: chrono::DateTime<chrono::Utc>,
    pub nbf: Option<chrono::DateTime<chrono::Utc>>,
    pub jti: Option<String>,
    pub scope: Option<String>,
    pub nonce: Option<String>,
    /// Custom claims not part of the standard set.
    pub custom: serde_json::Map<String, serde_json::Value>,
}

/// JWT signing key pair for the server module.
pub struct JwtSigningKey {
    // Ed25519 key pair (default), RSA, or EC P-256
    keypair: SigningKeypair,
}

impl JwtSigningKey {
    /// Generate a new Ed25519 key pair.
    pub fn generate_ed25519() -> Self;

    /// Sign a set of claims, returning a JWT string.
    pub fn sign_claims(&self, claims: &serde_json::Value) -> Result<String, JwtError>;
}
```

## Implementation Details

### What `jwt-simple` provides
- `Ed25519KeyPair`, `RS256KeyPair`, `ES256KeyPair` — key pair types
- `Claims` — claim builder with iss, aud, sub, exp, nbf, jti, custom
- `VerifierBuilder` — configure allowed algorithms, issuers, audiences
- Already supports all three algorithms we need

### Verification flow
1. Parse JWT header to extract `alg` and `kid`
2. Check algorithm is in `allowed_algorithms` list (reject if not)
3. Use `jwt-simple` verifier with configured public key
4. Verify signature
5. Validate claims: `iss` matches configured issuer (if set), `aud` matches (if set), `exp` not passed, `nbf` not future
6. Return `VerifiedClaims` on success

### Claim validation rules
- `iss` — strict equality match if configured
- `aud` — must contain configured audience if set; JWT `aud` can be string or array
- `exp` — token must not be expired
- `nbf` — if present, token must not be before this time
- `sub` — required, must be non-empty

### Breaking change
- `JwtToken::from_token()` (existing, unverified) is kept but deprecated
- New `JwtToken::from_verified_token(token: &str, verifier: &JwtVerifier)` replaces it
- `JwtManager` gains optional `JwtVerifier` field — when set, all token validation goes through it

### Error additions to `JwtError`
- `SignatureVerificationFailed` — signature does not match
- `AlgorithmNotAllowed` — token uses an algorithm not in allowlist
- `InvalidIssuer` — issuer claim does not match expected
- `InvalidAudience` — audience claim does not match expected
- `TokenNotYetValid` — nbf is in the future

## Dependencies

- `jwt-simple = "0.12"` — already in Cargo.toml
- No new dependencies

## Testing

- Sign a token with Ed25519 key, verify with matching public key → success
- Sign with Ed25519, verify with different key → `SignatureVerificationFailed`
- Token with wrong algorithm → `AlgorithmNotAllowed`
- Token with wrong issuer → `InvalidIssuer`
- Token with wrong audience → `InvalidAudience`
- Expired token → existing `TokenExpired`
- Not-yet-valid token → `TokenNotYetValid`
- Forged token (modified payload, same signature) → `SignatureVerificationFailed`
