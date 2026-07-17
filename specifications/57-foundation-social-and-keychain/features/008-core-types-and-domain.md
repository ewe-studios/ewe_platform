# F00: Core Types + Portable Domain Logic

## Goal

Port all Bitwarden domain logic that has no runtime coupling, using `foundation_db` traits for storage and `foundation_auth` for auth surfaces. No `worker::` types, no platform-specific code.

## Work

### 1. Error + Util (portable from orangevault)

**`core/error.rs`** — Port `AppError` enum:
```rust
pub enum AppError {
    Unauthorized(String),   // 401
    BadRequest(String),     // 400
    NotFound(String),       // 404
    Forbidden(String),      // 403
    Conflict(String),       // 409
    PayloadTooLarge(String), // 413
    TooManyRequests,        // 429
    Internal(String),       // 500
    OAuth {
        error: String,
        error_description: String,
        status: u16,
        two_factor_providers: Option<Vec<i32>>,
    },
}
```
- `to_response()` — serializes to Bitwarden error shape (PascalCase `ErrorModel` + OAuth variant)
- `into_response()` — converts `Result<Response, AppError>` to HTTP response

**`core/util.rs`** — Port helpers:
- `generate_uuid()` → `uuid::Uuid::new_v4()`
- `now_utc()` → `chrono::Utc::now().to_rfc3339_opts()`
- `now_epoch_secs()` → `chrono::Utc::now().timestamp()`
- `base64url_encode/decode`, `base64_encode/decode`, `hex_encode`
- `enforce_content_length`, `enforce_body_len`, `enforce_declared_size` (upload guards)
- `MAX_UPLOAD_BYTES = 100 * 1024 * 1024`

### 2. Models (portable serde types from orangevault)

**`core/models/user.rs`** — API request/response types:
- `PreloginRequest`, `PreloginResponse` — KDF params lookup
- `RegisterRequest`, `RegisterVerificationRequest` — user registration
- `TokenRequest` — form-urlencoded grant (password, refresh_token)
- `LoginResponse` — Bitwarden-shaped response with `Key`, `PrivateKey`, `Kdf`, `UserDecryptionOptions`, `AccountKeys`, `unofficialServer`
- `ProfileResponse`, `UpdateProfileRequest`, `ChangePasswordRequest`, `ChangeKdfRequest`, `UpdateKeysRequest`, `VerifyPasswordRequest`, `SecurityStampRequest`, `ApiKeyRequest`, `ApiKeyResponse`, `DeleteAccountRequest`

**`core/models/cipher.rs`** — Cipher types:
- `CipherRequest` — create/update with `type`, `name`, `notes`, `login`/`card`/`identity`/`secure_note` variants
- `CipherResponse` — Bitwarden-shaped response with `Data` wrapper
- `AttachmentRequestV2`, `AttachmentResponse`, `AttachmentUploadResponse`
- `BulkIdsRequest`, `BulkMoveRequest`, `ImportCiphersRequest`, `CipherCollectionsRequest`

**`core/models/folder.rs`** — `FolderRequest`, `FolderResponse`

**`core/models/organization.rs`** — Org types:
- `OrgCreateRequest`, `OrganizationResponse`
- `ShareCipherRequest` — share cipher to org with collection IDs
- `CollectionCreateRequest`, `CollectionResponse`, `CollectionDetailsResponse`, `UpdateCollectionRequest`, `CollectionSelection`
- `InviteRequest`, `MemberResponse`, `ConfirmRequest`, `UpdateMemberRequest`
- `PolicyRequest`, `PolicyResponse`

**`core/models/send.rs`** — Send types:
- `SendRequest`, `SendResponse`, `SendAccessRequest`, `SendAccessResponse`
- `SendFileUploadResponse`, `SendFileDownloadResponse`
- `SEND_TYPE_TEXT = 0`, `SEND_TYPE_FILE = 1`

**`core/models/sync.rs`** — `SyncResponse`, `DomainsResponse`, `GlobalDomain`, `default_global_domains()`

**`core/models/config.rs`** — `ConfigResponse`, `EnvironmentUrls`, `ServerInfo`

### 3. SignalR Notifications (portable)

**`core/notifications/mod.rs`** — SignalR MessagePack framing:
```rust
pub enum UpdateType {
    SyncCipherUpdate = 0, SyncCipherCreate = 1, SyncLoginDelete = 2,
    SyncFolderDelete = 3, SyncCiphers = 4, SyncVault = 5,
    SyncOrgKeys = 6, SyncFolderCreate = 7, SyncFolderUpdate = 8,
    SyncCipherDelete = 9, SyncSettings = 10, LogOut = 11,
    SyncSendCreate = 12, SyncSendUpdate = 13, SyncSendDelete = 14,
}
```
- `serialize_msgpack(value: &rmpv::Value) -> Vec<u8>` — VarInt length prefix + MessagePack (BinaryMessageFormat)
- `create_notification(update_type, context_id, payload) -> rmpv::Value` — SignalR Invocation frame
- `create_ping() -> rmpv::Value` — SignalR Ping frame (`[6]`)
- `json_to_rmpv(val: &serde_json::Value) -> rmpv::Value` — JSON → MessagePack conversion

### 4. Auth Adapters (Bitwarden-specific over foundation_auth)

All crypto comes from `foundation_auth` — no crypto code in keychain:
- `foundation_auth::shared::jwt::JwtSigningKey` — JWT signing/verification
- `foundation_auth::shared::pbkdf2::{pbkdf2_derive, pbkdf2_verify, SERVER_PASSWORD_ITERATIONS}` — password verification
- `foundation_auth::shared::two_factor::TOTPSecret` — TOTP generation/verification
```rust
/// Bitwarden-specific claims added to the JWT's `custom` map.
pub struct BitwardenClaims<'a> {
    pub sstamp: &'a str,           // Security stamp (invalidation token)
    pub device: &'a str,           // Device identifier
    pub premium: bool,
    pub email: &'a str,
    pub name: &'a str,
    pub email_verified: bool,
    pub orgowner: &'a [String],    // Org IDs where user is owner
    pub orgadmin: &'a [String],    // Org IDs where user is admin
    pub orguser: &'a [String],     // Org IDs where user is member
    pub orgmanager: &'a [String],  // Org IDs where user is manager
}

impl BitwardenClaims<'_> {
    /// Serialize into a serde_json::Value for foundation_auth::JwtSigningKey::sign_claims
    pub fn to_value(&self) -> serde_json::Value { ... }

    /// Extract from VerifiedClaims::custom map
    pub fn from_verified(claims: &VerifiedClaims) -> Result<Self, AppError> { ... }
}
```

**`core/auth/stamp_middleware.rs`** — Security stamp check after `foundation_auth` JWT verify:
```rust
/// After foundation_auth verifies the JWT, check the Bitwarden-specific
/// security stamp. Operations that rotate the stamp (password change, KDF
/// change, key rotation) immediately invalidate every outstanding access token.
pub async fn check_security_stamp(
    store: &dyn QueryStore,          // foundation_db
    user_id: &str,
    expected_stamp: &str,
) -> Result<bool, AppError> {
    let row = store.query("SELECT security_stamp FROM users WHERE uuid = ?", &[DataValue::Text(user_id)])?;
    // Extract stamp from SqlRow, compare constant-time
}
```

**`core/auth/token_response.rs`** — Adapter from `foundation_auth` to Bitwarden response:
```rust
/// Build Bitwarden LoginResponse from a foundation_auth TokenPair + user data.
pub fn build_login_response(
    access_token: &str,
    refresh_token: &str,
    user: &User,                     // Bitwarden user model
) -> LoginResponse {
    LoginResponse {
        access_token: access_token.into(),
        expires_in: jwt::ACCESS_TOKEN_EXPIRY, // 7200 (2 hours)
        token_type: "Bearer".into(),
        refresh_token: refresh_token.into(),
        key: user.akey.clone(),
        private_key: user.private_key.clone(),
        kdf: user.client_kdf_type,
        kdf_iterations: user.client_kdf_iter,
        kdf_memory: user.client_kdf_memory,
        kdf_parallelism: user.client_kdf_parallelism,
        unofficial_server: true,
        user_decryption_options: UserDecryptionOptions { ... },
        account_keys: user.public_key.as_ref().map(|_| AccountKeys { ... }),
        two_factor_token: None,
    }
}
```

### 5. DB Models (portable structs matching the 16 SQL tables)

Port from `orangevault/src/db/models.rs`:
- `User`, `Cipher`, `Folder`, `Favorite`, `FolderCipher`
- `Organization`, `Membership`, `Collection`, `UserCollection`, `CipherCollection`
- `TwoFactor`, `Send`, `Event`, `OrgPolicy`, `EquivalentDomain`
- `Attachment`, `Device`

These are **not** the storage trait — they're `serde::Deserialize` structs that query functions parse from `SqlRow` using `get<T>`/`get_by_name<T>`.

### 6. Query Functions (foundation_db QueryStore-based)

Port from `orangevault/src/db/queries.rs` — ~90 functions, all using `foundation_db::QueryStore`:
```rust
// Example: find_user_by_email uses foundation_db's QueryStore
pub fn find_user_by_email(store: &dyn QueryStore, email: &str) -> Result<Option<User>, AppError> {
    let mut stream = store.query(
        "SELECT * FROM users WHERE email = ?1",
        &[DataValue::Text(email.into())],
    )?;
    // Iterate StorageItemStream<SqlRow>, parse into User struct
}

// Example: execute uses foundation_db's QueryStore::execute
pub fn insert_user(store: &dyn QueryStore, user: &User) -> Result<(), AppError> {
    store.execute(
        "INSERT INTO users (uuid, email, name, ...) VALUES (?1, ?2, ?3, ...)",
        &[
            DataValue::Text(user.uuid.clone()),
            DataValue::Text(user.email.clone()),
            DataValue::Text(user.name.clone()),
            // ...
        ],
    )?;
    Ok(())
}
```

All 90 query functions ported: user CRUD, device CRUD, cipher CRUD, folder CRUD, org CRUD, membership CRUD, collection CRUD, 2FA CRUD, send CRUD, event CRUD, policy CRUD, attachment CRUD, equivalent domains, favorites, folder-cipher links, cipher-collection links.

### 7. API Handlers (portable, trait-bound)

Port from `orangevault/src/api/` — 14 modules, ~150 route handlers. Each handler takes `foundation_db` traits and `foundation_auth` surfaces instead of `worker::` types.

### Success Criterion

- `cargo check --lib` passes with no backend features enabled
- All types compile, no `worker::` types anywhere in `core/`
- All query functions compile against `dyn QueryStore` + `dyn KeyValueStore` + `dyn BlobStore`
- SignalR MessagePack frames round-trip correctly (unit test)
- `AppError` serializes to Bitwarden-compatible JSON
