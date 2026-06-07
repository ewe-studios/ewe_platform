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
    /// Uses HttpApp::new_writer() from foundation_http.
    pub fn http_app(&self) -> HttpApp<Arc<dyn ServeWriter>>;

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

### Route registration
The `http_app()` method registers all routes:

**TODO**: foundation_http predominantly for native provides the Serve trait and ServeWeb and ServeCf each for the different environments we need to support which you should build for, but why are you sto adament to use ServeWriter?

```rust
pub fn http_app(&self) -> HttpApp<Arc<dyn ServeWriter>> {
    let mut app = HttpApp::new_writer();

    // Store config, services, stores in ContextBag
    app.ctx.store(Arc::new(self.config.clone()));
    app.ctx.store(Arc::new(UserService::new(self.db.clone())));
    app.ctx.store(Arc::new(ClientService::new(self.db.clone())));
    app.ctx.store(Arc::new(TokenService::new(self.config.clone(), self.db.clone())));
    app.ctx.store(Arc::new(SessionService::new(self.db.clone())));

    // OIDC endpoints
    app.route_writer(SimpleMethod::GET, "/.well-known/openid-configuration");
    app.route_writer(SimpleMethod::GET, "/oidc/authorize");
    app.route_writer(SimpleMethod::POST, "/oidc/token");
    app.route_writer(SimpleMethod::GET, "/oidc/userinfo");
    app.route_writer(SimpleMethod::GET, "/oidc/jwks");
    app.route_writer(SimpleMethod::POST, "/oidc/introspect");
    app.route_writer(SimpleMethod::POST, "/oidc/device_authorization");

    // Auth endpoints
    app.route_writer(SimpleMethod::POST, "/auth/v1/login");
    app.route_writer(SimpleMethod::POST, "/auth/v1/mfa");

    // Middleware
    app.middleware(RateLimiter::new(10, Duration::from_secs(60)));  // login/token endpoints
    app.middleware(CorsMiddleware::new(CorsConfig::new()
        .with_allowed_origin("*")
        .with_allowed_method("GET")
        .with_allowed_method("POST")
        .with_allowed_header("Authorization")
        .with_allowed_header("Content-Type")));

    app
}
```

### ContextBag services
All services stored in `ContextBag` and retrieved by handlers via `ServeWriterFactory::create(bag)`.

### Server is native-only
The `server()` method is gated behind `#[cfg(not(target_arch = "wasm32"))]` because it requires TCP listening. The `http_app()` method is shared — it can be used on wasm (e.g., Cloudflare Workers) via the wasm HTTP dispatch path.

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

The IdP server supports two handler types for different deployment targets:

### Native (ServeWriter)
- `ServeWriter` trait — sync handler: `fn serve_writer(&self, bag, req, conn) -> ConnectionResult`
- Registered via `app.route_writer(method, path)` using `HttpApp::new_writer()`
- Handlers implement `ServeWriterFactory`: `fn create(bag: &ContextBag) -> Self`
- Internally, ServeWriter handlers bridge to async services using valtron
  (`from_future` + `execute` + `collect_one`) when they need to call `*_async` methods
- See `backends/foundation_http/src/shared/handlers/health.rs` for the pattern

### WASM/Browser (WebServe)
- `WebServe` trait — async handler: `async fn serve_web(&self, bag, req, conn) -> WebConnectionResult`
- Registered via `app.route_web(method, path)` using `HttpApp::new_web()`
- Handlers call `*_async` methods directly — no valtron bridging needed
- See `backends/foundation_http/src/shared/serve_web.rs` for the trait

### IdpServer Design

The `http_app()` method returns `HttpApp<Arc<dyn ServeWriter>>` for native TCP serving.
For WASM deployment, a separate `web_app()` method returns `HttpApp<Arc<dyn WebServe>>`.
Both register the same logical endpoints but use different handler types.

The `http_app()` builder stores services (UserService, TokenService, etc.) in ContextBag.
Handlers retrieve them via `ServeWriterFactory::create(bag)` and bridge to async via valtron.
