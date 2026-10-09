//! Auth middleware and guards — request authentication helpers.
//!
//! WHY: Downstream code needs to verify auth status before handling requests.
//!
//! WHAT: `AuthGuard` for checking valid tokens, `require_auth` and
//! `optional_auth` helpers.
//! HOW: Synchronous token validation via `AuthToken`. No framework-specific
//! middleware — provides composable guard functions instead.

use crate::shared::auth_token::AuthToken;

/// Request context carrying authentication info.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authenticated token if available.
    pub token: Option<AuthToken>,
    /// Request path.
    pub path: String,
    /// Client IP address.
    pub ip_address: Option<String>,
    /// User agent string.
    pub user_agent: Option<String>,
    /// Principal identifier (from JWT sub, session user_id).
    pub sub: Option<String>,
    /// RBAC roles from the credential.
    pub roles: Vec<String>,
}

impl AuthContext {
    /// Create a new auth context (unauthenticated).
    #[must_use]
    pub fn new(path: String, ip_address: Option<String>, user_agent: Option<String>) -> Self {
        Self {
            token: None,
            path,
            ip_address,
            user_agent,
            sub: None,
            roles: Vec::new(),
        }
    }

    /// Set the authenticated token.
    #[must_use]
    pub fn with_token(mut self, token: AuthToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Whether the request is authenticated.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.token.as_ref().is_some_and(|t| !t.is_expired())
    }
}

/// Result of an auth guard check.
#[derive(Debug)]
pub enum GuardResult {
    /// Request is authenticated — proceed.
    Authorized(Box<AuthContext>),
    /// Request lacks valid credentials — return 401.
    Unauthorized,
    /// Token is expired — return 401 with refresh hint.
    TokenExpired,
}

/// Require valid authentication for a request.
///
/// Returns `GuardResult::Authorized` if the token is present and not expired,
/// `GuardResult::TokenExpired` if the token exists but is expired, or
/// `GuardResult::Unauthorized` if no token is present.
#[must_use]
pub fn require_auth(ctx: AuthContext) -> GuardResult {
    match ctx.token {
        Some(ref token) => {
            if token.is_expired() {
                GuardResult::TokenExpired
            } else {
                GuardResult::Authorized(Box::new(ctx))
            }
        }
        None => GuardResult::Unauthorized,
    }
}

/// Optional authentication — proceeds whether or not the request is authenticated.
///
/// Useful for endpoints that work differently for authenticated vs anonymous users.
#[must_use]
pub fn optional_auth(mut ctx: AuthContext) -> GuardResult {
    if let Some(ref token) = ctx.token {
        if token.is_expired() {
            ctx.token = None;
        }
    }
    GuardResult::Authorized(Box::new(ctx))
}

/// Check whether a session cookie is present and extract the token value.
///
/// Returns the session token string if the cookie is found.
#[must_use]
pub fn extract_session_token(cookies: &[&str], cookie_name: &str) -> Option<String> {
    for cookie in cookies {
        if let Some(start) = cookie.find(&format!("{cookie_name}=")) {
            let rest = &cookie[start + cookie_name.len() + 1..];
            if let Some(end) = rest.find(';') {
                return Some(rest[..end].to_string());
            }
            return Some(rest.to_string());
        }
    }
    None
}

/// Check whether a bearer token is present in the Authorization header.
///
/// Returns the token value (without the "Bearer " prefix) if found.
/// Per RFC 9110, the scheme matching is case-insensitive.
#[must_use]
pub fn extract_bearer_token(auth_header: Option<&str>) -> Option<String> {
    let header = auth_header?;
    const PREFIX: &[u8] = b"Bearer ";
    let bytes = header.as_bytes();
    if bytes.len() >= PREFIX.len() && bytes[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        header.get(PREFIX.len()..).map(String::from)
    } else {
        None
    }
}

/// Validate that a token has one of the required scopes.
///
/// Returns `true` if the token's scope matches any of the required scopes,
/// or if no scopes are required (empty `required` slice).
#[must_use]
pub fn has_scope(ctx: &AuthContext, required: &[&str]) -> bool {
    if required.is_empty() {
        return true;
    }

    let Some(token) = &ctx.token else {
        return false;
    };

    let token_scope = match token {
        AuthToken::OAuth { scope, .. } => scope.as_deref(),
        _ => None,
    };

    let Some(scope_str) = token_scope else {
        return false;
    };

    let scopes: Vec<&str> = scope_str.split_whitespace().collect();
    required.iter().any(|r| scopes.contains(r))
}

