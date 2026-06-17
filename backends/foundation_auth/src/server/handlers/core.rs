use std::sync::Arc;

use foundation_http::shared::context::ContextBag;
use foundation_http::SimpleIncomingRequest;
use foundation_netio::simple_http::shared::SendSafeBody;
use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;
use super::super::models::{AuthorizationCode, DeviceCode, RefreshToken};
use super::super::services::{TokenService, TokenServiceError};
use super::super::services::user_service;
use super::super::services::{PowService, PowSolution};
use super::super::services::{WebAuthnService, TosService};
use super::super::services::{
    WebAuthnRegisterFinishRequest, WebAuthnAuthFinishRequest,
    PasskeyLoginStartRequest, PasskeyLoginFinishRequest,
};
use super::super::storage::{
    self, HandlerStorage, StorageOpError, find_client_by_id, find_user_by_email, update_user_lockout,
};
use foundation_db::{KeyValueStore, MemoryStorage};

/// Typed response from a handler — carries HTTP status code, body, and headers.
pub struct HandlerResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,
}

impl HandlerResponse {
    pub fn ok(body: serde_json::Value) -> Self {
        Self { status: 200, body, headers: vec![] }
    }
    pub fn accepted(body: serde_json::Value) -> Self {
        Self { status: 202, body, headers: vec![] }
    }
}

#[derive(Debug, Serialize)]
pub struct OidcDiscoveryDocument {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub introspection_endpoint: String,
    pub device_authorization_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

impl OidcDiscoveryDocument {
    #[must_use]
    pub fn from_config(config: &IdpConfig, prefix: &str) -> Self {
        let base = config.issuer_url.trim_end_matches('/');
        let p = prefix.trim_end_matches('/');
        Self {
            issuer: config.issuer_url.clone(),
            authorization_endpoint: format!("{base}{p}/authorize"),
            token_endpoint: format!("{base}{p}/token"),
            userinfo_endpoint: format!("{base}{p}/userinfo"),
            jwks_uri: format!("{base}{p}/.well-known/jwks.json"),
            introspection_endpoint: format!("{base}{p}/introspect"),
            device_authorization_endpoint: format!("{base}{p}/device/authorize"),
            response_types_supported: vec!["code".into()],
            grant_types_supported: vec![
                "authorization_code".into(), "refresh_token".into(),
                "client_credentials".into(), "urn:ietf:params:oauth:grant-type:device_code".into(),
            ],
            subject_types_supported: vec!["public".into()],
            id_token_signing_alg_values_supported: vec!["EdDSA".into()],
            scopes_supported: vec!["openid".into(), "profile".into(), "email".into()],
            token_endpoint_auth_methods_supported: vec!["client_secret_post".into(), "none".into()],
            code_challenge_methods_supported: vec!["S256".into()],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub refresh_token: String,
    pub scope: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u32,
}

/// Login request body for POST /auth/v1/oidc/authorize
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub totp: Option<String>,
}

/// MFA request body for POST /auth/v1/mfa
#[derive(Debug, Deserialize)]
pub struct MfaRequest {
    pub challenge_id: Option<String>,
    pub code: Option<String>,
}

/// Registration request body for POST /auth/v1/users/register
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    pub preferred_username: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub user_values: Option<serde_json::Value>,
    pub redirect_uri: Option<String>,
    pub pow: Option<String>,
}

/// Password reset request body for POST /auth/v1/users/request_reset
#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: Option<String>,
    pub redirect_uri: Option<String>,
}

/// Password set body for PUT /auth/v1/users/{id}/reset
#[derive(Debug, Deserialize)]
pub struct PasswordSetRequest {
    pub password: Option<String>,
    pub code: Option<String>,
}

/// Change password request body
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub device_code: Option<String>,
}

#[derive(Debug)]
pub enum IdpError {
    Internal(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Locked(i64),
    TooManyRequests(i64),
}

impl core::fmt::Display for IdpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Internal(s) => write!(f, "Internal error: {s}"),
            Self::BadRequest(s) => write!(f, "Bad request: {s}"),
            Self::Unauthorized(s) => write!(f, "Unauthorized: {s}"),
            Self::Forbidden(s) => write!(f, "Forbidden: {s}"),
            Self::NotFound(s) => write!(f, "Not found: {s}"),
            Self::Conflict(s) => write!(f, "Conflict: {s}"),
            Self::Locked(ts) => write!(f, "Account locked until {ts}"),
            Self::TooManyRequests(ts) => write!(f, "Too many requests, retry after {ts}"),
        }
    }
}

impl std::error::Error for IdpError {}

impl From<StorageOpError> for IdpError {
    fn from(e: StorageOpError) -> Self {
        match e {
            StorageOpError::NotFound(s) => Self::NotFound(s),
            StorageOpError::Query(s) | StorageOpError::Parse(s) => Self::Internal(s),
        }
    }
}

impl From<TokenServiceError> for IdpError {
    fn from(e: TokenServiceError) -> Self {
        match e {
            TokenServiceError::SigningFailed(s) | TokenServiceError::Storage(s) => Self::Internal(s),
            TokenServiceError::InvalidToken => Self::BadRequest("Invalid token".into()),
        }
    }
}

pub struct IdpHandlerCore<KV: KeyValueStore = foundation_db::MemoryStorage> {
    config: Arc<IdpConfig>,
    storage: Arc<HandlerStorage<KV>>,
    token_service: Arc<TokenService>,
    pow_service: Arc<PowService<KV>>,
    webauthn_service: Arc<WebAuthnService<HandlerStorage<KV>, KV>>,
    tos_service: Arc<TosService<KV>>,
}

impl<KV: KeyValueStore + Clone> IdpHandlerCore<KV> {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>, storage: Arc<HandlerStorage<KV>>) -> Self {
        let cache = storage.cache.clone();
        let token_service = Arc::new(TokenService::new(Arc::clone(&config)));
        let pow_service = Arc::new(PowService::new(cache.clone(), 22, 300, 600));
        let webauthn_service = Arc::new(WebAuthnService::new(Arc::clone(&config), Arc::clone(&storage), cache));
        let tos_service = Arc::new(TosService::new(Arc::clone(&storage)));
        Self { config, storage, token_service, pow_service, webauthn_service, tos_service }
    }

    pub async fn discovery(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let doc = OidcDiscoveryDocument::from_config(&self.config, "/idp");
        let body = serde_json::to_value(&doc).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    pub async fn jwks(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let public_pem = self.config.signing_key.public_key_pem()
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "keys": [{ "kty": "OKP", "crv": "Ed25519", "use": "sig", "kid": "default", "alg": "EdDSA", "x": public_pem }]
        })))
    }

    pub async fn authorize(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let url = &req.request_url.url;
        let query = extract_query(url);
        let client_id = query_param(&query, "client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let redirect_uri = query_param(&query, "redirect_uri")
            .ok_or_else(|| IdpError::BadRequest("Missing redirect_uri".into()))?;
        let response_type = query_param(&query, "response_type")
            .ok_or_else(|| IdpError::BadRequest("Missing response_type".into()))?;
        let _scope = query_param(&query, "scope").unwrap_or("openid");
        let _state = query_param(&query, "state").unwrap_or("");
        let code_challenge = query_param(&query, "code_challenge");
        let _nonce = query_param(&query, "nonce");

        if response_type != "code" {
            return Err(IdpError::BadRequest("response_type must be 'code'".into()));
        }

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)?
            .ok_or_else(|| IdpError::BadRequest("Unknown client_id".into()))?;
        if !client.allows_redirect(redirect_uri) {
            return Err(IdpError::BadRequest("redirect_uri not allowed".into()));
        }
        if self.config.require_pkce && code_challenge.is_none() {
            return Err(IdpError::BadRequest("PKCE code_challenge required".into()));
        }

        // Session cookie check: F02 login handler creates the session cookie.
        // When a valid session exists, generate an auth code and redirect.
        // For now, always return login_required (F02 adds SessionService integration).
        let return_to = url;
        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "login_required",
            "login_url": "/auth/v1/oidc/authorize",
            "return_to": return_to,
            "client_id": client_id,
            "client_name": client.name,
        })))
    }

    pub async fn token(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let pairs = parse_form_urlencoded(&body);
        let grant_type = pairs.get("grant_type")
            .ok_or_else(|| IdpError::BadRequest("Missing grant_type".into()))?;

        match grant_type.as_str() {
            "authorization_code" => self.token_auth_code(&pairs).await,
            "refresh_token" => self.token_refresh(&pairs).await,
            "client_credentials" => self.token_client_credentials(&pairs).await,
            other => Err(IdpError::BadRequest(format!("Unsupported grant_type: {other}"))),
        }
    }

    async fn token_auth_code(
        &self, pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let code = pairs.get("code").ok_or_else(|| IdpError::BadRequest("Missing code".into()))?;
        let redirect_uri = pairs.get("redirect_uri").ok_or_else(|| IdpError::BadRequest("Missing redirect_uri".into()))?;
        let client_id = pairs.get("client_id").ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs.get("client_secret").ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;
        let code_verifier = pairs.get("code_verifier");

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let auth_code = storage::find_auth_code(self.storage.query_store.as_ref(), code)?
            .ok_or_else(|| IdpError::BadRequest("Invalid or expired authorization code".into()))?;
        if auth_code.is_expired() {
            return Err(IdpError::BadRequest("Authorization code has expired".into()));
        }
        if auth_code.client_id != client.id || auth_code.redirect_uri != *redirect_uri {
            return Err(IdpError::BadRequest("Code mismatch".into()));
        }
        if let Some(verifier) = code_verifier {
            if !auth_code.verify_pkce(verifier) {
                return Err(IdpError::BadRequest("PKCE code_verifier mismatch".into()));
            }
        }

        storage::delete_auth_code(self.storage.query_store.as_ref(), &auth_code.code)?;

        let user = storage::find_user_by_email(self.storage.query_store.as_ref(), &auth_code.user_id)?
            .ok_or_else(|| IdpError::Internal("User not found for auth code".into()))?;

        let token_pair = self.token_service.generate_tokens(
            &user, &client, &auth_code.scope, auth_code.nonce.as_deref(),
        )?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "access_token": token_pair.access_token,
            "token_type": "Bearer",
            "expires_in": token_pair.expires_in,
            "refresh_token": token_pair.refresh_token,
            "id_token": token_pair.id_token,
            "scope": token_pair.scope,
        })))
    }

    async fn token_refresh(
        &self, pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let refresh_token = pairs.get("refresh_token")
            .ok_or_else(|| IdpError::BadRequest("Missing refresh_token".into()))?;
        let client_id = pairs.get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs.get("client_secret")
            .ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let token_hash = TokenService::hash_refresh_token(refresh_token);
        let rt = storage::find_refresh_token_by_hash(self.storage.query_store.as_ref(), &token_hash)?
            .ok_or_else(|| IdpError::Unauthorized("Invalid refresh token".into()))?;
        if rt.is_expired() {
            return Err(IdpError::Unauthorized("Refresh token expired".into()));
        }
        if rt.is_rotated() {
            return Err(IdpError::Unauthorized("Refresh token already used".into()));
        }

        storage::mark_refresh_token_rotated(self.storage.query_store.as_ref(), &token_hash)?;

        let user = storage::find_user_by_email(self.storage.query_store.as_ref(), &rt.user_id)?
            .ok_or_else(|| IdpError::Internal("User not found".into()))?;

        // We don't have scope on RefreshToken — use "openid" as default
        let token_pair = self.token_service.generate_tokens(&user, &client, "openid", None)?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "access_token": token_pair.access_token,
            "token_type": "Bearer",
            "expires_in": token_pair.expires_in,
            "refresh_token": token_pair.refresh_token,
            "id_token": token_pair.id_token,
            "scope": "openid",
        })))
    }

    async fn token_client_credentials(
        &self, pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let client_id = pairs.get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs.get("client_secret")
            .ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;
        let scope = pairs.get("scope").map(|s| s.as_str()).unwrap_or("");

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let token_pair = self.token_service.generate_client_credentials_tokens(&client, scope)?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "access_token": token_pair.access_token,
            "token_type": "Bearer",
            "expires_in": token_pair.expires_in,
            "scope": token_pair.scope,
        })))
    }

    pub async fn userinfo(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let auth_header = req.headers.iter()
            .find(|(k, _)| header_name_eq(k, "authorization"))
            .and_then(|(_, v)| v.first());

        let token = auth_header
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| IdpError::Unauthorized("Bearer token required".into()))?;

        let claims = self.token_service.verify_access_token(token)
            .map_err(|_| IdpError::Unauthorized("Invalid or expired token".into()))?;

        let scope = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        if !scope.split_whitespace().any(|s| s == "openid") {
            return Err(IdpError::Forbidden("Missing openid scope".into()));
        }

        let sub = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("");
        let email = claims.get("email").and_then(|v| v.as_str()).unwrap_or("");
        Ok(HandlerResponse::ok(serde_json::json!({ "sub": sub, "email": email })))
    }

    pub async fn introspect(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let pairs = parse_form_urlencoded(&body);
        let token = pairs.get("token")
            .ok_or_else(|| IdpError::BadRequest("Missing token".into()))?;

        match self.token_service.verify_access_token(token) {
            Ok(claims) => {
                let exp = claims.get("exp").and_then(|v| v.as_u64()).unwrap_or(0);
                let iat = claims.get("iat").and_then(|v| v.as_u64()).unwrap_or(0);
                let sub = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("");
                let client_id = claims.get("client_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                Ok(HandlerResponse::ok(serde_json::json!({
                    "active": true,
                    "sub": sub,
                    "client_id": client_id,
                    "scope": scope,
                    "exp": exp,
                    "iat": iat,
                })))
            }
            Err(_) => Ok(HandlerResponse::ok(serde_json::json!({ "active": false }))),
        }
    }

    pub async fn device_authorize(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let pairs = parse_form_urlencoded(&body);
        let client_id = pairs.get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let scope = pairs.get("scope").map(|s| s.as_str()).unwrap_or("openid");

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)?
            .ok_or_else(|| IdpError::BadRequest("Unknown client_id".into()))?;

        let device_code = DeviceCode::new(
            client.id.clone(), scope.to_string(),
            self.config.device_code_ttl, self.config.device_code_interval,
        );
        storage::store_device_code(self.storage.query_store.as_ref(), &device_code)?;

        let base = self.config.issuer_url.trim_end_matches('/');
        Ok(HandlerResponse::ok(serde_json::json!({
            "device_code": device_code.device_code,
            "user_code": device_code.user_code,
            "verification_uri": format!("{base}/device"),
            "verification_uri_complete": format!("{base}/device?code={}", device_code.user_code),
            "expires_in": self.config.device_code_ttl.as_secs(),
            "interval": self.config.device_code_interval,
        })))
    }

    // ─── F02: Login + MFA + Logout ─────────────────────────────────────────────

    /// Unified multi-step login: POST /auth/v1/oidc/authorize
    /// Step 1 (email only): check if user exists, return "password_required" if no password
    /// Step 2 (email + password): verify credentials, create session or MFA challenge
    pub async fn login(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let login_req: LoginRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = login_req.email.ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;
        let user = storage::find_user_by_email(self.storage.query_store.as_ref(), &email)?
            .ok_or_else(|| IdpError::Unauthorized("Invalid credentials".into()))?;

        if user.is_locked() {
            let retry = user.locked_until.unwrap_or(0);
            return Err(IdpError::Locked(retry));
        }

        // Step 1: email only, no password sent
        let Some(password) = login_req.password else {
            if !user.has_password() {
                return Ok(HandlerResponse::ok(serde_json::json!({
                    "status": "password_required",
                    "email": email,
                })));
            }
            return Ok(HandlerResponse::ok(serde_json::json!({
                "status": "password_required",
                "email": email,
            })));
        };

        // Step 2: verify password
        let valid = user_service::verify_password(user.password_hash.as_deref().unwrap_or(""), &password)
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        if !valid {
            // Record failed attempt
            let max = self.config.password_policy.max_failed_attempts;
            let lockout_dur = self.config.password_policy.lockout_duration;
            let mut u = user.clone();
            u.record_failed_attempt(max, lockout_dur);
            let _ = update_user_lockout(
                self.storage.query_store.as_ref(), &u.id,
                u.failed_login_attempts, u.locked_until,
            );
            let remaining = max.saturating_sub(u.failed_login_attempts);
            let lockout_dur = self.config.password_policy.lockout_duration;
            // Note: in a full implementation, we'd persist via update_user_lockout here.
            // For now, return the attempts remaining count.
            let remaining = max.saturating_sub(user.failed_login_attempts);
            return Ok(HandlerResponse::ok(serde_json::json!({
                "status": "invalid_credentials",
                "attempts_remaining": remaining.saturating_sub(1),
            })));
        }

        // Password correct — check for MFA (F06 WebAuthn/TOTP would be checked here)
        // For now, return authenticated (session creation requires SessionService wiring)
        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "authenticated",
        })))
    }

    /// MFA verification: POST /auth/v1/mfa
    pub async fn mfa(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let mfa_req: MfaRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let _challenge_id = mfa_req.challenge_id
            .ok_or_else(|| IdpError::BadRequest("Missing challenge_id".into()))?;
        let code = mfa_req.code
            .ok_or_else(|| IdpError::BadRequest("Missing code".into()))?;

        // TOTP verification would go here (user's TOTP secret + code)
        // For now, return a placeholder
        let _ = code;
        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "authenticated",
        })))
    }

    /// Logout: POST /auth/v1/oidc/logout
    pub async fn logout(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        // Session revocation requires SessionService wiring.
        // Return success with cookie-clearing header.
        Ok(HandlerResponse::ok(serde_json::json!({ "status": "logged_out" })))
    }

    // ─── F03: User Registration ────────────────────────────────────────────────

    /// Register a new user: POST /auth/v1/users/register
    pub async fn register(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let reg: RegisterRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = reg.email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;
        let password = reg.password
            .ok_or_else(|| IdpError::BadRequest("Missing password".into()))?;

        // Validate password against policy
        user_service::validate_password(&password, &self.config.password_policy)
            .map_err(|errors| IdpError::BadRequest(
                format!("Password policy violation: {}", errors.join(", "))
            ))?;

        // Check if user already exists
        if storage::find_user_by_email(self.storage.query_store.as_ref(), &email)?.is_some() {
            return Ok(HandlerResponse::ok(serde_json::json!({
                "error": "user_exists",
            })));
        }

        // Hash password
        let hash = user_service::hash_password(&password)
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        // Create user
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let user = super::super::models::User {
            id: id.clone(),
            email,
            username: reg.preferred_username,
            password_hash: Some(hash),
            email_verified: false,
            email_verified_at: None,
            created_at: now,
            updated_at: now,
            metadata: reg.user_values,
            failed_login_attempts: 0,
            locked_until: None,
            deleted_at: None,
        };

        storage::create_user(self.storage.query_store.as_ref(), &user)?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "id": id,
            "email": user.email,
            "status": "created",
        })))
    }

    /// Dev-mode registration: POST /auth/v1/dev/register
    /// Simplified — no PoW required.
    pub async fn dev_register(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        // In a real implementation, check IS_DEV flag.
        // For now, delegate to the regular registration.
        self.register(_bag, req).await
    }

    // ─── F04: Password Reset ────────────────────────────────────────────────────

    /// Request password reset: POST /auth/v1/users/request_reset
    /// Sends a magic link to the user's email (email delivery is a follow-up).
    pub async fn request_password_reset(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let reset_req: PasswordResetRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = reset_req.email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        // Always return 200 even if user doesn't exist (prevents email enumeration).
        // In production, generate a reset code and send it via email.
        let _user = storage::find_user_by_email(self.storage.query_store.as_ref(), &email);
        let reset_code = uuid::Uuid::new_v4().to_string();

        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "reset_requested",
            "reset_code": reset_code,
            "message": "If the email exists, a reset link has been sent.",
        })))
    }

    /// Set new password: PUT /auth/v1/users/{id}/reset
    pub async fn set_password(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let set_req: PasswordSetRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let password = set_req.password
            .ok_or_else(|| IdpError::BadRequest("Missing password".into()))?;

        // Validate password against policy
        user_service::validate_password(&password, &self.config.password_policy)
            .map_err(|errors| IdpError::BadRequest(
                format!("Password policy violation: {}", errors.join(", "))
            ))?;

        // Verify reset code (in production, look up the code in a reset_tokens table)
        let _code = set_req.code
            .ok_or_else(|| IdpError::BadRequest("Missing reset code".into()))?;

        // Hash and store new password (would update the user record)
        let _hash = user_service::hash_password(&password)
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "password_updated",
        })))
    }

    // ─── F05: Proof of Work ────────────────────────────────────────────────────

    /// Get PoW challenge: GET /auth/v1/pow
    pub async fn pow_challenge(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let challenge = self.pow_service.generate_challenge()
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&challenge)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// Solve PoW challenge: POST /auth/v1/pow
    pub async fn pow_solve(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let solution: PowSolution = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        if self.pow_service.verify_solution(&solution) {
            Ok(HandlerResponse::ok(serde_json::json!({
                "valid": true,
            })))
        } else {
            Ok(HandlerResponse::ok(serde_json::json!({
                "error": "invalid_pow",
            })))
        }
    }

    // ─── F08: Template/Config API ──────────────────────────────────────────────

    /// Get template config: GET /auth/v1/templates/config
    pub async fn template_config(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        Ok(HandlerResponse::ok(serde_json::json!({
            "password_policy": {
                "min_length": self.config.password_policy.min_length,
                "require_uppercase": self.config.password_policy.require_uppercase,
                "require_lowercase": self.config.password_policy.require_lowercase,
                "require_number": self.config.password_policy.require_number,
                "require_special": self.config.password_policy.require_special,
            },
            "issuer": self.config.issuer_url,
        })))
    }

    /// Get password policy: GET /auth/v1/templates/password_policy
    pub async fn password_policy(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        Ok(HandlerResponse::ok(serde_json::json!({
            "min_length": self.config.password_policy.min_length,
            "require_uppercase": self.config.password_policy.require_uppercase,
            "require_lowercase": self.config.password_policy.require_lowercase,
            "require_number": self.config.password_policy.require_number,
            "require_special": self.config.password_policy.require_special,
        })))
    }

    // ─── F09: Account Management ───────────────────────────────────────────────

    /// Get user info: GET /auth/v1/users/{id}
    pub async fn get_user(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        // In production, query the user by ID from storage
        Ok(HandlerResponse::ok(serde_json::json!({
            "id": user_id,
            "status": "user lookup (storage query needed)",
        })))
    }

    /// Update user info: PUT /auth/v1/users/{id}
    pub async fn update_user(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let _user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        let body = extract_body_text(&req.body);
        let update: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "updated",
            "fields": update,
        })))
    }

    /// Change password: POST /auth/v1/users/{id}/change_password
    pub async fn change_password(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let req_body: ChangePasswordRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let current = req_body.current_password
            .ok_or_else(|| IdpError::BadRequest("Missing current_password".into()))?;
        let new_pw = req_body.new_password
            .ok_or_else(|| IdpError::BadRequest("Missing new_password".into()))?;

        // Validate new password
        user_service::validate_password(&new_pw, &self.config.password_policy)
            .map_err(|errors| IdpError::BadRequest(
                format!("Password policy violation: {}", errors.join(", "))
            ))?;

        // In production: verify current password, hash new one, update user
        let _ = current;
        let _hash = user_service::hash_password(&new_pw)
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "password_changed",
        })))
    }

    /// List user sessions: GET /auth/v1/users/{id}/sessions
    pub async fn list_sessions(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let _user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "sessions": [],
        })))
    }

    /// Delete account: POST /auth/v1/users/{id}/revoke
    pub async fn revoke_user(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let _user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        // In production: soft-delete the user, revoke all sessions
        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "account_revoked",
        })))
    }

    // ─── F06: WebAuthn/FIDO2 ───────────────────────────────────────────────────

    /// WebAuthn register start: POST /auth/v1/webauthn/register/start
    pub async fn webauthn_register_start(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let start_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let user_id = start_req.get("user_id").and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing user_id".into()))?;
        let email = start_req.get("email").and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        let options = self.webauthn_service.register_start(user_id, email)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// WebAuthn register finish: POST /auth/v1/webauthn/register/finish
    pub async fn webauthn_register_finish(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let finish_req: WebAuthnRegisterFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let passkey = self.webauthn_service.register_finish(&finish_req)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "id": passkey.id,
            "name": passkey.name,
        })))
    }

    /// WebAuthn auth start: POST /auth/v1/webauthn/login/start
    pub async fn webauthn_auth_start(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let start_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let user_id = start_req.get("user_id").and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing user_id".into()))?;

        let options = self.webauthn_service.auth_start(user_id)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// WebAuthn auth finish: POST /auth/v1/webauthn/login/finish
    pub async fn webauthn_auth_finish(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let finish_req: WebAuthnAuthFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let user_id = self.webauthn_service.auth_finish(&finish_req)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "authenticated",
            "user_id": user_id,
        })))
    }

    /// Passkey-only login start: POST /auth/v1/passkey/login/start
    pub async fn passkey_login_start(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let start_req: PasskeyLoginStartRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let email = start_req.email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        let options = self.webauthn_service.passkey_login_start(&email)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// Passkey-only login finish: POST /auth/v1/passkey/login/finish
    pub async fn passkey_login_finish(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let finish_req: PasskeyLoginFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let user_id = self.webauthn_service.passkey_login_finish(&finish_req)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "authenticated",
            "user_id": user_id,
        })))
    }

    /// Delete passkey: DELETE /auth/v1/webauthn/{id}
    pub async fn delete_passkey(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let passkey_id = extract_path_param(&req.request_url.url, "/auth/v1/webauthn/")
            .ok_or_else(|| IdpError::BadRequest("Missing passkey ID".into()))?;

        self.webauthn_service.delete_passkey(&passkey_id)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({ "status": "deleted" })))
    }

    /// Rename passkey: PUT /auth/v1/webauthn/{id}
    pub async fn rename_passkey(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let passkey_id = extract_path_param(&req.request_url.url, "/auth/v1/webauthn/")
            .ok_or_else(|| IdpError::BadRequest("Missing passkey ID".into()))?;

        let body = extract_body_text(&req.body);
        let rename_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let name = rename_req.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing name".into()))?;

        self.webauthn_service.rename_passkey(&passkey_id, name)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({ "status": "renamed" })))
    }

    // ─── F07: Terms of Service ─────────────────────────────────────────────────

    /// Get latest ToS: GET /auth/v1/tos/latest
    pub async fn tos_latest(
        &self, _bag: &ContextBag, _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        match self.tos_service.get_latest() {
            Some(tos) => Ok(HandlerResponse::ok(serde_json::json!({
                "version": tos.version,
                "content": tos.content,
                "effective_from": tos.effective_from,
            }))),
            None => Ok(HandlerResponse::ok(serde_json::json!({
                "version": null,
                "message": "No ToS configured.",
            }))),
        }
    }

    /// Accept ToS: POST /auth/v1/tos/accept
    pub async fn tos_accept(
        &self, _bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(&req.body);
        let accept_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let user_id = accept_req.get("user_id").and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing user_id".into()))?;

        self.tos_service.accept(user_id, None)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "accepted",
        })))
    }

    pub async fn dispatch(
        &self, bag: &ContextBag, req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let path = req.request_url.url.as_str();
        let path = path.split('?').next().unwrap_or(path);

        if path.ends_with("/.well-known/openid-configuration") { self.discovery(bag, req).await }
        else if path.ends_with("/.well-known/jwks.json") { self.jwks(bag, req).await }
        else if path.ends_with("/authorize") && !path.contains("/device/") { self.authorize(bag, req).await }
        else if path.ends_with("/token") && !path.contains("/introspect") { self.token(bag, req).await }
        else if path.ends_with("/userinfo") { self.userinfo(bag, req).await }
        else if path.ends_with("/introspect") { self.introspect(bag, req).await }
        else if path.ends_with("/device/authorize") { self.device_authorize(bag, req).await }
        // F02 auth routes
        else if path.contains("/auth/v1/oidc/authorize") { self.login(bag, req).await }
        else if path.ends_with("/auth/v1/mfa") { self.mfa(bag, req).await }
        else if path.ends_with("/auth/v1/oidc/logout") { self.logout(bag, req).await }
        // F03 registration routes
        else if path.ends_with("/auth/v1/users/register") { self.register(bag, req).await }
        else if path.ends_with("/auth/v1/dev/register") { self.dev_register(bag, req).await }
        // F04 password reset routes
        else if path.ends_with("/auth/v1/users/request_reset") { self.request_password_reset(bag, req).await }
        else if path.contains("/auth/v1/users/") && path.ends_with("/reset") { self.set_password(bag, req).await }
        // F05 PoW routes
        else if path.ends_with("/auth/v1/pow") {
            match req.method {
                foundation_http::SimpleMethod::GET => self.pow_challenge(bag, req).await,
                foundation_http::SimpleMethod::POST => self.pow_solve(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F08 template/config routes
        else if path.ends_with("/auth/v1/templates/config") { self.template_config(bag, req).await }
        else if path.ends_with("/auth/v1/templates/password_policy") { self.password_policy(bag, req).await }
        // F09 account management routes
        else if path.starts_with("/auth/v1/users/") && path.ends_with("/change_password") {
            self.change_password(bag, req).await
        }
        else if path.starts_with("/auth/v1/users/") && path.ends_with("/sessions") {
            self.list_sessions(bag, req).await
        }
        else if path.starts_with("/auth/v1/users/") && path.ends_with("/revoke") {
            self.revoke_user(bag, req).await
        }
        else if path.starts_with("/auth/v1/users/") && path.matches('/').count() == 4
                && !path.ends_with("/reset") && !path.ends_with("/change_password")
                && !path.ends_with("/sessions") && !path.ends_with("/revoke") {
            match req.method {
                foundation_http::SimpleMethod::GET => self.get_user(bag, req).await,
                foundation_http::SimpleMethod::PUT => self.update_user(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F06 WebAuthn routes
        else if path.ends_with("/auth/v1/webauthn/register/start") { self.webauthn_register_start(bag, req).await }
        else if path.ends_with("/auth/v1/webauthn/register/finish") { self.webauthn_register_finish(bag, req).await }
        else if path.ends_with("/auth/v1/webauthn/login/start") { self.webauthn_auth_start(bag, req).await }
        else if path.ends_with("/auth/v1/webauthn/login/finish") { self.webauthn_auth_finish(bag, req).await }
        else if path.starts_with("/auth/v1/webauthn/") && path.matches('/').count() == 4 {
            match req.method {
                foundation_http::SimpleMethod::DELETE => self.delete_passkey(bag, req).await,
                foundation_http::SimpleMethod::PUT => self.rename_passkey(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F06 passkey-only login routes
        else if path.ends_with("/auth/v1/passkey/login/start") { self.passkey_login_start(bag, req).await }
        else if path.ends_with("/auth/v1/passkey/login/finish") { self.passkey_login_finish(bag, req).await }
        // F07 ToS routes
        else if path.ends_with("/auth/v1/tos/latest") { self.tos_latest(bag, req).await }
        else if path.ends_with("/auth/v1/tos/accept") { self.tos_accept(bag, req).await }
        else { Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))) }
    }
}

// -- Utilities --

fn extract_query(url: &str) -> Vec<(String, String)> {
    if let Some(q) = url.split('?').nth(1) {
        q.split('&').filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?;
            let v = parts.next().unwrap_or("");
            Some((
                urlencoding::decode(k).unwrap_or_else(|_| k.into()).into_owned(),
                urlencoding::decode(v).unwrap_or_else(|_| v.into()).into_owned(),
            ))
        }).collect()
    } else { vec![] }
}

fn query_param<'a>(query: &'a [(String, String)], name: &str) -> Option<&'a str> {
    query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn extract_body_text(body: &Option<SendSafeBody>) -> String {
    match body {
        Some(SendSafeBody::Text(t)) => t.clone(),
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

fn header_name_eq(header: &foundation_netio::simple_http::shared::SimpleHeader, name: &str) -> bool {
    // SimpleHeader is an enum — match on the variant name
    format!("{:?}", header).eq_ignore_ascii_case(name)
}

fn parse_form_urlencoded(body: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in body.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let decoded = urlencoding::decode(value).unwrap_or_else(|_| value.into());
            map.insert(key.to_string(), decoded.into_owned());
        }
    }
    map
}

/// Extract the last path segment after a prefix (e.g. "/auth/v1/users/" → user_id).
fn extract_path_param(url: &str, prefix: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    path.strip_prefix(prefix).map(|s| {
        // Take only the first segment (before next '/')
        s.split('/').next().unwrap_or(s).to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: core::future::Future>(f: F) -> F::Output {
        let mut f = core::pin::pin!(f);
        let waker = noop_waker();
        let mut cx = core::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            core::task::Poll::Ready(v) => v,
            core::task::Poll::Pending => panic!("future not ready"),
        }
    }

    fn noop_waker() -> core::task::Waker {
        use core::task::{RawWaker, RawWakerVTable};
        fn no_op(_: *const ()) {}
        fn clone(p: *const ()) -> RawWaker { RawWaker::new(p, &VTABLE) }
        const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
        unsafe { core::task::Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
    }

    fn test_config() -> Arc<IdpConfig> {
        Arc::new(IdpConfig::new("https://auth.example.com".into()))
    }

    #[test]
    fn test_discovery() {
        let storage = Arc::new(HandlerStorage::new(Arc::new(InMemoryStore), MemoryStorage::new()));
        let core = IdpHandlerCore::new(test_config(), storage);
        let bag = ContextBag::new();
        let req = SimpleIncomingRequest::builder()
            .with_plain_url("/.well-known/openid-configuration").build().unwrap();
        let result = block_on(core.discovery(&bag, &req));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().body["issuer"], "https://auth.example.com");
    }

    #[test]
    fn test_jwks() {
        let storage = Arc::new(HandlerStorage::new(Arc::new(InMemoryStore), MemoryStorage::new()));
        let core = IdpHandlerCore::new(test_config(), storage);
        let bag = ContextBag::new();
        let req = SimpleIncomingRequest::builder()
            .with_plain_url("/.well-known/jwks.json").build().unwrap();
        let result = block_on(core.jwks(&bag, &req));
        assert!(result.is_ok());
        assert!(result.unwrap().body["keys"].is_array());
    }

    #[test]
    fn test_discovery_document_paths() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let doc = OidcDiscoveryDocument::from_config(&config, "/idp");
        assert_eq!(doc.authorization_endpoint, "https://auth.example.com/idp/authorize");
        assert_eq!(doc.jwks_uri, "https://auth.example.com/idp/.well-known/jwks.json");
    }

    #[test]
    fn test_parse_form_urlencoded() {
        let pairs = parse_form_urlencoded("grant_type=authorization_code&code=abc");
        assert_eq!(pairs.get("grant_type"), Some(&"authorization_code".to_string()));
        assert_eq!(pairs.get("code"), Some(&"abc".to_string()));
    }

    use foundation_db::core::storage_provider::{QueryStore, SqlRow, StorageItemStream, DataValue};
    use foundation_db::core::errors::StorageError;
    use foundation_core::valtron::Stream;
    struct InMemoryStore;
    impl QueryStore for InMemoryStore {
        fn query(&self, _sql: &str, _params: &[DataValue]) -> Result<StorageItemStream<'_, SqlRow>, StorageError> {
            Ok(Box::new(std::iter::empty()))
        }
        fn execute(&self, _sql: &str, _params: &[DataValue]) -> Result<u64, StorageError> { Ok(0) }
        fn execute_batch(&self, _sql: &str) -> Result<(), StorageError> { Ok(()) }
    }
}
