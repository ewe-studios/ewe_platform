# Feature 02: JWKS Manager

## Description

Fetch, cache, and manage JSON Web Key Sets from OIDC providers. JWKS is how clients discover the public keys needed to verify JWT signatures. The manager supports key rotation (multiple keys in a single JWKS), TTL-based caching, and works on both native (via `SimpleHttpClient`) and wasm (via browser fetch).

## Modules

- `backends/foundation_auth/src/shared/jwks.rs` — shared types: `Jwks`, `Jwk`, `JwksManager`, `JwksError`
- `backends/foundation_auth/src/native/jwks.rs` — native HTTP fetch implementation
- `backends/foundation_auth/src/wasm_bindgen/jwks.rs` — wasm HTTP fetch implementation (gated behind `wasm-bindgen-oauth` feature)

## API Surface

```rust
/// A single JSON Web Key.
pub struct Jwk {
    pub key_id: String,          // kid
    pub key_type: String,        // kty: "OKP", "RSA", "EC"
    pub algorithm: String,       // alg: "EdDSA", "RS256", "ES256"
    pub use_: String,            // use: "sig"
    pub curve: Option<String>,   // crv: "Ed25519", "P-256"
    pub public_key: Vec<u8>,     // x (and y for EC)
    pub modulus: Option<Vec<u8>>, // n (RSA only)
    pub exponent: Option<Vec<u8>>, // e (RSA only)
}

/// A JSON Web Key Set.
pub struct Jwks {
    pub keys: Vec<Jwk>,
    pub issuer: Option<String>,  // Optional iss from JWKS endpoint
}

impl Jwks {
    /// Find a key by key ID.
    pub fn find_by_kid(&self, kid: &str) -> Option<&Jwk>;

    /// Parse from JSON string.
    pub fn from_json(json: &str) -> Result<Self, JwksError>;

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, JwksError>;
}

/// JWKS manager — fetches, caches, and provides keys for JWT verification.
pub struct JwksManager {
    jwks_url: String,
    cached_jwks: Option<(Jwks, std::time::Instant)>,
    cache_ttl: std::time::Duration,
    http_client: HttpClient,  // platform-specific
}

impl JwksManager {
    /// Create a new JWKS manager.
    pub fn new(jwks_url: String, cache_ttl: Duration) -> Self;

    /// Fetch JWKS from the URL, replacing cache.
    pub async fn fetch(&mut self) -> Result<&Jwks, JwksError>;

    /// Get the cached JWKS, refreshing if expired.
    pub async fn get(&mut self) -> Result<&Jwks, JwksError>;

    /// Find a key by key ID, fetching if cache is empty or expired.
    pub async fn find_key(&mut self, kid: &str) -> Result<Option<&Jwk>, JwksError>;

    /// Get a JWT verifier configured with the current JWKS.
    pub async fn verifier(&mut self) -> Result<JwtVerifier, JwksError>;
}
```

## Implementation Details

### JWKS JSON format
```json
{
  "keys": [
    {
      "kty": "OKP",
      "crv": "Ed25519",
      "x": "base64url-encoded-public-key",
      "kid": "key-2025-01",
      "use": "sig",
      "alg": "EdDSA"
    }
  ]
}
```

### Key type handling
- **OKP (Ed25519)**: `kty=OKP`, `crv=Ed25519`, `x` = 32 bytes base64url
- **RSA**: `kty=RSA`, `n` = modulus, `e` = exponent, `alg=RS256`
- **EC (P-256)**: `kty=EC`, `crv=P-256`, `x` + `y` = point coordinates, `alg=ES256`

### Caching strategy
- Cache TTL defaults to 1 hour
- On cache miss or expiry: fetch, parse, replace cache
- `fetch()` always fetches (force refresh)
- `get()` returns cached if valid, fetches otherwise
- No background refresh — lazy refresh on access

### Native implementation
- Uses `foundation_netio::simple_http::client::SimpleHttpClient`
- GET to JWKS URL, parse JSON response

### Wasm implementation
- Uses browser `fetch` API via `web-sys`
- GET to JWKS URL, parse JSON response
- Gated behind `wasm-bindgen-oauth` feature

### Error additions to `JwtError` (or new `JwksError`)
- `JwksFetchFailed` — HTTP request failed
- `JwksParseError` — JSON parsing failed
- `KeyNotFound` — requested kid not found in JWKS
- `UnsupportedKeyType` — kty not recognized (not OKP/RSA/EC)
- `UnsupportedAlgorithm` — alg not supported

## Dependencies

- Existing: `foundation_netio`, `serde`, `serde_json`, `base64`
- Existing: `jwt-simple` (for Jwk → verifier conversion)
- Wasm: `web-sys` (already available via `wasm-bindgen-oauth` feature)

## Testing

- Parse JWKS JSON with Ed25519 key → `find_by_kid` returns key
- Parse JWKS JSON with multiple keys → correct key found by kid
- Cache hit → no HTTP request made
- Cache expiry → HTTP request made, cache updated
- `fetch()` → always makes HTTP request
- Wasm: parse JWKS JSON (unit test, no HTTP)
- Error: malformed JSON → `JwksParseError`
- Error: kid not found → `KeyNotFound`

## Sync/Async Notes

The `async fn` methods shown are the primary implementation. For sync callers,
use valtron bridging (no tokio):

```rust
let task = from_future(async move { manager.get().await });
let stream = execute(task, None)?;
collect_one(stream).ok_or_else(|| JwksError::NoResult)?
```

See the valtron skill and requirements.md "Valtron Bridging" section for full patterns.
