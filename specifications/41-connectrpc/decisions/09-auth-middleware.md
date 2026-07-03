# Decision 09: Authentication Middleware

## Context

connect-go's `authn` package provides HTTP middleware for authenticating RPC requests:

```go
type AuthFunc func(ctx context.Context, req *http.Request) (any, error)

type Middleware struct {
    auth AuthFunc
    errW *connect.ErrorWriter
}
```

Key helpers:
- `BearerToken(request)` — extract `Authorization: Bearer <token>` (case-insensitive prefix)
- `InferProtocol(request)` — detect Connect/gRPC/gRPC-Web from Content-Type
- `InferProcedure(url)` — extract `/service/method` from URL path
- `SetInfo(ctx, info)` / `GetInfo(ctx)` — attach/retrieve auth info on context
- `Errorf(template, args)` — create `CodeUnauthenticated` error

The middleware wraps an `http.Handler`, authenticating before the handler runs. Errors are written using `connect.ErrorWriter` which formats errors in the correct protocol.

Our platform has `foundation_auth` with:
- JWT: `JwtManager`, `JwtVerifier`, `JwtVerifierConfig`, `VerifiedClaims`
- OAuth: `OAuthConfig`, `OAuthManager`, `TokenResponse`
- OIDC: `DiscoveryClient`, `OidcDiscovery`
- Sessions: `SessionManager`, `SessionConfig`
- TOTP/2FA: `TOTPSecret`, `BackupCodeSet`
- Helpers: `extract_bearer_token()`, `extract_session_token()`, `has_scope()`, `require_auth()`, `optional_auth()`
- `AuthContext` for storing auth state

## Decision

### Strategy

Build ConnectRPC authentication as middleware on top of foundation_auth. The connect-go authn pattern (AuthFunc + Middleware + helpers) translates directly, but we delegate actual credential verification to foundation_auth's existing infrastructure.

### AuthFunc Trait

```rust
/// Authentication function for ConnectRPC requests.
/// Receives the raw HTTP request before deserialization/decompression.
/// Returns authentication information on success, or ConnectError on failure.
pub trait AuthFunc: Send + Sync + 'static {
    /// Authenticate the request. Return Some(info) on success, or Err on failure.
    /// If the request doesn't require authentication (e.g., health checks),
    /// return Ok(None).
    fn authenticate(
        &self,
        request: &SimpleIncomingRequest,
    ) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>>;
}

/// Implement AuthFunc for closures.
impl<F> AuthFunc for F
where
    F: Fn(&SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>>
        + Send + Sync + 'static,
{
    fn authenticate(&self, request: &SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>> {
        (self)(request)
    }
}
```

### ConnectRPC Auth Middleware

```rust
/// HTTP middleware that authenticates ConnectRPC requests before dispatch.
/// Operates at the HTTP level (before decompression/deserialization).
pub struct AuthMiddleware<H> {
    auth: Arc<dyn AuthFunc>,
    error_writer: ErrorWriter,
    inner: H,
}

impl<H> AuthMiddleware<H> {
    pub fn new(auth: Arc<dyn AuthFunc>, inner: H, options: HandlerOptions) -> Self;
}

// Implements foundation_http's handler/middleware trait:
// 1. Call auth.authenticate(request)
// 2. On error: error_writer.write(response, request, error) → return error response
// 3. On success: attach auth info to request extensions
// 4. Forward to inner handler
```

Auth info is stored in the request's `Extensions` type-map and accessed via:

```rust
/// Retrieve authentication information from the per-call context (Decision 04 §Ctx).
pub fn get_auth_info<T: 'static>(ctx: &Ctx) -> Option<&T> {
    ctx.request.extensions.get::<T>()
}

/// Strip authentication information (runs during dispatch, before the RequestContext is
/// frozen into the shared Ctx — extensions are written middleware-side, read handler-side).
pub fn without_auth_info(ctx: &mut RequestContext) {
    ctx.extensions.remove::<AuthInfo>();
}
```

### Protocol Inference Helpers

Port from connect-go's authn:

```rust
/// Detect the RPC protocol from a request's Content-Type and method.
/// Returns None if the request doesn't appear to be an RPC request.
pub fn infer_protocol(request: &SimpleIncomingRequest) -> Option<&'static str> {
    let content_type = request.headers().get("content-type").unwrap_or("");
    let method = request.method();
    let content_type = canonicalize_content_type(content_type);

    match () {
        _ if method == SimpleMethod::Post && is_grpc_content_type(&content_type) => Some("grpc"),
        _ if method == SimpleMethod::Post && is_grpc_web_content_type(&content_type) => Some("grpc-web"), // hyphenated (R11)
        _ if method == SimpleMethod::Post && is_connect_content_type(&content_type) => Some("connect"),
        _ if method == SimpleMethod::Get && has_connect_query_params(request) => Some("connect"),
        _ => None,
    }
}

/// Extract the RPC procedure ("/service/method") from the URL path.
/// Returns None if the path doesn't contain a valid service/method pattern.
pub fn infer_procedure(url: &SimpleUrl) -> Option<String> {
    let path = url.path();
    let last_slash = path.rfind('/')?;
    let penultimate_slash = path[..last_slash].rfind('/')?;
    let procedure = &path[penultimate_slash..];
    // Verify service and method parts are non-empty
    if last_slash == path.len() - 1 || penultimate_slash == last_slash - 1 {
        return None;
    }
    Some(procedure.to_string())
}

/// Extract bearer token from Authorization header (case-insensitive "Bearer " prefix).
pub fn bearer_token(request: &SimpleIncomingRequest) -> Option<&str> {
    let auth = request.headers().get("authorization")?;
    let prefix = "Bearer ";
    if auth.len() < prefix.len() {
        return None;
    }
    if auth[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&auth[prefix.len()..])
    } else {
        None
    }
}

/// Create a CodeUnauthenticated error.
pub fn unauthenticated(message: impl Into<String>) -> ConnectError {
    ConnectError::unauthenticated(message)
}
```

### Pre-Built Authenticators (foundation_auth Integration)

These bridge foundation_auth's verification infrastructure into the ConnectRPC AuthFunc:

#### JWT Bearer Token Authenticator

```rust
/// Authenticates requests using JWT bearer tokens via foundation_auth's JwtVerifier.
pub struct JwtAuthenticator {
    verifier: JwtVerifier,
}

impl JwtAuthenticator {
    pub fn new(config: JwtVerifierConfig) -> Result<Self, AuthError>;

    /// Create from JWKS URL (fetches keys automatically).
    pub fn from_jwks_url(url: &str) -> Result<Self, AuthError>;

    /// Create from OIDC discovery URL.
    pub fn from_oidc_discovery(issuer_url: &str) -> Result<Self, AuthError>;
}

impl AuthFunc for JwtAuthenticator {
    fn authenticate(&self, request: &SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>> {
        let token = bearer_token(request)
            .ok_or_else(|| unauthenticated("missing bearer token"))?;

        let claims = self.verifier.verify(token)
            .map_err(|e| unauthenticated(format!("invalid token: {e}")))?;

        Ok(Some(Box::new(claims)))  // VerifiedClaims stored in extensions
    }
}
```

#### Session Cookie Authenticator

```rust
/// Authenticates requests using session cookies via foundation_auth's SessionManager.
pub struct SessionAuthenticator {
    session_manager: SessionManager,
    cookie_name: String,
}

impl SessionAuthenticator {
    pub fn new(session_manager: SessionManager, cookie_name: &str) -> Self;
}

impl AuthFunc for SessionAuthenticator {
    fn authenticate(&self, request: &SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>> {
        let token = extract_session_token(request, &self.cookie_name)
            .ok_or_else(|| unauthenticated("missing session cookie"))?;

        let session = self.session_manager.validate(&token)
            .map_err(|e| unauthenticated(format!("invalid session: {e}")))?;

        Ok(Some(Box::new(session)))
    }
}
```

#### Composite Authenticator

```rust
/// Try multiple authenticators in order. First success wins.
pub struct CompositeAuthenticator {
    authenticators: Vec<Arc<dyn AuthFunc>>,
}

impl CompositeAuthenticator {
    pub fn new(authenticators: Vec<Arc<dyn AuthFunc>>) -> Self;
}

impl AuthFunc for CompositeAuthenticator {
    fn authenticate(&self, request: &SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>> {
        let mut last_err = None;
        for auth in &self.authenticators {
            match auth.authenticate(request) {
                Ok(info) => return Ok(info),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| unauthenticated("no authentication method succeeded")))
    }
}
```

#### Per-Procedure Auth

```rust
/// Apply different authentication per procedure.
pub struct PerProcedureAuth {
    /// Default auth (applied if no procedure-specific auth matches).
    default_auth: Option<Arc<dyn AuthFunc>>,
    /// Procedure-specific auth. Key: "/service/method".
    procedure_auth: HashMap<String, Arc<dyn AuthFunc>>,
    /// Procedures that skip authentication entirely.
    unauthenticated_procedures: HashSet<String>,
}

impl PerProcedureAuth {
    pub fn new() -> Self;
    pub fn default_auth(mut self, auth: Arc<dyn AuthFunc>) -> Self;
    pub fn procedure(mut self, procedure: &str, auth: Arc<dyn AuthFunc>) -> Self;
    pub fn unauthenticated(mut self, procedure: &str) -> Self;
}

impl AuthFunc for PerProcedureAuth {
    fn authenticate(&self, request: &SimpleIncomingRequest) -> ConnectResult<Option<Box<dyn Any + Send + Sync>>> {
        let procedure = infer_procedure(request.url());

        if let Some(proc) = &procedure {
            if self.unauthenticated_procedures.contains(proc.as_str()) {
                return Ok(None);
            }
            if let Some(auth) = self.procedure_auth.get(proc.as_str()) {
                return auth.authenticate(request);
            }
        }

        if let Some(default) = &self.default_auth {
            return default.authenticate(request);
        }

        Ok(None) // no auth configured
    }
}
```

### What's Missing from foundation_auth

After comparing connect-go's authn with foundation_auth, these gaps need filling:

1. **`extract_bearer_token` in foundation_auth**: Already exists. Verify it does case-insensitive "Bearer " prefix matching per RFC 9110 Section 11.1.
2. **Protocol-aware error writing**: foundation_auth's middleware returns generic HTTP errors. ConnectRPC needs protocol-aware errors (JSON for Connect, trailers for gRPC). The `ErrorWriter` integration handles this.
3. **JWKS auto-refresh**: foundation_auth has `JwksManager` but verify it supports background key rotation for long-running servers.

### Scope Authorization (Post-Auth)

Authorization runs as a **seam interceptor** (not HTTP middleware), so it uses Decision 04's
**async, future-returning** `Interceptor` fn-types over bytes + metadata — the principal is
read from `ctx.extensions` (metadata the auth middleware inserted), never from the decoded
message. Authorization is **Cedar-policy-based** (R14); `has_scope` is the simple fast path:

```rust
/// ConnectRPC seam interceptor that authorizes via foundation_auth Cedar policies (R14).
pub struct AuthzInterceptor {
    policies: Arc<CedarPolicySet>,   // or `required_scopes: Vec<String>` for the simple gate
}

impl Interceptor for AuthzInterceptor {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let policies = self.policies.clone();
        // UnaryFunc = Arc<dyn Fn(Ctx, UnaryCall) -> BoxFuture<'static, ConnectResult<UnaryReply>>>
        Arc::new(move |ctx, call| {
            let policies = policies.clone();
            let next = next.clone();
            Box::pin(async move {
                let principal = get_auth_info::<AuthContext>(&ctx)
                    .ok_or_else(|| ErrorTrace::new(ConnectError::unauthenticated("not authenticated")))?;
                // Cedar: principal + action(=ctx.spec().procedure) + resource(from ctx) → allow/deny.
                // Fast path for a basic scope gate is `has_scope(principal, &["scope"])` (R8 signature).
                if !policies.is_allowed(principal, &ctx.spec().procedure, &ctx) {
                    return Err(ConnectError::permission_denied("policy denied").into());
                }
                next(ctx, call).await   // owned args; await the wrapped async call
            })
        })
    }
    // wrap_streaming_handler / wrap_streaming_client: same async, future-returning shape (Decision 04).
}
```

## Consequences

- Auth middleware runs at the HTTP level (before decompression), matching connect-go
- `ErrorWriter` ensures auth errors are formatted per the client's protocol
- foundation_auth's JWT, session, and OAuth infrastructure is reused
- `PerProcedureAuth` allows mixing public and authenticated endpoints
- Auth info flows through `RequestContext.extensions` — handlers access it via `get_auth_info::<T>()`
- Scope/permission checks run as interceptors (after deserialization, per-RPC)
- **Cedar policy authorization (decided, R14):** foundation_auth now supports **Cedar
  policies** — we lean on them for authorization. Scope/permission interceptors evaluate a
  Cedar policy set against the request principal (from the authenticator) + the procedure as
  the action + any resource in `RequestContext`, rather than hand-rolled scope checks. This
  gives declarative, centrally-managed per-procedure authz; `has_scope` remains the simple
  fast path for basic scope gates.

## Review-Gap Coverage

**Q9 decision:** fix the small mismatches **upstream in foundation_auth** and reuse them;
bridge only where a sync/async boundary genuinely forces it (R13).

- **R7 — bearer parsing:** make `extract_bearer_token` RFC 9110 case-insensitive in
  foundation_auth and reuse it (don't duplicate an `eq_ignore_ascii_case` helper here).
- **R8 — `has_scope`:** use the real signature `has_scope(&AuthContext, &[&str])`.
- **R9 — `SessionManager` generic:** expose a type-erased session authenticator (or carry
  the `CredentialStore` generic through the middleware) so it can be stored as `dyn`.
- **R10 — `extract_session_token`:** real API is `(cookies: &[&str], cookie_name: &str)`;
  adapt at the call site (pull cookies from `SimpleIncomingRequest`).
- **R11 — protocol string:** use hyphenated `"grpc-web"` to match connect-go.
- **R12 — dependency (settled):** the auth middleware lives in `foundation_connectrpc`
  behind an **`auth` feature flag (off by default)** — no separate crate; enabling it pulls
  `foundation_auth` (and transitively `foundation_db`) only for servers that use it.
- **R13 — JWKS (sync bridge):** `JwtVerifier::from_config` only; `JwksManager` is async.
  The synchronous middleware uses a pre-fetched / cached JWK set refreshed out-of-band,
  rather than fetching inside the request path.
- **T10 — iroh identity:** add a `PublicKeyAuthenticator`; `Peer` carries the Ed25519
  public key when the connection is iroh-based.
- **R14 — Cedar authorization:** evaluate foundation_auth Cedar policies in the
  scope/permission interceptor (principal = authenticated identity, action = procedure,
  resource = request context). See Consequences.
- **R15 — mTLS identity (decided, add):** extract the client certificate chain from
  foundation_netio's TLS session at the connection boundary and surface it via
  `ConnectionContext` → `RequestContext.extensions` as `PeerCertificates`. *What it takes:*
  (a) request client-auth in the rustls `ServerConfig` (optional/required) in `netcap/ssl`;
  (b) read the peer certs off the completed handshake and attach them to the connection
  identity (`Endpoint<I>` / `ConnectionContext`, Decision 04 Q13); (c) a
  `ClientCertAuthenticator` that validates/maps the cert (subject/SAN) to a principal.
  Transport-level, so it composes with token auth rather than replacing it.
- **R16 — OAuth introspection (decided, add):** provide an `IntrospectionAuthenticator` over
  foundation_auth's `IntrospectionClient` for opaque (non-JWT) bearer tokens; like JWKS
  (R13) it runs the network call out-of-band / cached, not inline in the request path.
- **R17 — rate limiting (decided, add):** provide a rate-limiting **seam interceptor** that
  maps rejection to `CodeResourceExhausted` (HTTP 429), keyed on principal/peer; reuse
  foundation_http's limiter where it fits, wrap it as an interceptor for per-procedure limits.
- **R18 — CORS (decided, verify+expand):** reuse foundation_http `CorsMiddleware`, but verify
  it allows the ConnectRPC request headers (`Connect-Protocol-Version`, `Connect-Timeout-Ms`,
  `Connect-Content-Encoding`, `Connect-Accept-Encoding`) and methods; expand its allow-list
  (or add a Connect-aware preset) if it doesn't. Runs at the HTTP layer, outside the seam
  interceptors (Decision 08 ordering).

## Open Questions

*Resolved — the former open questions are now decided items R15–R18 (mTLS, OAuth
introspection, rate limiting, CORS) in Review-Gap Coverage above, and R14 (Cedar) in
Consequences. No open questions remain for this decision.*
