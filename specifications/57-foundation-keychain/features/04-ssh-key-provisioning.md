# F04: SSH Key Provisioning + App Registry

## Goal

Add credential provisioning APIs on top of the Bitwarden vault: app registration, SSH key generation/storage/retrieval. This turns the keychain from a password vault into a credential provisioning service that apps can use to auto-provision themselves.

## Work

### 1. App Registry

**Models:**
```rust
pub struct App {
    pub id: String,          // UUID
    pub name: String,
    pub description: Option<String>,
    pub secret_hash: String,  // Argon2id hash of the app secret
    pub created_at: String,
}
```

**Endpoints:**
```
POST /api/apps/register
  Body: { "name": "my-service", "description": "..." }
  → 200 { "app_id": "...", "secret": "<one-time>", "created_at": "..." }

GET /api/apps/:id
  Auth: Bearer <app_secret>
  → 200 { "id": "...", "name": "...", "created_at": "..." }

DELETE /api/apps/:id
  Auth: Bearer <app_secret>
  → 204
```

### 2. SSH Key Management

**Models:**
```rust
pub struct SshKey {
    pub id: String,           // UUID
    pub app_id: String,       // FK to apps
    pub name: String,
    pub key_type: String,     // "ed25519" | "rsa4096"
    pub public_key: String,   // "ssh-ed25519 AAAA..." or "ssh-rsa AAAA..."
    pub private_key_encrypted: String, // age-encrypted private key
    pub comment: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}
```

**Endpoints:**
```
POST /api/credentials/ssh-keys
  Auth: Bearer <app_secret> or X-App-Secret: <app_secret>
  Body: { "name": "deploy-key", "key_type": "ed25519", "comment": "prod", "ttl_hours": 720 }
  → 200 { "id": "...", "name": "...", "public_key": "...", "private_key": "...", "created_at": "...", "expires_at": "..." }

GET /api/credentials/ssh-keys
  Auth: Bearer <app_secret>
  → 200 { "data": [{ "id": "...", "name": "...", "public_key": "...", "created_at": "..." }, ...] }

GET /api/credentials/ssh-keys/:id
  Auth: Bearer <app_secret>
  → 200 { "id": "...", "public_key": "...", "private_key": "...", "created_at": "..." }

DELETE /api/credentials/ssh-keys/:id
  Auth: Bearer <app_secret>
  → 204
```

### 3. Key Generation

Uses the `ssh_key` crate (pure Rust, WASM-compatible):

```rust
use ssh_key::{Algorithm, PrivateKey, PublicKey};

fn generate_ed25519(comment: &str) -> (PrivateKey, PublicKey) {
    let private = PrivateKey::random(&mut rand::thread_rng(), Algorithm::Ed25519).unwrap();
    let public = private.public_key().clone();
    // Set comment on public key
    (private, public)
}
```

### 4. At-Rest Encryption

Private keys are encrypted before storage using `foundation_auth::password_hash` or the `age` crate (decision 08). The encryption key is derived from a `KEYCHAIN_MASTER_KEY` env var.

```rust
// On creation:
let encrypted = age_encrypt(&master_key, &private_key.to_bytes())?;
store.private_key_encrypted = base64_encode(&encrypted);

// On retrieval:
let private_key_bytes = age_decrypt(&master_key, &base64_decode(&stored.private_key_encrypted)?)?;
let private_key = PrivateKey::from_bytes(&private_key_bytes)?;
```

### 5. Auth Middleware

App secret authentication:
```rust
pub async fn auth_from_app_secret(req: &Request, store: &dyn QueryStore) -> Result<App, AppError> {
    let secret = extract_app_secret(req)?; // X-App-Secret header or Bearer token
    let app = find_app_by_name(store, &secret.name).await?;
    if !argon2id_verify(&secret.value, &app.secret_hash) {
        return Err(AppError::Unauthorized("Invalid app secret".into()));
    }
    Ok(app)
}
```

### 6. DB Schema

```sql
CREATE TABLE apps (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  secret_hash TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE ssh_keys (
  id TEXT PRIMARY KEY,
  app_id TEXT NOT NULL REFERENCES apps(id),
  name TEXT NOT NULL,
  key_type TEXT NOT NULL,
  public_key TEXT NOT NULL,
  private_key_encrypted TEXT NOT NULL,
  comment TEXT,
  expires_at TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_ssh_keys_app ON ssh_keys(app_id);
CREATE INDEX idx_ssh_keys_expires ON ssh_keys(expires_at);
```

### 7. Cron Job: Expired Key Cleanup

```
0 * * * * — Delete SSH keys past their expires_at
```

Uses `foundation_cronjobs` (native) or Workers cron triggers (Cloudflare).

### Success Criterion

- App registration returns a one-time secret (hashed for storage)
- SSH key generation works for Ed25519 and RSA 4096
- Private keys are encrypted at rest
- Keys are only returned on explicit retrieval (never in list)
- Expired keys are auto-deleted by cron job
- Works on both Cloudflare Workers (WASM) and native
