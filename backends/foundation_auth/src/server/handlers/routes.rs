//! The `IdP` route table: each (method, path) pair maps to one [`Route`].
//!
//! The table is a `static`, so it is built once. A path matches only as a
//! whole: there is no `ends_with` or `contains` guessing and no order to
//! get wrong. A pattern is a list of segments; `{id}` matches exactly one
//! segment. The OIDC routes (`/authorize`, `/token`, ...) live under a
//! configurable prefix (`IdpServer::register_routes(app, prefix)`), so their
//! patterns start with `*`, which matches any prefix (zero or more whole
//! segments). The `/auth/v1/...` routes are fixed and match only from the
//! root.

use foundation_http::SimpleMethod;

/// The handler a request goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Discovery,
    Jwks,
    /// `GET */authorize`: the social login start when the query has
    /// `provider`, else the OIDC authorization endpoint.
    Authorize,
    Callback,
    Token,
    Userinfo,
    Introspect,
    DeviceAuthorize,
    Login,
    Mfa,
    TotpEnrollStart,
    TotpEnrollConfirm,
    Logout,
    Register,
    DevRegister,
    RequestPasswordReset,
    SetPassword,
    PowChallenge,
    PowSolve,
    TemplateConfig,
    PasswordPolicy,
    GetUser,
    UpdateUser,
    ChangePassword,
    ListSessions,
    RevokeUser,
    WebauthnRegisterStart,
    WebauthnRegisterFinish,
    WebauthnAuthStart,
    WebauthnAuthFinish,
    DeletePasskey,
    RenamePasskey,
    PasskeyLoginStart,
    PasskeyLoginFinish,
    TosLatest,
    TosAccept,
}

/// The methods the table knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Get,
    Post,
    Put,
    Delete,
}

impl Verb {
    fn of(method: &SimpleMethod) -> Option<Self> {
        match method {
            SimpleMethod::GET => Some(Self::Get),
            SimpleMethod::POST => Some(Self::Post),
            SimpleMethod::PUT => Some(Self::Put),
            SimpleMethod::DELETE => Some(Self::Delete),
            _ => None,
        }
    }
}

/// One row of the table.
pub struct RouteDef {
    pub verb: Verb,
    pub pattern: &'static str,
    pub route: Route,
}

const fn def(verb: Verb, pattern: &'static str, route: Route) -> RouteDef {
    RouteDef {
        verb,
        pattern,
        route,
    }
}

use Verb::{Delete, Get, Post, Put};

/// Every route the `IdP` serves. Keep it in step with
/// `IdpServer::register_routes`.
pub static ROUTES: &[RouteDef] = &[
    // OIDC (under the configurable prefix)
    def(Get, "*/.well-known/openid-configuration", Route::Discovery),
    def(Get, "*/.well-known/jwks.json", Route::Jwks),
    def(Get, "*/authorize", Route::Authorize),
    def(Post, "*/token", Route::Token),
    def(Get, "*/userinfo", Route::Userinfo),
    def(Post, "*/introspect", Route::Introspect),
    def(Post, "*/device/authorize", Route::DeviceAuthorize),
    def(Get, "*/callback", Route::Callback),
    // Login, MFA, logout
    def(Post, "/auth/v1/oidc/authorize", Route::Login),
    def(Post, "/auth/v1/mfa", Route::Mfa),
    def(
        Post,
        "/auth/v1/mfa/totp/enroll/start",
        Route::TotpEnrollStart,
    ),
    def(
        Post,
        "/auth/v1/mfa/totp/enroll/confirm",
        Route::TotpEnrollConfirm,
    ),
    def(Post, "/auth/v1/oidc/logout", Route::Logout),
    // Registration and password reset
    def(Post, "/auth/v1/users/register", Route::Register),
    def(Post, "/auth/v1/dev/register", Route::DevRegister),
    def(
        Post,
        "/auth/v1/users/request_reset",
        Route::RequestPasswordReset,
    ),
    def(Put, "/auth/v1/users/{id}/reset", Route::SetPassword),
    // Proof of work
    def(Get, "/auth/v1/pow", Route::PowChallenge),
    def(Post, "/auth/v1/pow", Route::PowSolve),
    // Templates and config
    def(Get, "/auth/v1/templates/config", Route::TemplateConfig),
    def(
        Get,
        "/auth/v1/templates/password_policy",
        Route::PasswordPolicy,
    ),
    // Account management
    def(Get, "/auth/v1/users/{id}", Route::GetUser),
    def(Put, "/auth/v1/users/{id}", Route::UpdateUser),
    def(
        Post,
        "/auth/v1/users/{id}/change_password",
        Route::ChangePassword,
    ),
    def(Get, "/auth/v1/users/{id}/sessions", Route::ListSessions),
    def(Post, "/auth/v1/users/{id}/revoke", Route::RevokeUser),
    // WebAuthn and passkeys
    def(
        Post,
        "/auth/v1/webauthn/register/start",
        Route::WebauthnRegisterStart,
    ),
    def(
        Post,
        "/auth/v1/webauthn/register/finish",
        Route::WebauthnRegisterFinish,
    ),
    def(
        Post,
        "/auth/v1/webauthn/login/start",
        Route::WebauthnAuthStart,
    ),
    def(
        Post,
        "/auth/v1/webauthn/login/finish",
        Route::WebauthnAuthFinish,
    ),
    def(Delete, "/auth/v1/webauthn/{id}", Route::DeletePasskey),
    def(Put, "/auth/v1/webauthn/{id}", Route::RenamePasskey),
    def(
        Post,
        "/auth/v1/passkey/login/start",
        Route::PasskeyLoginStart,
    ),
    def(
        Post,
        "/auth/v1/passkey/login/finish",
        Route::PasskeyLoginFinish,
    ),
    // Terms of Service
    def(Get, "/auth/v1/tos/latest", Route::TosLatest),
    def(Post, "/auth/v1/tos/accept", Route::TosAccept),
];

/// True if `path` matches `pattern` as a whole (see the module docs).
fn matches(pattern: &str, path: &str) -> bool {
    let path_segments: Vec<&str> = path.split('/').collect();
    if let Some(rest) = pattern.strip_prefix('*') {
        // `rest` starts with `/`: its segments (after that empty first one)
        // must be the tail of the path. What comes before them is the prefix:
        // the root, then whole non-empty segments.
        let tail: Vec<&str> = rest.split('/').skip(1).collect();
        let Some(split) = path_segments.len().checked_sub(tail.len()) else {
            return false;
        };
        let prefix = &path_segments[..split];
        let prefix_ok = prefix.first() == Some(&"") && prefix.iter().skip(1).all(|s| !s.is_empty());
        prefix_ok && segments_match(&tail, &path_segments[split..])
    } else {
        let pattern_segments: Vec<&str> = pattern.split('/').collect();
        segments_match(&pattern_segments, &path_segments)
    }
}

fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    pattern.len() == path.len()
        && pattern
            .iter()
            .zip(path)
            .all(|(p, s)| if *p == "{id}" { !s.is_empty() } else { p == s })
}

/// The route for a request, or `None` when no route has this method and path.
/// `path` is the path without the query string.
#[must_use]
pub fn route_for(method: &SimpleMethod, path: &str) -> Option<Route> {
    let verb = Verb::of(method)?;
    ROUTES
        .iter()
        .find(|def| def.verb == verb && matches(def.pattern, path))
        .map(|def| def.route)
}
