---
feature: "IdP Server"
description: "IdP HTTP server using foundation_http, router setup, CORS, rate limiting"
status: "pending"
priority: "high"
depends_on: ["00-query-store-stream-parity", "01-jwt-verifier"]
estimated_effort: "large"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0%
---

# Feature 09: IdP Server

## Description

The main IdP HTTP server built on `foundation_http`. Provides a builder pattern to configure the server, register all OIDC routes, and start serving. Gated behind the `server` feature flag.

## Module

`backends/foundation_auth/src/server/idp_server.rs`

## API Surface

```rust
/// IdP server configuration.
pub struct IdpConfig {
    /// Issuer URL (used in tokens and discovery).
    pub issuer_url: String,
    /// Signing key for JWT tokens (Ed25519 default).
    pub signing_key: JwtSigningKey,
    /// Access token lifetime (default: 15 minutes).
    pub access_token_ttl: Duration,
    /// Refresh token lifetime (default: 7 days).
    pub refresh_token_ttl: Duration,
    /// Session cookie lifetime (default: 8 hours).
    pub session_ttl: Duration,
    /// Require PKCE for authorization code flow (default: true).
    pub require_pkce: bool,
    /// Allowed signing algorithms (default: [EdDSA, RS256, ES256]).
    pub allowed_algorithms: Vec<Algorithm>,
    /// Password policy configuration.
    pub password_policy: PasswordPolicy,
}

/// Default password policy.
pub struct PasswordPolicy {
    pub min_length: usize,       // default: 12
    pub require_uppercase: bool, // default: true
    pub require_lowercase: bool, // default: true
    pub require_number: bool,    // default: true
    pub require_special: bool,   // default: true
}

/// IdP server — builds and runs the OIDC HTTP server.
pub struct IdpServer {
    config: IdpConfig,
    db: StorageProvider,  // foundation_db backend (Turso, libsql, memory, etc.)
}

impl IdpServer {
    /// Create a new IdP server.
    pub fn new(config: IdpConfig, db: StorageProvider) -> Self;

    /// Build the HttpApp with all OIDC routes registered.
    /// Uses HttpApp::new() from foundation_http.
    pub fn http_app(&self) -> HttpApp<Arc<dyn Serve>>;

    /// Create an HttpServer with the given address.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn server(self, addr: &str) -> foundation_http::native::server::HttpServer {
        self.http_app().server(addr)
    }

    /// Create an HttpServer with custom config.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn server_with_config(
        self,
        addr: &str,
        config: foundation_http::native::server::ServerConfig,
    ) -> foundation_http::native::server::HttpServer {
        self.http_app().server_with_config(addr, config)
    }
}

impl IdpConfig {
    /// Create a default config with generated Ed25519 signing key.
    pub fn new(issuer_url: String) -> Self;
}
```

## Implementation Details

### Architecture — Async Core + Native Adapter

Each handler has an **async core method** that contains the actual business logic.
The three `foundation_http` traits are served as follows:

- **`Serve`** (native, sync) — needs valtron bridge → separate `ServeAdapter` struct
- **`ServeCf`** (CF Workers, async) — implemented directly on `IdpHandlerCore`
- **`ServeWeb`** (browser/WASM, async) — implemented directly on `IdpHandlerCore`

```
                     ┌─────────────────────────────────┐
                     │    IdpHandlerCore               │
                     │                                 │
                     │ async fn authorize(...)          │  ← implements ServeCf
                     │ async fn token(...)              │  ← implements ServeWeb
                     │ async fn userinfo(...)           │
                     │ ... all endpoints ...            │
                     └────────────┬────────────────────┘
                                  │
                     ┌────────────┘
                     ▼
           ┌─────────────────┐
           │  ServeAdapter    │
           │  (native sync)   │  ← only adapter struct needed
           │  wraps Arc<Core> │     valtron: from_future + execute + collect_one
           │  valtron bridge  │
           └─────────────────┘
```

#### The Async Core (implements ServeCf + ServeWeb directly)

```rust
/// Shared async business logic for all IdP endpoints.
/// Implements ServeCf (CF Workers) and ServeWeb (WASM) directly — both are async.
pub struct IdpHandlerCore {
    config: Arc<IdpConfig>,
    db: StorageProvider,
}

impl IdpHandlerCore {
    pub async fn authorize(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn token(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn userinfo(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    // ... jwks, introspect, device_authorize, login, mfa ...
}

// === Cloudflare Workers (ServeCf) — async, direct impl ===
impl ServeCf for IdpHandlerCore {
    async fn serve_cf(
        &self, bag: &ContextBag, req: Request, conn: &mut CfConnection,
    ) -> CfConnectionResult {
        self.authorize(bag, &req).await.to_cf_connection_response(conn)
    }
}

// === Browser/WASM (ServeWeb) — async, direct impl ===
impl ServeWeb for IdpHandlerCore {
    async fn serve_web(
        &self, bag: &ContextBag, req: Request, conn: &mut WebConnection,
    ) -> WebConnectionResult {
        self.authorize(bag, &req).await.to_web_connection_response(conn)
    }
}
```

#### Native Adapter (Serve — sync, valtron bridge)

The only adapter struct. Wraps `Arc<IdpHandlerCore>` and bridges async core to sync trait.

```rust
pub struct ServeAdapter {
    core: Arc<IdpHandlerCore>,
}

impl Serve for ServeAdapter {
    fn serve(&self, bag: &ContextBag, req: Request, conn: &mut Connection) -> ConnectionResult {
        let core = Arc::clone(&self.core);
        let bag = bag.clone();
        let req = req.clone();
        let task = from_future(async move {
            core.authorize(&bag, &req).await
        });
        let stream = execute(task, None)?;
        collect_one(stream)
            .ok_or_else(|| IdpError::NoResult)?
            .to_connection_response(conn)
    }
}
```

### Route Registration

```rust
impl IdpServer {
    /// Native TCP server — uses Serve trait, valtron bridges async core.
    pub fn http_app(&self) -> HttpApp<Arc<dyn Serve>>;

    /// Cloudflare Workers — uses ServeCf trait, valtron bridges async core.
    pub fn cf_app(&self) -> HttpApp<Arc<dyn ServeCf>>;

    /// Browser/WASM — uses ServeWeb trait, direct async calls.
    pub fn web_app(&self) -> HttpApp<Arc<dyn ServeWeb>>;
}
```

### ContextBag services
All services stored in `ContextBag` and accessed by `IdpHandlerCore`. The three
adapters are thin — they just bridge the transport layer to the core.

### Server is native-only
The `server()` method is gated behind `#[cfg(not(target_arch = "wasm32"))]` because
it requires TCP listening. The `http_app()`, `cf_app()`, and `web_app()` methods
are shared — they can be used on any platform via the appropriate HTTP dispatch path.

## Dependencies

- New: `foundation_http = { workspace = true }` (gated behind `server` feature)
- Existing: `foundation_db` (StorageProvider, QueryStore, KeyValueStore)
- Existing: `serde`, `serde_json`, `tracing`

## Cargo.toml Changes

```toml
[dependencies]
foundation_http = { workspace = true, optional = true }

[features]
server = ["foundation_http"]
```

## Testing

- `IdpConfig::new()` → defaults set correctly, Ed25519 key generated
- `http_app()` → router has all expected routes
- Server starts and responds to health check (when integrated with Feature 12 handlers)
- `IdpConfig` builder overrides defaults correctly

## Handler Architecture

Each endpoint has one async core method in `IdpHandlerCore` and three thin adapters:

### Native (`Serve` trait)
- `ServeAdapter` wraps `IdpHandlerCore`
- `fn serve()` calls `from_future(async move { core.authorize(...).await })` + `execute` + `collect_one`
- Same valtron bridge pattern for every endpoint — no logic duplication

### Cloudflare Workers (`ServeCf` trait)
- `ServeCfAdapter` wraps `IdpHandlerCore`
- Same valtron bridge pattern as native — different transport type

### Browser/WASM (`ServeWeb` trait)
- `ServeWebAdapter` wraps `IdpHandlerCore`
- `async fn serve_web()` calls `core.authorize(...).await` directly — no bridging needed

### Why This Pattern

- **DRY**: All business logic lives once in `IdpHandlerCore`
- **Thin adapters**: Each is ~10 lines — just valtron bridge + response conversion
- **Testable**: Core can be unit-tested without HTTP framework
- **Extensible**: New `foundation_http` trait → just one more thin adapter
