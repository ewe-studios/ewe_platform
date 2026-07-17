# F02: Native Backend

## Goal

Implement the native (Docker/VPS) backend using `foundation_db`'s native backends (`Turso` or `Libsql` for SQL, `JsonFile` or filesystem for blobs) + `foundation_auth`'s `JwtSigningKey` for crypto + `foundation_http` for HTTP serving + `foundation_netio` for WebSocket notifications.

## Work

### 1. Native Server Bootstrap (`server/native.rs`)

```rust
pub async fn run_server(config: KeychainConfig) -> Result<(), AppError> {
    // Initialize foundation_db StorageProvider
    let db = StorageProvider::new(StorageBackend::Turso { url: config.db_url })?;
    // Run migrations using foundation_db::SchemaMigration
    let migrator = SchemaMigration::new(&db, "migrations/")?;
    migrator.run()?;

    // Initialize foundation_auth JWT signing key
    let signing_key = JwtSigningKey::generate_ed25519(); // or RS256

    // Initialize blob store (filesystem or JsonFile)
    let blob = StorageProvider::new(StorageBackend::JsonFile { path: config.data_dir })?;

    // Build context
    let ctx = KeychainContext { db, blob, signing_key };

    // Start foundation_http server wired to api/ handlers
    let server = foundation_http::Server::new(config.listen_addr)?;
    server.register_routes(&ctx).await?;

    // Start cron scheduler
    let scheduler = Scheduler::new();
    scheduler.add("0 */6 * * *", || purge_expired_sends(&ctx.db, &ctx.blob)).await?;
    scheduler.add("0 0 * * *", || purge_trashed_ciphers(&ctx.db)).await?;

    // Start WebSocket notification server (foundation_netio)
    let ws_server = foundation_netio::WebSocketServer::new(config.ws_addr)?;
    ws_server.start(NotificationHandler::new()).await?;

    // Run until shutdown
    server.run().await
}
```

### 2. Native HTTP (`native/http.rs`)

Wire `foundation_http` to all ~150 api/ handlers:
- h2/h1 server (F47-complete in spec-41)
- Route registration matching the Bitwarden API paths
- CORS + security headers middleware
- File upload handling (multipart for attachments, raw bytes for send files)

### 3. Native Notifications (`native/notifier.rs`)

WebSocket server via `foundation_netio` (F51-complete):
- Accept WebSocket connections on `/notifications/hub`
- SignalR handshake: JSON text frame `{"protocol":"messagepack","version":1}\x1E`
- Accept with binary `{}\x1E`
- Per-user connection registry: `HashMap<user_uuid, Vec<WsSender>>`
- Broadcast SignalR MessagePack frames to matching users
- 15s ping via timer (no DO alarms needed — native has a process)

### 4. Native Crypto

**No custom crypto needed.** Native uses:
- `foundation_auth::shared::jwt::JwtSigningKey` for JWT signing/verification (RS256/ES256/EdDSA)
- Standard `pbkdf2` crate for PBKDF2-HMAC-SHA256 (no WebCrypto 100K limit — can do 600K)
- `foundation_auth::shared::two_factor::TOTPSecret` for TOTP (uses `hmac` + `sha2` crates internally)

### 5. Schema Migration

Use `foundation_db::SchemaMigration` to run `migrations/0001_initial.sql`:
```rust
let migrator = SchemaMigration::new(&db, "migrations/")?;
migrator.run()?;
```

### Success Criterion

- `cargo check --features backend-native` passes on `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`
- Server starts, runs migrations, listens on configured port
- Bitwarden CLI can register, login, sync, create/delete ciphers against the native server
- WebSocket notifications work (SignalR handshake + binary frames)
- Cron jobs execute on schedule
