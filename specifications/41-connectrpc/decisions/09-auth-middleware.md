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
    ) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError>;
}

/// Implement AuthFunc for closures.
impl<F> AuthFunc for F
where
    F: Fn(&SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError>
        + Send + Sync + 'static,
{
    fn authenticate(&self, request: &SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError> {
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
/// Retrieve authentication information from the request context.
pub fn get_auth_info<T: 'static>(ctx: &RequestContext) -> Option<&T> {
    ctx.extensions.get::<T>()
}

/// Strip authentication information from context (e.g., before forwarding to untrusted service).
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
        _ if method == SimpleMethod::Post && is_grpc_web_content_type(&content_type) => Some("grpcweb"),
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
    fn authenticate(&self, request: &SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError> {
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
    fn authenticate(&self, request: &SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError> {
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
    fn authenticate(&self, request: &SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError> {
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
    fn authenticate(&self, request: &SimpleIncomingRequest) -> Result<Option<Box<dyn Any + Send + Sync>>, ConnectError> {
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

Foundation_auth provides `has_scope()` and `require_auth()`. These work as interceptors (not HTTP middleware):

```rust
/// ConnectRPC interceptor that checks JWT scopes.
pub struct ScopeInterceptor {
    required_scopes: Vec<String>,
}

impl Interceptor for ScopeInterceptor {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let scopes = self.required_scopes.clone();
        Box::new(move |ctx, req| {
            let claims = get_auth_info::<VerifiedClaims>(ctx)
                .ok_or_else(|| ConnectError::unauthenticated("not authenticated"))?;

            for scope in &scopes {
                if !has_scope(claims, scope) {
                    return Err(ConnectError::permission_denied(
                        format!("missing required scope: {scope}")
                    ));
                }
            }

            next(ctx, req)
        })
    }
    // streaming variants similar
}
```

## Consequences

- Auth middleware runs at the HTTP level (before decompression), matching connect-go
- `ErrorWriter` ensures auth errors are formatted per the client's protocol
- foundation_auth's JWT, session, and OAuth infrastructure is reused
- `PerProcedureAuth` allows mixing public and authenticated endpoints
- Auth info flows through `RequestContext.extensions` — handlers access it via `get_auth_info::<T>()`
- Scope/permission checks run as interceptors (after deserialization, per-RPC)

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
- **R12 — dependency:** put the auth middleware behind a feature flag / in a separate
  crate so `foundation_connectrpc` doesn't unconditionally pull `foundation_auth →
  foundation_db`.
- **R13 — JWKS (sync bridge):** `JwtVerifier::from_config` only; `JwksManager` is async.
  The synchronous middleware uses a pre-fetched / cached JWK set refreshed out-of-band,
  rather than fetching inside the request path.
- **T10 — iroh identity:** add a `PublicKeyAuthenticator`; `Peer` carries the Ed25519
  public key when the connection is iroh-based.

## Open Questions

1. **mTLS identity**: connect-go's authn doesn't handle mTLS (that's transport-level). Foundation_netio supports TLS — can we extract client certificates and pass them through? This would be in `RequestContext.extensions` as `PeerCertificates`.
2. **OAuth token introspection**: For opaque tokens (not JWTs), foundation_auth has `IntrospectionClient`. Should we provide an `IntrospectionAuthenticator` that calls the token introspection endpoint?
3. **Rate limiting**: `CodeResourceExhausted` maps to HTTP 429. Should we provide a rate-limiting middleware, or leave that to foundation_http's existing middleware?
4. **CORS**: connect-go has a separate `cors-go` package. foundation_http has `CorsMiddleware`. Verify the CORS middleware allows ConnectRPC-specific headers (`Connect-Protocol-Version`, `Connect-Timeout-Ms`, `Connect-Content-Encoding`, `Connect-Accept-Encoding`) and methods.
