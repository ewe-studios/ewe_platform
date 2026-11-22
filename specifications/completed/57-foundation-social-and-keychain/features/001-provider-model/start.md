---
feature: "01-provider-model"
spec: "57-foundation-social-and-keychain"
depends: "decisions/00-identity-broker-pattern.md"
status: "complete"
---

# Feature 01: Provider Model + CRUD Service

## Goal

Define the `UpstreamProvider` entity and a service for CRUD operations. This is the data model that all other features depend on.

## WHAT

### UpstreamProvider Entity

```rust
pub struct UpstreamProvider {
    pub id: String,                    // "google", "github", custom slug
    pub name: String,                  // "Google", "GitHub"
    pub provider_type: ProviderType,   // Oidc, Oauth2
    pub client_id: String,
    pub client_secret_ciphertext: Option<Vec<u8>>,  // encrypted
    pub encryption_key_id: String,     // identifies the key used for encryption
    pub authorization_url: Option<String>, // set if no discovery
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub discovery_url: Option<String>, // OIDC discovery URL
    pub scopes: Vec<String>,           // ["openid", "email", "profile"]
    pub is_active: bool,
    pub mapping_config: ProviderMapping, // claim mapping rules
    pub created_at: i64,
    pub updated_at: i64,
}

pub enum ProviderType {
    Oidc,     // Full OIDC (discovery + userinfo + ID token)
    Oauth2,   // OAuth2 only (custom authorize/token/userinfo URLs)
}

pub struct ProviderMapping {
    pub email_field: String,        // default: "email"
    pub email_verified_field: String, // default: "email_verified"
    pub name_field: String,         // default: "name"
    pub subject_field: String,      // default: "sub" (OIDC) or "id" (OAuth2)
    pub username_field: Option<String>, // default: "preferred_username"
}
```

### ProviderService

```rust
pub struct ProviderService<QS: QueryStore + 'static> {
    query_store: Arc<QS>,
    crypto: ProviderCrypto,  // encryption/decryption wrapper
}

impl ProviderService {
    pub fn create(&self, provider: UpstreamProvider) -> Result<UpstreamProvider, Error>;
    pub fn find_by_id(&self, id: &str) -> Result<Option<UpstreamProvider>, Error>;
    pub fn find_active(&self) -> Result<Vec<UpstreamProvider>, Error>;
    pub fn update(&self, id: &str, updates: ProviderUpdate) -> Result<UpstreamProvider, Error>;
    pub fn delete(&self, id: &str) -> Result<(), Error>;
    pub fn set_secret(&self, id: &str, plaintext_secret: &str) -> Result<(), Error>;
    pub fn get_secret(&self, id: &str) -> Result<String, Error>;  // decrypt
}
```

### ProviderCrypto

Encrypts/decrypts provider secrets using ChaCha20-Poly1305 with an Argon2id-derived key from `PROVIDER_SECRET_KEY` env var.

## HOW

### Files to create

- `backends/foundation_auth/src/server/models/provider.rs` — UpstreamProvider, ProviderType, ProviderMapping
- `backends/foundation_auth/src/server/services/provider_service.rs` — ProviderService + ProviderCrypto
- `backends/foundation_auth/src/server/models/mod.rs` — add `mod provider; pub use`

### Dependencies

- `chacha20poly1305` crate (new dependency in Cargo.toml)
- `argon2` crate (already in dependency tree via password hashing)
- Migration 024 (feature 02) — the service uses it, but define the model first

### Validation

- `create` validates: id is non-empty, client_id is non-empty, at least one of discovery_url or (authorization_url + token_url) is set
- `set_secret` encrypts before storing
- `get_secret` decrypts and returns plaintext (only used during auth flow, never logged)

---

_Created: 2026-07-17_
