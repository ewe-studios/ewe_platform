use std::sync::Arc;

use foundation_http::shared::context::ContextBag;
use foundation_http::SimpleIncomingRequest;
use foundation_netio::shared::http::SendSafeBody;
use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;
use super::super::models::{DeviceCode, PendingLogin};
use super::super::services::user_service;
use super::super::services::{
    MfaService, SessionService, TosAcceptError, TosService, WebAuthnService,
};
use super::super::services::{
    PasskeyLoginFinishRequest, PasskeyLoginStartRequest, WebAuthnAuthFinishRequest,
    WebAuthnRegisterFinishRequest,
};
use super::super::services::{PowService, PowSolution};
use super::super::services::{TokenService, TokenServiceError};
use super::super::storage::{
    self, find_client_by_id, update_user_lockout, HandlerStorage, StorageOpError,
};
use foundation_db::core::storage_provider::{DataValue, QueryStore, SqlRow};
use foundation_db::{KeyValueStore, MemoryStorage};
use futures_lite::StreamExt;

use crate::shared::auth_session::{UpstreamAuthSession, UpstreamAuthSessionStore};
use crate::shared::provider::{ProviderMapping, ProviderType, UpstreamProvider};
use crate::shared::upstream_client::UpstreamOidcClient;

/// Typed response from a handler — carries HTTP status code, body, and headers.
#[derive(Debug)]
pub struct HandlerResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,
}

impl HandlerResponse {
    #[must_use]
    pub fn ok(body: serde_json::Value) -> Self {
        Self {
            status: 200,
            body,
            headers: vec![],
        }
    }
    #[must_use]
    pub fn accepted(body: serde_json::Value) -> Self {
        Self {
            status: 202,
            body,
            headers: vec![],
        }
    }
    /// Build a 302 redirect. The body is empty; `Location` is set from `url`.
    #[must_use]
    pub fn redirect(url: &str) -> Self {
        Self {
            status: 302,
            body: serde_json::json!({}),
            headers: vec![("Location".to_string(), url.to_string())],
        }
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
                "authorization_code".into(),
                "refresh_token".into(),
                "client_credentials".into(),
                "urn:ietf:params:oauth:grant-type:device_code".into(),
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
    /// Ignored. A user with a TOTP secret gets a `mfa_required` challenge and
    /// sends the code to `POST /auth/v1/mfa`. A code in the login body would
    /// let a caller guess codes with no per-challenge limit.
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

/// Password reset request body for POST `/auth/v1/users/request_reset`
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
    /// The request body is larger than `IdpConfig::max_request_body_bytes`.
    PayloadTooLarge(usize),
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
            Self::PayloadTooLarge(limit) => write!(f, "Request body larger than {limit} bytes"),
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
            TokenServiceError::SigningFailed(s) | TokenServiceError::Storage(s) => {
                Self::Internal(s)
            }
            TokenServiceError::InvalidToken => Self::BadRequest("Invalid token".into()),
        }
    }
}

pub struct IdpHandlerCore<KV: KeyValueStore + Clone = MemoryStorage> {
    config: Arc<IdpConfig>,
    storage: Arc<HandlerStorage<KV>>,
    token_service: Arc<TokenService>,
    pow_service: Arc<PowService<KV>>,
    webauthn_service: Arc<WebAuthnService<HandlerStorage<KV>, KV>>,
    tos_service: Arc<TosService<KV>>,
    mfa_service: Arc<MfaService<KV>>,
    session_service: Arc<SessionService>,
    /// In-memory upstream auth session store (F011). When present, social login
    /// routes are enabled.
    session_store: Option<Arc<crate::shared::auth_session::memory::MemoryAuthSessionStore>>,
}

impl<KV: KeyValueStore + Clone> IdpHandlerCore<KV> {
    /// Build the handlers over a config and storage.
    ///
    /// # Panics
    ///
    /// Never in practice: it panics only if building the default in-memory
    /// session service fails, and a 32-byte random key is always valid.
    #[must_use]
    pub fn new(config: Arc<IdpConfig>, storage: Arc<HandlerStorage<KV>>) -> Self {
        let cache = storage.cache.clone();
        let token_service = Arc::new(TokenService::new(Arc::clone(&config)));
        let pow_service = Arc::new(PowService::new(cache.clone(), 22, 300, 600));
        let webauthn_service = Arc::new(WebAuthnService::new(
            Arc::clone(&config),
            Arc::clone(&storage),
            cache,
        ));
        let tos_service = Arc::new(TosService::new(Arc::clone(&storage)));
        let mfa_service = Arc::new(
            MfaService::new(Arc::clone(&storage), config.totp_keys.clone())
                .with_algorithm(config.totp_algorithm),
        );
        // A fresh in-memory session service. `IdpServer` and `ServeAdapter`
        // replace it with one shared by all routes (`with_session_service`).
        let session_service = Arc::new(
            SessionService::in_memory_with(crate::shared::session::SessionConfig {
                secure_cookies: config.secure_cookies,
                ..Default::default()
            })
            .expect("a 32-byte signing key is a valid session key"),
        );
        Self {
            config,
            storage,
            token_service,
            pow_service,
            webauthn_service,
            tos_service,
            mfa_service,
            session_service,
            session_store: None,
        }
    }

    /// Use this session service (shared by all routes) to create and read
    /// login sessions.
    #[must_use]
    pub fn with_session_service(mut self, sessions: Arc<SessionService>) -> Self {
        self.session_service = sessions;
        self
    }

    /// Enable social login by attaching a session store.
    #[must_use]
    pub fn with_social_login(
        mut self,
        store: Arc<crate::shared::auth_session::memory::MemoryAuthSessionStore>,
    ) -> Self {
        self.session_store = Some(store);
        self
    }

    /// The server configuration.
    #[must_use]
    pub fn config(&self) -> &IdpConfig {
        &self.config
    }

    /// Discovery.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn discovery(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let doc = OidcDiscoveryDocument::from_config(&self.config, "/idp");
        let body = serde_json::to_value(&doc).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// Jwks.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn jwks(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let public_pem = self
            .config
            .signing_key
            .public_key_pem()
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "keys": [{ "kty": "OKP", "crv": "Ed25519", "use": "sig", "kid": "default", "alg": "EdDSA", "x": public_pem }]
        })))
    }

    /// Authorize.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn authorize(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
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

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
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

    /// Token.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn token(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let pairs = parse_form_urlencoded(&body);
        let grant_type = pairs
            .get("grant_type")
            .ok_or_else(|| IdpError::BadRequest("Missing grant_type".into()))?;

        match grant_type.as_str() {
            "authorization_code" => self.token_auth_code(&pairs).await,
            "refresh_token" => self.token_refresh(&pairs).await,
            "client_credentials" => self.token_client_credentials(&pairs).await,
            other => Err(IdpError::BadRequest(format!(
                "Unsupported grant_type: {other}"
            ))),
        }
    }

    async fn token_auth_code(
        &self,
        pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let code = pairs
            .get("code")
            .ok_or_else(|| IdpError::BadRequest("Missing code".into()))?;
        let redirect_uri = pairs
            .get("redirect_uri")
            .ok_or_else(|| IdpError::BadRequest("Missing redirect_uri".into()))?;
        let client_id = pairs
            .get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs
            .get("client_secret")
            .ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;
        let code_verifier = pairs.get("code_verifier");

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let auth_code = storage::find_auth_code(self.storage.query_store.as_ref(), code)
            .await?
            .ok_or_else(|| IdpError::BadRequest("Invalid or expired authorization code".into()))?;
        if auth_code.is_expired() {
            return Err(IdpError::BadRequest(
                "Authorization code has expired".into(),
            ));
        }
        if auth_code.client_id != client.id || auth_code.redirect_uri != *redirect_uri {
            return Err(IdpError::BadRequest("Code mismatch".into()));
        }
        if let Some(verifier) = code_verifier {
            if !auth_code.verify_pkce(verifier) {
                return Err(IdpError::BadRequest("PKCE code_verifier mismatch".into()));
            }
        }

        storage::delete_auth_code(self.storage.query_store.as_ref(), &auth_code.code).await?;

        let user = storage::find_user_by_id(self.storage.query_store.as_ref(), &auth_code.user_id)
            .await?
            .ok_or_else(|| IdpError::Internal("User not found for auth code".into()))?;

        let token_pair = self.token_service.generate_tokens(
            &user,
            &client,
            &auth_code.scope,
            auth_code.nonce.as_deref(),
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
        &self,
        pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let refresh_token = pairs
            .get("refresh_token")
            .ok_or_else(|| IdpError::BadRequest("Missing refresh_token".into()))?;
        let client_id = pairs
            .get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs
            .get("client_secret")
            .ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let token_hash = TokenService::hash_refresh_token(refresh_token);
        let rt =
            storage::find_refresh_token_by_hash(self.storage.query_store.as_ref(), &token_hash)
                .await?
                .ok_or_else(|| IdpError::Unauthorized("Invalid refresh token".into()))?;
        if rt.is_expired() {
            return Err(IdpError::Unauthorized("Refresh token expired".into()));
        }
        if rt.is_rotated() {
            return Err(IdpError::Unauthorized("Refresh token already used".into()));
        }

        storage::mark_refresh_token_rotated(self.storage.query_store.as_ref(), &token_hash).await?;

        let user = storage::find_user_by_email(self.storage.query_store.as_ref(), &rt.user_id)
            .await?
            .ok_or_else(|| IdpError::Internal("User not found".into()))?;

        // We don't have scope on RefreshToken — use "openid" as default
        let token_pair = self
            .token_service
            .generate_tokens(&user, &client, "openid", None)?;

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
        &self,
        pairs: &std::collections::HashMap<String, String>,
    ) -> Result<HandlerResponse, IdpError> {
        let client_id = pairs
            .get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let client_secret = pairs
            .get("client_secret")
            .ok_or_else(|| IdpError::BadRequest("Missing client_secret".into()))?;
        let scope = pairs.get("scope").map_or("", std::string::String::as_str);

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Invalid client".into()))?;
        if !client.verify_secret(client_secret) {
            return Err(IdpError::Unauthorized("Invalid client_secret".into()));
        }

        let token_pair = self
            .token_service
            .generate_client_credentials_tokens(&client, scope)?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "access_token": token_pair.access_token,
            "token_type": "Bearer",
            "expires_in": token_pair.expires_in,
            "scope": token_pair.scope,
        })))
    }

    /// Userinfo.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn userinfo(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let auth_header = req
            .headers
            .iter()
            .find(|(k, _)| header_name_eq(k, "authorization"))
            .and_then(|(_, v)| v.first());

        let token = auth_header
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| IdpError::Unauthorized("Bearer token required".into()))?;

        let claims = self
            .token_service
            .verify_access_token(token)
            .map_err(|_| IdpError::Unauthorized("Invalid or expired token".into()))?;

        let scope = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        if !scope.split_whitespace().any(|s| s == "openid") {
            return Err(IdpError::Forbidden("Missing openid scope".into()));
        }

        let sub = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("");
        let email = claims.get("email").and_then(|v| v.as_str()).unwrap_or("");
        Ok(HandlerResponse::ok(
            serde_json::json!({ "sub": sub, "email": email }),
        ))
    }

    /// Introspect.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn introspect(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let pairs = parse_form_urlencoded(&body);
        let token = pairs
            .get("token")
            .ok_or_else(|| IdpError::BadRequest("Missing token".into()))?;

        match self.token_service.verify_access_token(token) {
            Ok(claims) => {
                let exp = claims
                    .get("exp")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let iat = claims
                    .get("iat")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let sub = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("");
                let client_id = claims
                    .get("client_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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

    /// Device authorize.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn device_authorize(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let pairs = parse_form_urlencoded(&body);
        let client_id = pairs
            .get("client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing client_id".into()))?;
        let scope = pairs
            .get("scope")
            .map_or("openid", std::string::String::as_str);

        let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
            .ok_or_else(|| IdpError::BadRequest("Unknown client_id".into()))?;

        let device_code = DeviceCode::new(
            client.id.clone(),
            scope.to_string(),
            self.config.device_code_ttl,
            self.config.device_code_interval,
        );
        storage::store_device_code(self.storage.query_store.as_ref(), &device_code).await?;

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
    /// Step 1 (email only): check if user exists, return "`password_required`" if no password
    /// Step 2 (email + password): verify credentials, create session or MFA challenge
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn login(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let login_req: LoginRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = login_req
            .email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;
        let user = storage::find_user_by_email(self.storage.query_store.as_ref(), &email)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Invalid credentials".into()))?;

        if user.is_locked() {
            let retry = user.locked_until.unwrap_or(0);
            return Err(IdpError::Locked(retry));
        }

        // Step 1: email only, no password sent
        let Some(password) = login_req.password else {
            return Ok(HandlerResponse::ok(serde_json::json!({
                "status": "password_required",
                "email": email,
            })));
        };

        // The client and redirect URI are checked before any session or
        // `ToS` code can carry them to the end of the login.
        if login_req.client_id.is_none() && login_req.redirect_uri.is_some() {
            return Err(IdpError::BadRequest(
                "redirect_uri needs a client_id".into(),
            ));
        }
        if let Some(client_id) = login_req.client_id.as_deref() {
            let client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
                .await?
                .ok_or_else(|| IdpError::BadRequest("Unknown client_id".into()))?;
            if let Some(redirect_uri) = login_req.redirect_uri.as_deref() {
                if !client.allows_redirect(redirect_uri) {
                    return Err(IdpError::BadRequest("redirect_uri not allowed".into()));
                }
            }
        }

        // Step 2: verify password
        let valid =
            user_service::verify_password(user.password_hash.as_deref().unwrap_or(""), &password)
                .map_err(|e| IdpError::Internal(e.to_string()))?;

        if !valid {
            // Record failed attempt
            let max = self.config.password_policy.max_failed_attempts;
            let lockout_dur = self.config.password_policy.lockout_duration;
            let mut u = user.clone();
            u.record_failed_attempt(max, lockout_dur);
            update_user_lockout(
                self.storage.query_store.as_ref(),
                &u.id,
                u.failed_login_attempts,
                u.locked_until,
            )
            .await?;
            let remaining = max.saturating_sub(u.failed_login_attempts);
            return Ok(HandlerResponse::ok(serde_json::json!({
                "status": "invalid_credentials",
                "attempts_remaining": remaining,
            })));
        }

        let pending = PendingLogin {
            auth_method: "password".into(),
            client_id: login_req.client_id,
            redirect_uri: login_req.redirect_uri,
            state: None,
            nonce: None,
        };

        // MFA: a user with a TOTP secret must prove it before the ToS gate
        // and the session.
        if self.mfa_service.has_totp(&user.id).await? {
            let challenge_id = self
                .mfa_service
                .create_challenge(&user.id, pending, self.config.mfa_challenge_ttl)
                .await?;
            return Ok(HandlerResponse::ok(serde_json::json!({
                "status": "mfa_required",
                "challenge_id": challenge_id,
                "type": "totp",
            })));
        }

        self.clear_failed_attempts(&user).await?;
        self.complete_login(&user.id, pending, req).await
    }

    /// Count a failed second-factor attempt toward the account lockout.
    async fn record_failed_attempt(
        &self,
        user: &super::super::models::User,
    ) -> Result<(), IdpError> {
        let mut u = user.clone();
        u.record_failed_attempt(
            self.config.password_policy.max_failed_attempts,
            self.config.password_policy.lockout_duration,
        );
        update_user_lockout(
            self.storage.query_store.as_ref(),
            &u.id,
            u.failed_login_attempts,
            u.locked_until,
        )
        .await?;
        Ok(())
    }

    /// The signed-in user of the request, for the TOTP enrollment routes. A
    /// locked account gets 423: failed codes count toward the lockout.
    async fn enrollment_user(
        &self,
        req: &SimpleIncomingRequest,
    ) -> Result<super::super::models::User, IdpError> {
        let user_id = self
            .authenticated_user(req)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Authentication required".into()))?;
        let user = storage::find_user_by_id(self.storage.query_store.as_ref(), &user_id)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Authentication required".into()))?;
        if user.is_locked() {
            return Err(IdpError::Locked(user.locked_until.unwrap_or(0)));
        }
        Ok(user)
    }

    /// A full successful login ends the run of failed attempts: reset the
    /// count and any lockout.
    async fn clear_failed_attempts(
        &self,
        user: &super::super::models::User,
    ) -> Result<(), IdpError> {
        if user.failed_login_attempts > 0 || user.locked_until.is_some() {
            update_user_lockout(self.storage.query_store.as_ref(), &user.id, 0, None).await?;
        }
        Ok(())
    }

    /// MFA verification: POST /auth/v1/mfa
    ///
    /// The body holds the `challenge_id` from the `mfa_required` login answer
    /// and the one-time `code`. The user comes from the challenge, never from
    /// the body. A challenge allows one attempt; a wrong code counts as a
    /// failed login attempt for the user (account lockout applies).
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn mfa(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let mfa_req: MfaRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let challenge_id = mfa_req
            .challenge_id
            .ok_or_else(|| IdpError::BadRequest("Missing challenge_id".into()))?;
        let code = mfa_req
            .code
            .ok_or_else(|| IdpError::BadRequest("Missing code".into()))?;

        let invalid = || IdpError::Unauthorized("Invalid or expired MFA challenge or code".into());

        let record = self
            .mfa_service
            .take_challenge(&challenge_id)
            .await?
            .ok_or_else(invalid)?;
        let user = storage::find_user_by_id(self.storage.query_store.as_ref(), &record.user_id)
            .await?
            .ok_or_else(invalid)?;
        if user.is_locked() {
            return Err(IdpError::Locked(user.locked_until.unwrap_or(0)));
        }
        // Checks the code and spends its time step (a code works once).
        if !self.mfa_service.verify_totp_code(&user.id, &code).await? {
            self.record_failed_attempt(&user).await?;
            return Err(invalid());
        }

        self.clear_failed_attempts(&user).await?;
        let mut pending = record.pending_login;
        pending.auth_method = "password+totp".into();
        self.complete_login(&user.id, pending, req).await
    }

    /// Finish a proven authentication. Every path that proves a user (password,
    /// MFA, passkey, WebAuthn, social login) ends here.
    ///
    /// If the user has not accepted the active `ToS` version, no session or
    /// authorization code is made. The answer is
    /// `206 {"status": "tos_required", "tos_await_code", "tos_version"}` and the
    /// client posts the code to `POST /auth/v1/tos/accept`. Otherwise the
    /// session is created (`202` with `Set-Cookie` and `Location`), or, for
    /// social login, the authorization code is made (`302`).
    async fn complete_login(
        &self,
        user_id: &str,
        pending: PendingLogin,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        if let Some(challenge) = self
            .tos_service
            .gate(user_id, pending.clone(), self.config.tos_code_ttl)
            .await?
        {
            return Ok(HandlerResponse {
                status: 206,
                body: serde_json::json!({
                    "status": "tos_required",
                    "tos_await_code": challenge.code,
                    "tos_version": challenge.tos_version,
                }),
                headers: vec![],
            });
        }

        if pending.auth_method == "social" {
            // The code is bound to the client that asked for the login; the
            // token endpoint checks it.
            let auth_code = crate::server::models::AuthorizationCode::new(
                user_id.to_string(),
                pending.client_id.clone().unwrap_or_default(),
                pending.redirect_uri.clone().unwrap_or_default(),
                None,
                "openid profile email".to_string(),
                pending.nonce.clone(),
                self.config.auth_code_ttl,
            );
            storage::store_auth_code(self.storage.query_store.as_ref(), &auth_code)
                .await
                .map_err(|e| IdpError::Internal(format!("store auth code: {e}")))?;
            let app_state = pending.state.as_deref().unwrap_or("");
            let redirect_uri = pending.redirect_uri.as_deref().unwrap_or("/");
            let redirect_to = format!("{redirect_uri}?code={}&state={app_state}", auth_code.code);
            return Ok(HandlerResponse::redirect(&redirect_to));
        }

        let user_agent = request_header(req, "user-agent");
        let (_session, cookies) = self
            .session_service
            .create_session(user_id, None, user_agent.as_deref())
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let mut headers: Vec<(String, String)> = cookies
            .iter()
            .map(|c| ("Set-Cookie".to_string(), c.to_set_cookie_string()))
            .collect();
        headers.push((
            "Location".to_string(),
            pending.redirect_uri.unwrap_or_else(|| "/".to_string()),
        ));
        Ok(HandlerResponse {
            status: 202,
            body: serde_json::json!({
                "status": "authenticated",
                "user_id": user_id,
            }),
            headers,
        })
    }

    /// The user of the request's session cookie or bearer access token, if
    /// either is valid.
    async fn authenticated_user(
        &self,
        req: &SimpleIncomingRequest,
    ) -> Result<Option<String>, IdpError> {
        let cookie_name = self.session_service.token_cookie_name();
        if let Some(token) = request_cookie(req, cookie_name) {
            if let Some(session) = self
                .session_service
                .get_session(&token)
                .await
                .map_err(|e| IdpError::Internal(e.to_string()))?
            {
                return Ok(Some(session.user_id));
            }
        }
        if let Some(header) = request_header(req, "authorization") {
            if let Some(token) = crate::shared::middleware::extract_bearer_token(Some(&header)) {
                if let Ok(claims) = self.token_service.verify_access_token(&token) {
                    if let Some(sub) = claims.get("sub").and_then(|v| v.as_str()) {
                        return Ok(Some(sub.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Start TOTP enrollment: POST /auth/v1/mfa/totp/enroll/start
    ///
    /// Needs a session (cookie or bearer). Makes a secret and returns it with
    /// an `otpauth_uri` for an authenticator app. The secret is stored
    /// encrypted and is not active until `enroll/confirm` gets a first code. A
    /// user who already has a TOTP secret must send the `current_code` in the
    /// body, so a stolen session cannot replace the second factor.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid, not authenticated, or a
    /// storage or service call fails.
    pub async fn totp_enroll_start(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user = self.enrollment_user(req).await?;
        let user_id = user.id.clone();
        if self.config.totp_keys.active().is_none() {
            return Err(IdpError::Internal(
                "TOTP enrollment is not configured (no encryption key)".into(),
            ));
        }
        let body = extract_body_text(req.body.as_ref());
        let body: serde_json::Value = if body.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&body)
                .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?
        };

        if self.mfa_service.has_totp(&user_id).await? {
            let code = body
                .get("current_code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    IdpError::Unauthorized("current_code is required to replace the secret".into())
                })?;
            if !self.mfa_service.verify_totp_code(&user_id, code).await? {
                self.record_failed_attempt(&user).await?;
                return Err(IdpError::Unauthorized("Invalid current_code".into()));
            }
        }

        let enrollment = self
            .mfa_service
            .start_enrollment(&user_id, self.config.totp_enrollment_ttl)
            .await?;
        let issuer = self.config.issuer_url.trim_end_matches('/');
        let uri = format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm={}&digits=6&period=30",
            urlencoding::encode(issuer),
            urlencoding::encode(&user.email),
            enrollment.secret_base32,
            urlencoding::encode(issuer),
            enrollment.algorithm.uri_name(),
        );
        Ok(HandlerResponse::ok(serde_json::json!({
            "secret": enrollment.secret_base32,
            "otpauth_uri": uri,
            "expires_in": self.config.totp_enrollment_ttl.as_secs(),
        })))
    }

    /// Confirm TOTP enrollment: POST /auth/v1/mfa/totp/enroll/confirm
    ///
    /// Needs a session. The body is `{"code": "..."}`, a first code made from
    /// the secret that `enroll/start` returned. One start allows one attempt.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid, not authenticated, or a
    /// storage or service call fails.
    pub async fn totp_enroll_confirm(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user = self.enrollment_user(req).await?;
        let body = extract_body_text(req.body.as_ref());
        let body: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let code = body
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing code".into()))?;
        if !self.mfa_service.confirm_enrollment(&user.id, code).await? {
            self.record_failed_attempt(&user).await?;
            return Err(IdpError::Unauthorized(
                "Invalid code or no enrollment in progress".into(),
            ));
        }
        Ok(HandlerResponse::ok(
            serde_json::json!({ "status": "enrolled" }),
        ))
    }

    /// Logout: POST /auth/v1/oidc/logout
    ///
    /// Revokes the session of the session cookie and clears the session
    /// cookies. With no cookie, or one that is not a live session, it still
    /// answers 200 (the client is logged out either way). A bearer access
    /// token is a signed token with no server-side record, so logout cannot
    /// revoke it; it ends when it expires.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if a storage or service call fails.
    pub async fn logout(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let cookie_name = self.session_service.token_cookie_name();
        if let Some(token) = request_cookie(req, cookie_name) {
            if let Some(session) = self
                .session_service
                .get_session(&token)
                .await
                .map_err(|e| IdpError::Internal(e.to_string()))?
            {
                self.session_service
                    .revoke_session(&session.id)
                    .await
                    .map_err(|e| IdpError::Internal(e.to_string()))?;
            }
        }
        let headers = self
            .session_service
            .clearing_cookies()
            .iter()
            .map(|c| ("Set-Cookie".to_string(), c.to_set_cookie_string()))
            .collect();
        Ok(HandlerResponse {
            status: 200,
            body: serde_json::json!({ "status": "logged_out" }),
            headers,
        })
    }

    // ─── F03: User Registration ────────────────────────────────────────────────

    /// Register a new user: POST /auth/v1/users/register
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn register(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let reg: RegisterRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = reg
            .email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;
        let password = reg
            .password
            .ok_or_else(|| IdpError::BadRequest("Missing password".into()))?;

        // Validate password against policy
        user_service::validate_password(&password, &self.config.password_policy).map_err(
            |errors| {
                IdpError::BadRequest(format!("Password policy violation: {}", errors.join(", ")))
            },
        )?;

        // Check if user already exists
        if storage::find_user_by_email(self.storage.query_store.as_ref(), &email)
            .await?
            .is_some()
        {
            return Err(IdpError::Conflict("user_exists".into()));
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

        storage::create_user(self.storage.query_store.as_ref(), &user).await?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "id": id,
            "email": user.email,
            "status": "created",
        })))
    }

    /// Dev-mode registration: `POST /auth/v1/dev/register`.
    /// Simplified — no `PoW` required.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError::NotFound`] (404) if [`IdpConfig::dev_mode`] is off.
    /// Otherwise returns [`IdpError`] if the request is not valid or a storage
    /// or service call fails.
    pub async fn dev_register(
        &self,
        bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        if !self.config.dev_mode {
            return Err(IdpError::NotFound("not found".into()));
        }
        self.register(bag, req).await
    }

    // ─── F04: Password Reset ────────────────────────────────────────────────────

    /// Request password reset: POST `/auth/v1/users/request_reset`
    /// Sends a magic link to the user's email (email delivery is a follow-up).
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn request_password_reset(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let reset_req: PasswordResetRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let email = reset_req
            .email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        // Always return 202 whether or not the user exists (prevents email
        // enumeration); a storage failure is still an error. Email delivery of
        // the reset code is a follow-up.
        let _user = storage::find_user_by_email(self.storage.query_store.as_ref(), &email).await?;
        let reset_code = uuid::Uuid::new_v4().to_string();

        Ok(HandlerResponse::accepted(serde_json::json!({
            "status": "reset_requested",
            "reset_code": reset_code,
            "message": "If the email exists, a reset link has been sent.",
        })))
    }

    /// Set new password: PUT /auth/v1/users/{id}/reset
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn set_password(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let set_req: PasswordSetRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let password = set_req
            .password
            .ok_or_else(|| IdpError::BadRequest("Missing password".into()))?;

        // Validate password against policy
        user_service::validate_password(&password, &self.config.password_policy).map_err(
            |errors| {
                IdpError::BadRequest(format!("Password policy violation: {}", errors.join(", ")))
            },
        )?;

        // Verify reset code (in production, look up the code in a reset_tokens table)
        let _code = set_req
            .code
            .ok_or_else(|| IdpError::BadRequest("Missing reset code".into()))?;

        // Hash and store new password (would update the user record)
        let _hash = user_service::hash_password(&password)
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "password_updated",
        })))
    }

    // ─── F05: Proof of Work ────────────────────────────────────────────────────

    /// Get `PoW` challenge: GET /auth/v1/pow
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn pow_challenge(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let challenge = self
            .pow_service
            .generate_challenge()
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body =
            serde_json::to_value(&challenge).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// Solve `PoW` challenge: POST /auth/v1/pow
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn pow_solve(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let solution: PowSolution = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        if self.pow_service.verify_solution(&solution).await {
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
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn template_config(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
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

    /// Get password policy: GET `/auth/v1/templates/password_policy`
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn password_policy(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
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
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn get_user(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        let user = storage::find_user_by_id(self.storage.query_store.as_ref(), &user_id)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?
            .ok_or_else(|| IdpError::NotFound("User not found".into()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "id": user.id,
            "email": user.email,
            "username": user.username,
            "email_verified": user.email_verified,
            "created_at": user.created_at,
            "updated_at": user.updated_at,
        })))
    }

    /// Update user info: PUT /auth/v1/users/{id}
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn update_user(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        let body = extract_body_text(req.body.as_ref());
        let update: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let username = update.get("username").and_then(|v| v.as_str());
        storage::update_user_profile(self.storage.query_store.as_ref(), &user_id, username)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "updated",
            "username": username,
        })))
    }

    /// Change password: POST `/auth/v1/users/{id}/change_password`
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn change_password(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        let body = extract_body_text(req.body.as_ref());
        let req_body: ChangePasswordRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let current = req_body
            .current_password
            .ok_or_else(|| IdpError::BadRequest("Missing current_password".into()))?;
        let new_pw = req_body
            .new_password
            .ok_or_else(|| IdpError::BadRequest("Missing new_password".into()))?;

        // Fetch user to verify current password
        let user = storage::find_user_by_id(self.storage.query_store.as_ref(), &user_id)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?
            .ok_or_else(|| IdpError::NotFound("User not found".into()))?;

        let current_hash = user
            .password_hash
            .ok_or_else(|| IdpError::BadRequest("User has no password set".into()))?;

        // Verify current password
        let valid = user_service::verify_password(&current_hash, &current)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        if !valid {
            return Err(IdpError::BadRequest("Current password is incorrect".into()));
        }

        // Validate new password
        user_service::validate_password(&new_pw, &self.config.password_policy).map_err(
            |errors| {
                IdpError::BadRequest(format!("Password policy violation: {}", errors.join(", ")))
            },
        )?;

        // Hash and store new password
        let new_hash =
            user_service::hash_password(&new_pw).map_err(|e| IdpError::Internal(e.to_string()))?;

        storage::update_user_password(self.storage.query_store.as_ref(), &user_id, &new_hash)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "password_changed",
        })))
    }

    /// List user sessions: GET /auth/v1/users/{id}/sessions
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub fn list_sessions(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let _user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        // Sessions are managed by SessionService (CredentialStorage) — not SQL
        Ok(HandlerResponse::ok(serde_json::json!({
            "sessions": [],
            "note": "session listing requires SessionStore integration",
        })))
    }

    /// Delete account: POST /auth/v1/users/{id}/revoke
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn revoke_user(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let user_id = extract_path_param(&req.request_url.url, "/auth/v1/users/")
            .ok_or_else(|| IdpError::BadRequest("Missing user ID".into()))?;

        storage::soft_delete_user(self.storage.query_store.as_ref(), &user_id)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(HandlerResponse::ok(serde_json::json!({
            "status": "account_revoked",
        })))
    }

    // ─── F06: WebAuthn/FIDO2 ───────────────────────────────────────────────────

    /// WebAuthn register start: POST /auth/v1/webauthn/register/start
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn webauthn_register_start(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let start_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let user_id = start_req
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing user_id".into()))?;
        let email = start_req
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        let options = self
            .webauthn_service
            .register_start(user_id, email)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// WebAuthn register finish: POST /auth/v1/webauthn/register/finish
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn webauthn_register_finish(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let finish_req: WebAuthnRegisterFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let passkey = self
            .webauthn_service
            .register_finish(&finish_req)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(serde_json::json!({
            "id": passkey.id,
            "name": passkey.name,
        })))
    }

    /// WebAuthn auth start: POST /auth/v1/webauthn/login/start
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn webauthn_auth_start(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let start_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let user_id = start_req
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing user_id".into()))?;

        let options = self
            .webauthn_service
            .auth_start(user_id)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// WebAuthn auth finish: POST /auth/v1/webauthn/login/finish
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn webauthn_auth_finish(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let finish_req: WebAuthnAuthFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let user_id = self
            .webauthn_service
            .auth_finish(&finish_req)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        self.complete_login(&user_id, passkey_pending("webauthn"), req)
            .await
    }

    /// Passkey-only login start: POST /auth/v1/passkey/login/start
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn passkey_login_start(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let start_req: PasskeyLoginStartRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let email = start_req
            .email
            .ok_or_else(|| IdpError::BadRequest("Missing email".into()))?;

        let options = self
            .webauthn_service
            .passkey_login_start(&email)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        let body = serde_json::to_value(&options).map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(body))
    }

    /// Passkey-only login finish: POST /auth/v1/passkey/login/finish
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn passkey_login_finish(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let finish_req: PasskeyLoginFinishRequest = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;

        let user_id = self
            .webauthn_service
            .passkey_login_finish(&finish_req)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        self.complete_login(&user_id, passkey_pending("passkey"), req)
            .await
    }

    /// Delete passkey: DELETE /auth/v1/webauthn/{id}
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn delete_passkey(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let passkey_id = extract_path_param(&req.request_url.url, "/auth/v1/webauthn/")
            .ok_or_else(|| IdpError::BadRequest("Missing passkey ID".into()))?;

        self.webauthn_service
            .delete_passkey(&passkey_id)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(
            serde_json::json!({ "status": "deleted" }),
        ))
    }

    /// Rename passkey: PUT /auth/v1/webauthn/{id}
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn rename_passkey(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let passkey_id = extract_path_param(&req.request_url.url, "/auth/v1/webauthn/")
            .ok_or_else(|| IdpError::BadRequest("Missing passkey ID".into()))?;

        let body = extract_body_text(req.body.as_ref());
        let rename_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        let name = rename_req
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IdpError::BadRequest("Missing name".into()))?;

        self.webauthn_service
            .rename_passkey(&passkey_id, name)
            .await
            .map_err(|e| IdpError::Internal(e.to_string()))?;
        Ok(HandlerResponse::ok(
            serde_json::json!({ "status": "renamed" }),
        ))
    }

    // ─── F07: Terms of Service ─────────────────────────────────────────────────

    /// Get latest `ToS`: GET /auth/v1/tos/latest
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn tos_latest(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        match self.tos_service.get_latest().await? {
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

    /// Accept `ToS`: POST /auth/v1/tos/accept
    ///
    /// Two modes:
    ///
    /// - `{"tos_await_code": "...", "tos_version": "..."}`: the code from a
    ///   `206 tos_required` login answer. The acceptance is recorded for the
    ///   user the code was issued to, then the pending login is finished (the
    ///   same answer as a normal login: `202` with `Set-Cookie` and `Location`).
    /// - `{"tos_version": "..."}` with a valid session cookie or bearer access
    ///   token: the acceptance is recorded for the session's user.
    ///
    /// A `user_id` in the body is ignored in both modes. With neither a code
    /// nor a session the answer is 401.
    ///
    /// Errors: an unknown, used or expired code gives 401 `invalid_tos_code`
    /// (one answer for all three). A version that is not the code's version or
    /// not the active version gives 409 `tos_version_mismatch` with the active
    /// version.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn tos_accept(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let body = extract_body_text(req.body.as_ref());
        let accept_req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| IdpError::BadRequest(format!("Invalid JSON: {e}")))?;
        // `user_id` in the body is deliberately never read.
        let version = accept_req.get("tos_version").and_then(|v| v.as_str());
        let code = accept_req.get("tos_await_code").and_then(|v| v.as_str());

        if let Some(code) = code {
            let version =
                version.ok_or_else(|| IdpError::BadRequest("Missing tos_version".into()))?;
            // Deleted by `take_code` before anything else happens.
            let Some(record) = self.tos_service.take_code(code).await? else {
                return Ok(HandlerResponse {
                    status: 401,
                    body: serde_json::json!({
                        "error": "invalid_tos_code",
                        "error_description": "The ToS code is not valid.",
                    }),
                    headers: vec![],
                });
            };
            if record.tos_version != version {
                return self.tos_version_mismatch().await;
            }
            match self
                .tos_service
                .accept_version(&record.user_id, &record.tos_version, None)
                .await
            {
                Ok(()) => {}
                Err(TosAcceptError::VersionMismatch { .. } | TosAcceptError::NoActiveVersion) => {
                    return self.tos_version_mismatch().await;
                }
                Err(TosAcceptError::Storage(e)) => return Err(e.into()),
            }
            return self
                .complete_login(&record.user_id, record.pending_login, req)
                .await;
        }

        let user_id = self
            .authenticated_user(req)
            .await?
            .ok_or_else(|| IdpError::Unauthorized("Authentication required".into()))?;
        let version = version.ok_or_else(|| IdpError::BadRequest("Missing tos_version".into()))?;
        match self
            .tos_service
            .accept_version(&user_id, version, None)
            .await
        {
            Ok(()) => Ok(HandlerResponse::ok(serde_json::json!({
                "status": "accepted",
                "tos_version": version,
            }))),
            Err(TosAcceptError::VersionMismatch { .. } | TosAcceptError::NoActiveVersion) => {
                self.tos_version_mismatch().await
            }
            Err(TosAcceptError::Storage(e)) => Err(e.into()),
        }
    }

    /// The 409 answer for a wrong or stale `ToS` version; it names the active
    /// version.
    async fn tos_version_mismatch(&self) -> Result<HandlerResponse, IdpError> {
        let active = self.tos_service.get_latest().await?.map(|v| v.version);
        Ok(HandlerResponse {
            status: 409,
            body: serde_json::json!({
                "error": "tos_version_mismatch",
                "error_description": "The ToS version is not the active version.",
                "active_version": active,
            }),
            headers: vec![],
        })
    }

    // ── F011 Social login handlers ────────────────────────────────────────

    /// `GET /authorize?provider=google&state=...&redirect_uri=...`
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn social_authorize(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let Some(session_store) = self.session_store.as_ref() else {
            return Err(IdpError::BadRequest("social login not configured".into()));
        };
        let query = extract_query(&req.request_url.url);

        let provider_id = query_param(&query, "provider")
            .ok_or_else(|| IdpError::BadRequest("Missing 'provider' query param".into()))?;
        let app_state = query_param(&query, "state").unwrap_or_default().to_string();
        let app_redirect_uri = query_param(&query, "redirect_uri")
            .ok_or_else(|| IdpError::BadRequest("Missing 'redirect_uri' query param".into()))?;

        // The authorization code is sent to this URI at the end of the login:
        // it must be one the client registered (exact match), like in `login`.
        let client_id = query_param(&query, "client_id")
            .ok_or_else(|| IdpError::BadRequest("Missing 'client_id' query param".into()))?;
        let app_client = find_client_by_id(self.storage.query_store.as_ref(), client_id)
            .await?
            .ok_or_else(|| IdpError::BadRequest("Unknown client_id".into()))?;
        if !app_client.allows_redirect(app_redirect_uri) {
            return Err(IdpError::BadRequest("redirect_uri not allowed".into()));
        }

        let provider = lookup_upstream_provider(self.storage.query_store.as_ref(), provider_id)
            .await
            .map_err(|e| IdpError::Internal(format!("provider lookup: {e}")))?
            .ok_or_else(|| IdpError::NotFound(format!("unknown provider: {provider_id}")))?;

        if !provider.is_active {
            return Err(IdpError::Forbidden(format!(
                "provider {provider_id} is not active"
            )));
        }

        let mut client = UpstreamOidcClient::new(provider.clone(), app_redirect_uri.to_string());
        if provider.discovery_url.is_some() {
            let mut dc = crate::shared::discovery::DiscoveryClient::default();
            client
                .discover_and_configure(&mut dc)
                .await
                .map_err(|e| IdpError::BadRequest(format!("discovery: {e}")))?;
        } else {
            client
                .configure()
                .map_err(|e| IdpError::BadRequest(format!("oauth config: {e}")))?;
        }

        let upstream_state = crate::shared::oauth::OAuthManager::generate_state();
        let (auth_url, pkce) = client
            .authorize_url(&upstream_state)
            .map_err(|e| IdpError::Internal(format!("authorize url: {e}")))?;

        let nonce = if provider.provider_type == ProviderType::Oidc {
            Some(crate::shared::oauth::OAuthManager::generate_nonce())
        } else {
            None
        };

        session_store
            .insert(
                &UpstreamAuthSession::with_app_state(
                    upstream_state,
                    provider_id.to_string(),
                    Some(pkce.code_verifier),
                    nonce,
                    Some(app_redirect_uri.to_string()),
                    600,
                    if app_state.is_empty() {
                        None
                    } else {
                        Some(app_state)
                    },
                )
                .with_client_id(client_id),
            )
            .map_err(|e| IdpError::Internal(format!("session store: {e}")))?;

        Ok(HandlerResponse::redirect(&auth_url))
    }

    /// `GET /callback?provider=google&code=...&state=...`
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn social_callback(
        &self,
        _bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let Some(session_store) = self.session_store.as_ref() else {
            return Err(IdpError::BadRequest("social login not configured".into()));
        };
        let query = extract_query(&req.request_url.url);

        let code = query_param(&query, "code")
            .ok_or_else(|| IdpError::BadRequest("Missing 'code' query param".into()))?;
        let state = query_param(&query, "state")
            .ok_or_else(|| IdpError::BadRequest("Missing 'state' query param".into()))?;

        let session = session_store
            .take(state)
            .map_err(|e| IdpError::BadRequest(format!("session: {e}")))?;

        let provider =
            lookup_upstream_provider(self.storage.query_store.as_ref(), &session.provider_id)
                .await
                .map_err(|e| IdpError::Internal(format!("provider lookup: {e}")))?
                .ok_or_else(|| {
                    IdpError::NotFound(format!("unknown provider: {}", session.provider_id))
                })?;

        // Build configured client, exchange code, fetch userinfo.
        let mut client = UpstreamOidcClient::new(provider.clone(), "unused".to_string());
        if provider.discovery_url.is_some() {
            let mut dc = crate::shared::discovery::DiscoveryClient::default();
            client
                .discover_and_configure(&mut dc)
                .await
                .map_err(|e| IdpError::BadRequest(format!("discovery: {e}")))?;
        } else {
            client
                .configure()
                .map_err(|e| IdpError::BadRequest(format!("oauth config: {e}")))?;
        }

        let token = client
            .exchange_code(code, session.code_verifier.as_deref(), None)
            .await
            .map_err(|e| IdpError::BadRequest(format!("token exchange: {e}")))?;

        let profile = client
            .fetch_userinfo(&token.access_token)
            .await
            .map_err(|e| IdpError::BadRequest(format!("userinfo: {e}")))?;

        let user_id = provision_from_upstream_profile(self.storage.query_store.as_ref(), &profile)
            .await
            .map_err(|e| IdpError::Internal(format!("provisioning: {e}")))?;

        let pending = PendingLogin {
            auth_method: "social".into(),
            client_id: session.client_id.clone(),
            redirect_uri: session.redirect_to.clone(),
            state: session.app_state.clone(),
            nonce: session.nonce.clone(),
        };
        self.complete_login(&user_id, pending, req).await
    }

    /// The MFA routes (`/auth/v1/mfa` and TOTP enrollment); `None` for any
    /// other path.
    async fn dispatch_mfa(
        &self,
        bag: &ContextBag,
        req: &SimpleIncomingRequest,
        path: &str,
    ) -> Option<Result<HandlerResponse, IdpError>> {
        if path.ends_with("/auth/v1/mfa/totp/enroll/start") {
            Some(self.totp_enroll_start(bag, req).await)
        } else if path.ends_with("/auth/v1/mfa/totp/enroll/confirm") {
            Some(self.totp_enroll_confirm(bag, req).await)
        } else if path.ends_with("/auth/v1/mfa") {
            Some(self.mfa(bag, req).await)
        } else {
            None
        }
    }

    /// Dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError`] if the request is not valid or a storage or service call fails.
    pub async fn dispatch(
        &self,
        bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<HandlerResponse, IdpError> {
        let path = req.request_url.url.as_str();
        let path = path.split('?').next().unwrap_or(path);

        if path.ends_with("/.well-known/openid-configuration") {
            self.discovery(bag, req)
        } else if path.ends_with("/.well-known/jwks.json") {
            self.jwks(bag, req)
        }
        // F011: social login — /authorize?provider=X dispatches here
        else if path.ends_with("/authorize") && has_query_param("provider", req) {
            self.social_authorize(bag, req).await
        } else if path.ends_with("/callback") && has_query_param("provider", req) {
            self.social_callback(bag, req).await
        }
        // F02 unified login: POST /auth/v1/oidc/authorize. It must be matched
        // before the generic `/authorize` branch below, which would take it.
        else if path.contains("/auth/v1/oidc/authorize") {
            self.login(bag, req).await
        } else if path.ends_with("/authorize") && !path.contains("/device/") {
            self.authorize(bag, req).await
        } else if path.ends_with("/token") && !path.contains("/introspect") {
            self.token(bag, req).await
        } else if path.ends_with("/userinfo") {
            self.userinfo(bag, req)
        } else if path.ends_with("/introspect") {
            self.introspect(bag, req)
        } else if path.ends_with("/device/authorize") {
            self.device_authorize(bag, req).await
        }
        // F02 auth routes
        else if let Some(result) = self.dispatch_mfa(bag, req, path).await {
            result
        } else if path.ends_with("/auth/v1/oidc/logout") {
            self.logout(bag, req).await
        }
        // F03 registration routes
        else if path.ends_with("/auth/v1/users/register") {
            self.register(bag, req).await
        } else if path.ends_with("/auth/v1/dev/register") {
            self.dev_register(bag, req).await
        }
        // F04 password reset routes
        else if path.ends_with("/auth/v1/users/request_reset") {
            self.request_password_reset(bag, req).await
        } else if path.contains("/auth/v1/users/") && path.ends_with("/reset") {
            self.set_password(bag, req)
        }
        // F05 PoW routes
        else if path.ends_with("/auth/v1/pow") {
            match req.method {
                foundation_http::SimpleMethod::GET => self.pow_challenge(bag, req).await,
                foundation_http::SimpleMethod::POST => self.pow_solve(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F08 template/config routes
        else if path.ends_with("/auth/v1/templates/config") {
            self.template_config(bag, req)
        } else if path.ends_with("/auth/v1/templates/password_policy") {
            self.password_policy(bag, req)
        }
        // F09 account management routes
        else if path.starts_with("/auth/v1/users/") && path.ends_with("/change_password") {
            self.change_password(bag, req).await
        } else if path.starts_with("/auth/v1/users/") && path.ends_with("/sessions") {
            self.list_sessions(bag, req)
        } else if path.starts_with("/auth/v1/users/") && path.ends_with("/revoke") {
            self.revoke_user(bag, req).await
        } else if path.starts_with("/auth/v1/users/")
            && path.matches('/').count() == 4
            && !path.ends_with("/reset")
            && !path.ends_with("/change_password")
            && !path.ends_with("/sessions")
            && !path.ends_with("/revoke")
        {
            match req.method {
                foundation_http::SimpleMethod::GET => self.get_user(bag, req).await,
                foundation_http::SimpleMethod::PUT => self.update_user(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F06 WebAuthn routes
        else if path.ends_with("/auth/v1/webauthn/register/start") {
            self.webauthn_register_start(bag, req).await
        } else if path.ends_with("/auth/v1/webauthn/register/finish") {
            self.webauthn_register_finish(bag, req).await
        } else if path.ends_with("/auth/v1/webauthn/login/start") {
            self.webauthn_auth_start(bag, req).await
        } else if path.ends_with("/auth/v1/webauthn/login/finish") {
            self.webauthn_auth_finish(bag, req).await
        } else if path.starts_with("/auth/v1/webauthn/") && path.matches('/').count() == 4 {
            match req.method {
                foundation_http::SimpleMethod::DELETE => self.delete_passkey(bag, req).await,
                foundation_http::SimpleMethod::PUT => self.rename_passkey(bag, req).await,
                _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
            }
        }
        // F06 passkey-only login routes
        else if path.ends_with("/auth/v1/passkey/login/start") {
            self.passkey_login_start(bag, req).await
        } else if path.ends_with("/auth/v1/passkey/login/finish") {
            self.passkey_login_finish(bag, req).await
        }
        // F07 ToS routes
        else if path.ends_with("/auth/v1/tos/latest") {
            self.tos_latest(bag, req).await
        } else if path.ends_with("/auth/v1/tos/accept") {
            self.tos_accept(bag, req).await
        } else {
            Err(IdpError::NotFound(format!("Unknown endpoint: {path}")))
        }
    }
}

// -- Utilities --

fn extract_query(url: &str) -> Vec<(String, String)> {
    if let Some(q) = url.split('?').nth(1) {
        q.split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let k = parts.next()?;
                let v = parts.next().unwrap_or("");
                Some((
                    urlencoding::decode(k)
                        .unwrap_or_else(|_| k.into())
                        .into_owned(),
                    urlencoding::decode(v)
                        .unwrap_or_else(|_| v.into())
                        .into_owned(),
                ))
            })
            .collect()
    } else {
        vec![]
    }
}

fn query_param<'a>(query: &'a [(String, String)], name: &str) -> Option<&'a str> {
    query
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Check whether a request URL has a particular query parameter.
fn has_query_param(name: &str, req: &SimpleIncomingRequest) -> bool {
    let url = &req.request_url.url;
    extract_query(url).iter().any(|(k, _)| k == name)
}

/// Look up an upstream provider by slug from the `IdP` database.
async fn lookup_upstream_provider(
    qs: &dyn QueryStore,
    id: &str,
) -> Result<Option<UpstreamProvider>, String> {
    let mut stream = qs
        .query(
            "SELECT id, name, provider_type, client_id, client_secret_ciphertext, \
         encryption_key_id, authorization_url, token_url, userinfo_url, discovery_url, \
         scopes, is_active, mapping_config, created_at, updated_at \
         FROM upstream_providers WHERE id = ?",
            &[DataValue::Text(id.to_string())],
        )
        .await
        .map_err(|e| e.to_string())?;

    match stream.next().await {
        Some(Ok(row)) => parse_provider_row(&row),
        Some(Err(e)) => Err(format!("db: {e}")),
        None => Ok(None),
    }
}

/// Decode an `UpstreamProvider` from a SELECT result row.
fn parse_provider_row(row: &SqlRow) -> Result<Option<UpstreamProvider>, String> {
    let get = |name: &str| -> Result<String, String> {
        row.get_by_name::<String>(name)
            .map_err(|e| format!("col {name}: {e}"))
    };
    let get_opt_str = |name: &str| -> Result<Option<String>, String> {
        row.get_by_name::<Option<String>>(name)
            .map_err(|e| format!("col {name}: {e}"))
    };
    let pt: String = get("provider_type")?;
    let scopes_json: String = get("scopes")?;
    let mapping_json: String = get("mapping_config")?;
    let is_active_int: i64 = row
        .get_by_name::<i64>("is_active")
        .map_err(|e| format!("col is_active: {e}"))?;
    let secret: Option<Vec<u8>> = row
        .get_by_name::<Option<Vec<u8>>>("client_secret_ciphertext")
        .map_err(|e| format!("col client_secret_ciphertext: {e}"))?;

    let scopes: Vec<String> = serde_json::from_str(&scopes_json).unwrap_or_default();
    let mapping_config: ProviderMapping = serde_json::from_str(&mapping_json).unwrap_or_default();

    Ok(Some(UpstreamProvider {
        id: get("id")?,
        name: get("name")?,
        provider_type: ProviderType::from_str_lenient(&pt),
        client_id: get("client_id")?,
        client_secret_ciphertext: secret,
        encryption_key_id: get("encryption_key_id")?,
        authorization_url: get_opt_str("authorization_url")?,
        token_url: get_opt_str("token_url")?,
        userinfo_url: get_opt_str("userinfo_url")?,
        discovery_url: get_opt_str("discovery_url")?,
        scopes,
        is_active: is_active_int != 0,
        mapping_config,
        created_at: 0,
        updated_at: 0,
    }))
}

/// Minimal provisioning: find user by email, or create a new row.
async fn provision_from_upstream_profile(
    qs: &dyn QueryStore,
    profile: &crate::shared::upstream_client::UpstreamProfile,
) -> Result<String, String> {
    if let (Some(email), true) = (&profile.email, profile.email_verified) {
        if let Some(user_id) = find_user_by_email_lax(qs, email).await? {
            return Ok(user_id);
        }
    }
    let user_id = uuid::Uuid::new_v4().to_string();
    let name = profile.name.as_deref().unwrap_or(&profile.sub);
    let email = profile.email.as_deref().unwrap_or("");
    let now = chrono::Utc::now().timestamp_millis();
    qs.execute(
        "INSERT INTO users (id, email, username, password_hash, \
         created_at, updated_at) VALUES (?, ?, ?, '', ?, ?)",
        &[
            DataValue::Text(user_id.clone()),
            DataValue::Text(email.to_string()),
            DataValue::Text(name.to_string()),
            DataValue::Integer(now),
            DataValue::Integer(now),
        ],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(user_id)
}

async fn find_user_by_email_lax(
    qs: &dyn QueryStore,
    email: &str,
) -> Result<Option<String>, String> {
    let mut stream = qs
        .query(
            "SELECT id FROM users WHERE email = ?",
            &[DataValue::Text(email.to_lowercase())],
        )
        .await
        .map_err(|e| e.to_string())?;
    match stream.next().await {
        Some(Ok(row)) => row
            .get_by_name::<String>("id")
            .map(Some)
            .map_err(|e| format!("{e}")),
        Some(Err(e)) => Err(format!("db: {e}")),
        None => Ok(None),
    }
}

/// The `PendingLogin` for a passkey or WebAuthn login (no client data).
fn passkey_pending(method: &str) -> PendingLogin {
    PendingLogin {
        auth_method: method.to_string(),
        client_id: None,
        redirect_uri: None,
        state: None,
        nonce: None,
    }
}

/// The first value of a request header, by case-insensitive name.
fn request_header(req: &SimpleIncomingRequest, name: &str) -> Option<String> {
    req.headers
        .iter()
        .find(|(k, _)| header_name_eq(k, name))
        .and_then(|(_, v)| v.first().cloned())
}

/// The value of one cookie of the request, by exact name.
fn request_cookie(req: &SimpleIncomingRequest, name: &str) -> Option<String> {
    req.headers
        .iter()
        .filter(|(k, _)| header_name_eq(k, "cookie"))
        .flat_map(|(_, values)| values.iter())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.to_string())
}

fn extract_body_text(body: Option<&SendSafeBody>) -> String {
    match body {
        Some(SendSafeBody::Text(t)) => t.clone(),
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

/// Compare a header against `name`, case-insensitively, using the header's
/// canonical name rather than its `Debug` form.
#[must_use]
pub fn header_name_eq(header: &foundation_netio::shared::http::SimpleHeader, name: &str) -> bool {
    // `Display` renders known variants as canonical lowercase (hyphens), and
    // `Custom` as its exact value; both compare correctly case-insensitively.
    header.to_string().eq_ignore_ascii_case(name)
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

/// Extract the last path segment after a prefix (e.g. "/auth/v1/users/" → `user_id`).
fn extract_path_param(url: &str, prefix: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    path.strip_prefix(prefix).map(|s| {
        // Take only the first segment (before next '/')
        s.split('/').next().unwrap_or(s).to_string()
    })
}
