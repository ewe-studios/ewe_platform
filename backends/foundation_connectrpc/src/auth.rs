//! Auth module — authentication & authorization for ConnectRPC.
//!
//! WHY: ConnectRPC middleware needs authentication (bearer token extraction, JWT
//! verification, composite auth chains, per-procedure auth configuration, and
//! scope-based authorization interceptors).
//!
//! WHAT: The single normalized auth result ([`AuthInfo`]), the [`AuthFunc`] trait
//! (with a blanket impl for async closures), JWT / composite / per-procedure
//! authenticators, an [`AuthzInterceptor`] for scope checks, bearer token
//! extraction, protocol/procedure inference helpers, and an
//! [`authenticate_request`] HTTP-middleware utility.
//!
//! HOW: The entire module is gated on `cfg(feature = "auth")`. It reuses
//! `foundation_auth` types and follows the Decision 09 / Decision 06 patterns
//! from the connectrpc spec.
//!
//! # Design note: Sync vs Send
//!
//! `AuthFunc::authenticate` receives `&SimpleHeaders` and `&SimpleUrl` rather
//! than `&SimpleIncomingRequest` because the latter is not `Sync`. The returned
//! `BoxFuture<'a, ...>` requires `Send`, which means every captured reference
//! must be `Send` — and `&T` is only `Send` when `T: Sync`. Splitting the
//! incoming request into its Sync parts avoids this restriction.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use foundation_auth::{
    extract_bearer_token as fa_extract_bearer_token, has_scope, AuthContext, AuthToken,
    ConfidentialText,
};
use foundation_auth::{JwtVerifier, JwtVerifierConfig};
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{
    Extensions, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, SimpleUrl,
};

use crate::context::Ctx;
use crate::error::{Code, ConnectError, ConnectResult};
use crate::error_writer::{status_from_code, ErrorWriter};
use crate::interceptor::{
    Interceptor, StreamingClientFunc, StreamingHandlerFunc, UnaryFunc,
};
use crate::protocol::canonicalize_content_type;
use crate::transport::BoxFuture;

// ============================================================================
// B1: AuthInfo — the single normalized auth result (Decision 09 A6)
// ============================================================================

/// The single normalized auth result.
///
/// Authenticators produce it; the authz layer reads it.
#[derive(Clone, Debug)]
pub struct AuthInfo {
    /// Principal identity — subject, scopes, roles.
    pub context: AuthContext,
    /// Raw artifacts: VerifiedClaims, session object, PeerCertificates, ...
    pub artifacts: Extensions,
}

impl AuthInfo {
    /// Create a new auth info with the given context and empty artifacts.
    #[must_use]
    pub fn new(context: AuthContext) -> Self {
        Self {
            context,
            artifacts: Extensions::new(),
        }
    }

    /// Add a typed artifact to the auth info.
    #[must_use]
    pub fn with_artifact<T: Send + Sync + 'static>(mut self, artifact: T) -> Self {
        self.artifacts.insert(artifact);
        self
    }
}

/// Retrieve the [`AuthInfo`] from a per-call [`Ctx`].
///
/// Returns `None` if no auth info has been inserted into the context's
/// extensions (i.e. the request is not authenticated).
#[must_use]
pub fn get_auth_info(ctx: &Ctx) -> Option<&AuthInfo> {
    ctx.request.extensions.get::<AuthInfo>()
}

/// Retrieve a raw artifact by type from the authentication info.
///
/// Returns `None` if no auth info is present or the artifact type is not stored.
#[must_use]
pub fn get_auth_artifact<T: 'static>(ctx: &Ctx) -> Option<&T> {
    get_auth_info(ctx)?.artifacts.get::<T>()
}

// ============================================================================
// B2: AuthFunc trait — async authentication function
// ============================================================================

/// Async authentication function.
///
/// Implementations inspect the incoming HTTP request (headers + URL) and return:
/// - `Ok(Some(info))` — authentication succeeded, an identity was established.
/// - `Ok(None)` — no authentication was attempted (continue without auth).
/// - `Err(trace)` — authentication failed (return the error to the caller).
///
/// The trait has a **blanket impl for async closures**, so you can write:
///
/// ```rust,ignore
/// let auth = |headers: &SimpleHeaders, url: &SimpleUrl| async move {
///     let token = extract_bearer_token_from_headers(headers);
///     // ... verify token ...
///     Ok(Some(auth_info))
/// };
/// ```
pub trait AuthFunc: Send + Sync + 'static {
    /// Authenticate the incoming request.
    ///
    /// Receives the request's headers and URL separately (the full
    /// `SimpleIncomingRequest` is not `Sync`, so passing it would break the
    /// `Send` bound on [`BoxFuture`]).
    fn authenticate<'a>(
        &'a self,
        headers: &'a SimpleHeaders,
        url: &'a SimpleUrl,
    ) -> BoxFuture<'a, ConnectResult<Option<AuthInfo>>>;
}

// Blanket impl for async fns — any closure
// `Fn(&SimpleHeaders, &SimpleUrl) -> Fut` where `Fut: Future<Output =
// ConnectResult<Option<AuthInfo>>>` implements `AuthFunc`.
impl<F, Fut> AuthFunc for F
where
    F: Fn(&SimpleHeaders, &SimpleUrl) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ConnectResult<Option<AuthInfo>>> + Send + 'static,
{
    fn authenticate<'a>(
        &'a self,
        headers: &'a SimpleHeaders,
        url: &'a SimpleUrl,
    ) -> BoxFuture<'a, ConnectResult<Option<AuthInfo>>> {
        Box::pin(self(headers, url))
    }
}

// ============================================================================
// B3: Protocol / procedure inference helpers
// ============================================================================

/// Infer the RPC protocol from headers and method.
///
/// Returns `None` if the request is not a recognized RPC request.
#[must_use]
pub fn infer_protocol(request: &SimpleIncomingRequest) -> Option<&'static str> {
    let ct = request
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .map(String::as_str)
        .unwrap_or("");
    let canonical = canonicalize_content_type(ct);

    if request.method == SimpleMethod::POST {
        if canonical.starts_with("application/grpc-web") {
            return Some("grpc-web");
        }
        if canonical.starts_with("application/grpc") {
            return Some("grpc");
        }
        if canonical.starts_with("application/connect+") || canonical.starts_with("application/") {
            return Some("connect");
        }
    }

    if request.method == SimpleMethod::GET {
        // Check for Connect GET query params: `?connect=v1`
        if request
            .request_url
            .queries
            .as_ref()
            .and_then(|q| q.get("connect"))
            .map_or(false, |v| v == "v1")
        {
            return Some("connect");
        }
    }

    None
}

/// Extract the procedure path from the URL.
///
/// Returns the last 2 non-empty path segments as `"/{service}/{method}"`.
/// Returns `None` if fewer than 2 segments exist.
#[must_use]
pub fn infer_procedure(url: &SimpleUrl) -> Option<String> {
    let path = &url.url;
    // Strip scheme + authority to get just the path portion.
    let path_only = if let Some(pos) = path.find("://") {
        let after_scheme = &path[pos + 3..];
        match after_scheme.find('/') {
            Some(p) => &after_scheme[p..],
            None => "/",
        }
    } else {
        path.as_str()
    };

    let segments: Vec<&str> = path_only
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() >= 2 {
        Some(format!(
            "/{}/{}",
            segments[segments.len() - 2],
            segments[segments.len() - 1]
        ))
    } else {
        None
    }
}

// ============================================================================
// B4: Bearer token helper (ConnectRPC version)
// ============================================================================

/// Extract a bearer token from the `Authorization` header.
///
/// Per RFC 9110, the scheme matching is case-insensitive.
#[must_use]
pub fn bearer_token(request: &SimpleIncomingRequest) -> Option<String> {
    let auth = request.headers.get(&SimpleHeader::AUTHORIZATION)?;
    let header = auth.first()?;
    // Delegates to foundation_auth's extract_bearer_token which uses
    // byte-wise case-insensitive comparison per RFC 9110.
    fa_extract_bearer_token(Some(header.as_str()))
}

/// Internal helper: extract a bearer token from [`SimpleHeaders`] directly.
///
/// Used by authenticator implementations that receive headers + URL separately
/// rather than a full [`SimpleIncomingRequest`].
fn bearer_token_from_headers(headers: &SimpleHeaders) -> Option<String> {
    let auth = headers.get(&SimpleHeader::AUTHORIZATION)?;
    let header = auth.first()?;
    fa_extract_bearer_token(Some(header.as_str()))
}

// ============================================================================
// B5: JWT Authenticator
// ============================================================================

/// Authenticator that validates a JWT bearer token using a [`JwtVerifier`].
///
/// The verifier is **synchronous** (CPU-only, no I/O), so verification is
/// performed inline without spawning a blocking task.
pub struct JwtAuthenticator {
    verifier: JwtVerifier,
}

impl JwtAuthenticator {
    /// Create a new JWT authenticator from a [`JwtVerifierConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`foundation_auth::JwtError`] if the key material cannot be parsed.
    pub fn new(config: JwtVerifierConfig) -> Result<Self, foundation_auth::JwtError> {
        let verifier = JwtVerifier::from_config(config)?;
        Ok(Self { verifier })
    }
}

impl AuthFunc for JwtAuthenticator {
    fn authenticate<'a>(
        &'a self,
        headers: &'a SimpleHeaders,
        url: &'a SimpleUrl,
    ) -> BoxFuture<'a, ConnectResult<Option<AuthInfo>>> {
        // Extract what we need before the async block so we don't hold non-Send
        // references across the await boundary.
        let token = bearer_token_from_headers(headers);
        let path = url.url.clone();

        Box::pin(async move {
            let token = match token {
                Some(t) => t,
                None => return Ok(None),
            };

            match self.verifier.verify(&token) {
                Ok(claims) => {
                    let mut ctx = AuthContext::new(path, None, None);
                    ctx.sub = Some(claims.sub.clone());

                    let auth_token = AuthToken::Jwt {
                        token: ConfidentialText::new(token),
                        expires_at: claims.exp.timestamp() as f64,
                        issuer: Some(claims.iss.clone()),
                        audience: Some(claims.aud.clone()),
                    };
                    ctx.token = Some(auth_token);

                    let mut info = AuthInfo::new(ctx);
                    info.artifacts.insert(claims);
                    Ok(Some(info))
                }
                Err(e) => Err(ConnectError::new(
                    Code::Unauthenticated,
                    format!("invalid token: {e}"),
                )
                .into()),
            }
        })
    }
}

// ============================================================================
// B6: Composite Authenticator
// ============================================================================

/// An authenticator that chains multiple [`AuthFunc`] implementations.
///
/// Each authenticator is tried in order:
/// - `Ok(Some(info))` — success, returned immediately.
/// - `Ok(None)` — no opinion, try the next authenticator.
/// - `Err(trace)` — failure, remembered as `last_err`; subsequent
///   authenticators are still tried. If ALL fail, the last error is returned.
#[derive(Default)]
pub struct CompositeAuthenticator {
    authenticators: Vec<Arc<dyn AuthFunc>>,
}

impl CompositeAuthenticator {
    /// Create an empty composite authenticator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            authenticators: Vec::new(),
        }
    }

    /// Add an authenticator to the chain.
    #[must_use]
    pub fn with(mut self, auth: impl AuthFunc) -> Self {
        self.authenticators.push(Arc::new(auth));
        self
    }

    /// Add an already-`Arc`-wrapped authenticator to the chain.
    #[must_use]
    pub fn with_arc(mut self, auth: Arc<dyn AuthFunc>) -> Self {
        self.authenticators.push(auth);
        self
    }
}

impl AuthFunc for CompositeAuthenticator {
    fn authenticate<'a>(
        &'a self,
        headers: &'a SimpleHeaders,
        url: &'a SimpleUrl,
    ) -> BoxFuture<'a, ConnectResult<Option<AuthInfo>>> {
        // Don't pass the reference to url/headers across await points —
        // extract what we need and proxy via the trait.
        Box::pin(async move {
            let mut last_err = None;
            for auth in &self.authenticators {
                match auth.authenticate(headers, url).await {
                    Ok(Some(info)) => return Ok(Some(info)),
                    Ok(None) => continue,
                    Err(e) => last_err = Some(e),
                }
            }
            match last_err {
                Some(e) => Err(e),
                None => Ok(None),
            }
        })
    }
}

// ============================================================================
// B7: Per-Procedure Auth
// ============================================================================

/// Authenticator that dispatches to different [`AuthFunc`] implementations
/// based on the RPC procedure path.
///
/// Evaluation order:
/// 1. If the procedure is in `unauthenticated_procedures`, return `Ok(None)`.
/// 2. If a procedure-specific authenticator is configured, use it.
/// 3. If a default authenticator is configured, use it.
/// 4. Otherwise return `Ok(None)`.
pub struct PerProcedureAuth {
    /// Authenticator used when no procedure-specific one matches.
    default_auth: Option<Arc<dyn AuthFunc>>,
    /// Procedure-specific authenticators (keyed by `"/{service}/{method}"`).
    procedure_auth: HashMap<String, Arc<dyn AuthFunc>>,
    /// Procedures that do not require authentication.
    unauthenticated_procedures: HashSet<String>,
}

impl PerProcedureAuth {
    /// Create an empty per-procedure auth configurator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_auth: None,
            procedure_auth: HashMap::new(),
            unauthenticated_procedures: HashSet::new(),
        }
    }

    /// Set the default authenticator (used when no procedure-specific one matches).
    #[must_use]
    pub fn with_default(mut self, auth: impl AuthFunc) -> Self {
        self.default_auth = Some(Arc::new(auth));
        self
    }

    /// Register an authenticator for a specific procedure.
    #[must_use]
    pub fn with_procedure(
        mut self,
        procedure: impl Into<String>,
        auth: impl AuthFunc,
    ) -> Self {
        self.procedure_auth.insert(procedure.into(), Arc::new(auth));
        self
    }

    /// Mark a procedure as unauthenticated (no auth check needed).
    #[must_use]
    pub fn without_auth(mut self, procedure: impl Into<String>) -> Self {
        self.unauthenticated_procedures.insert(procedure.into());
        self
    }
}

impl Default for PerProcedureAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthFunc for PerProcedureAuth {
    fn authenticate<'a>(
        &'a self,
        headers: &'a SimpleHeaders,
        url: &'a SimpleUrl,
    ) -> BoxFuture<'a, ConnectResult<Option<AuthInfo>>> {
        Box::pin(async move {
            let procedure = match infer_procedure(url) {
                Some(p) => p,
                None => return Ok(None),
            };

            // 1. Check if explicitly unauthenticated
            if self.unauthenticated_procedures.contains(&procedure) {
                return Ok(None);
            }

            // 2. Check for procedure-specific authenticator
            if let Some(auth) = self.procedure_auth.get(&procedure) {
                return auth.authenticate(headers, url).await;
            }

            // 3. Use default authenticator
            if let Some(ref auth) = self.default_auth {
                return auth.authenticate(headers, url).await;
            }

            // 4. No auth configured for this procedure
            Ok(None)
        })
    }
}

// ============================================================================
// B8: AuthzInterceptor — scope-based authorization at the interceptor level
// ============================================================================

/// Interceptor that enforces scope-based authorization on RPC calls.
///
/// Requires that [`authenticate_request`] (or equivalent middleware) has been
/// called before dispatch so that [`AuthInfo`] is present in the context's
/// extensions.
pub struct AuthzInterceptor {
    required_scopes: Vec<String>,
}

impl AuthzInterceptor {
    /// Create an authz interceptor that checks for the given scopes.
    ///
    /// If `required_scopes` is empty, no scope check is performed (all
    /// authenticated requests pass through).
    #[must_use]
    pub fn new(required_scopes: Vec<String>) -> Self {
        Self { required_scopes }
    }
}

impl Interceptor for AuthzInterceptor {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let scopes = self.required_scopes.clone();
        Arc::new(move |ctx, call| {
            let scopes = scopes.clone();
            let next = next.clone();
            Box::pin(async move {
                if !scopes.is_empty() {
                    let info = get_auth_info(&ctx).ok_or_else(|| {
                        ErrorTrace::from(ConnectError::new(
                            Code::Unauthenticated,
                            "not authenticated",
                        ))
                    })?;
                    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
                    if !has_scope(&info.context, &scope_refs) {
                        return Err(ConnectError::new(
                            Code::PermissionDenied,
                            "insufficient scope",
                        )
                        .into());
                    }
                }
                next(ctx, call).await
            })
        })
    }

    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        let scopes = self.required_scopes.clone();
        Arc::new(move |ctx, spec| {
            let scopes = scopes.clone();
            let next = next.clone();
            Box::pin(async move {
                if !scopes.is_empty() {
                    let info = get_auth_info(&ctx).ok_or_else(|| {
                        ErrorTrace::from(ConnectError::new(
                            Code::Unauthenticated,
                            "not authenticated",
                        ))
                    })?;
                    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
                    if !has_scope(&info.context, &scope_refs) {
                        return Err(ConnectError::new(
                            Code::PermissionDenied,
                            "insufficient scope",
                        )
                        .into());
                    }
                }
                next(ctx, spec).await
            })
        })
    }

    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        let scopes = self.required_scopes.clone();
        Arc::new(move |ctx, call| {
            let scopes = scopes.clone();
            let next = next.clone();
            Box::pin(async move {
                if !scopes.is_empty() {
                    let info = get_auth_info(&ctx).ok_or_else(|| {
                        ErrorTrace::from(ConnectError::new(
                            Code::Unauthenticated,
                            "not authenticated",
                        ))
                    })?;
                    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
                    if !has_scope(&info.context, &scope_refs) {
                        return Err(ConnectError::new(
                            Code::PermissionDenied,
                            "insufficient scope",
                        )
                        .into());
                    }
                }
                next(ctx, call).await
            })
        })
    }
}

// ============================================================================
// B9: authenticate_request — HTTP middleware helper
// ============================================================================

/// Authenticate a request before dispatch.
///
/// Calls the given [`AuthFunc`] against the request. On success, inserts the
/// [`AuthInfo`] into the request's extensions (so it is available in the
/// dispatcher and interceptors). On failure, renders an error response using
/// the [`ErrorWriter`].
///
/// # Errors
///
/// Returns `Err(SimpleOutgoingResponse)` if authentication fails. The response
/// is fully formed in the correct protocol format.
pub async fn authenticate_request(
    auth: &dyn AuthFunc,
    request: &mut SimpleIncomingRequest,
    error_writer: &ErrorWriter,
) -> Result<Option<AuthInfo>, SimpleOutgoingResponse> {
    // Extract Sync parts of the request before calling authenticate, since
    // SimpleIncomingRequest is not Sync and we need a Send future.
    let result = {
        let headers: &SimpleHeaders = &request.headers;
        let url: &SimpleUrl = &request.request_url;
        auth.authenticate(headers, url).await
    };

    match result {
        Ok(Some(info)) => {
            let exts = request.extensions.get_or_insert_with(Extensions::new);
            exts.insert(info.clone());
            Ok(Some(info))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            let status = status_from_code(e.current_context().code());
            let mut resp = SimpleOutgoingResponse::builder()
                .with_status(status)
                .build()
                .expect("valid response");
            // Best-effort error body rendering; if it fails, return the bare
            // status response anyway.
            let _ = error_writer.write(&mut resp, request, &e);
            Err(resp)
        }
    }
}
