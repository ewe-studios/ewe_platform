//! OAuth 2.0 flows using browser/worker fetch API.

use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::shared::oauth::{OAuthConfig, OAuthError, PkceChallenge};
use crate::shared::oauth_token::OAuthToken;

/// OAuth manager for handling OAuth flows via the browser/worker fetch API.
pub struct OAuthManager {
    config: OAuthConfig,
}

impl OAuthManager {
    /// Create a new OAuth manager with the given configuration.
    #[must_use]
    pub fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Get the OAuth configuration.
    #[must_use]
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Generate a random state parameter for CSRF protection.
    #[must_use]
    pub fn generate_state() -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Generate the authorization URL with PKCE support.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the configuration is invalid or the URL cannot be parsed.
    pub fn get_authorization_url(
        &self,
        state: &str,
    ) -> Result<(String, Option<PkceChallenge>), OAuthError> {
        self.config.validate()?;

        let mut query = foundation_core::url::Query::new();
        query.append("response_type", &self.config.response_type);
        query.append("client_id", &self.config.client_id);
        query.append("redirect_uri", &self.config.redirect_uri);
        query.append("state", state);

        if !self.config.scopes.is_empty() {
            let scopes_joined = self.config.scopes.join(" ");
            query.append("scope", &scopes_joined);
        }

        let pkce = if self.config.pkce_enabled {
            let challenge = PkceChallenge::generate();
            query.append("code_challenge", &challenge.code_challenge);
            query.append("code_challenge_method", &challenge.challenge_method);
            Some(challenge)
        } else {
            None
        };

        let query_string = query.to_string();
        let base = &self.config.authorization_url;
        let url = if query_string.is_empty() {
            base.clone()
        } else if base.contains('?') {
            format!("{base}&{query_string}")
        } else {
            format!("{base}?{query_string}")
        };

        Ok((url, pkce))
    }

    /// Validate the state parameter.
    #[must_use]
    pub fn validate_state(expected: &str, actual: &str) -> bool {
        expected.as_bytes() == actual.as_bytes()
    }

    /// Exchange authorization code for tokens.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("authorization_code")),
            format!("code={}", urlencoding::encode(code)),
            format!(
                "redirect_uri={}",
                urlencoding::encode(&self.config.redirect_uri)
            ),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
        ];

        if let Some(ref secret) = self.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        if let Some(verifier) = code_verifier {
            body_parts.push(format!("code_verifier={}", urlencoding::encode(verifier)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Client credentials flow for service-to-service authentication.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    pub async fn client_credentials(
        &self,
        scopes: Option<Vec<String>>,
    ) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        let Some(ref client_secret) = self.config.client_secret else {
            return Err(OAuthError::MissingClientSecret);
        };

        let scope_str;
        if let Some(ref s) = scopes {
            scope_str = s.join(" ");
        } else if !self.config.scopes.is_empty() {
            scope_str = self.config.scopes.join(" ");
        } else {
            scope_str = String::new();
        }

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("client_credentials")),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
            format!("client_secret={}", urlencoding::encode(client_secret)),
        ];

        if !scope_str.is_empty() {
            body_parts.push(format!("scope={}", urlencoding::encode(&scope_str)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Refresh an access token using a refresh token.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the refresh request fails or the response cannot be parsed.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("refresh_token")),
            format!("refresh_token={}", urlencoding::encode(refresh_token)),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
        ];

        if let Some(ref secret) = self.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.config.token_url, &body).await?;
        parse_refresh_response(resp, refresh_token).await
    }
}

/// Perform a POST request with URL-encoded body via the fetch API.
async fn fetch_post(url: &str, body: &str) -> Result<Response, OAuthError> {
    let req = build_request(url, body)?;
    let promise = js_fetch(&req);
    let js_value = JsFuture::from(promise)
        .await
        .map_err(|e| OAuthError::TokenRequestFailed(format!("fetch failed: {e:?}")))?;
    js_value
        .dyn_into::<Response>()
        .map_err(|_| OAuthError::TokenRequestFailed("fetch returned non-Response".into()))
}

/// Build a web_sys::Request for POST with form-urlencoded body.
fn build_request(url: &str, body: &str) -> Result<Request, OAuthError> {
    let mut init = RequestInit::new();
    init.set_method("POST");
    init.set_body(&wasm_bindgen::JsValue::from_str(body));

    let headers = web_sys::Headers::new()
        .map_err(|e| OAuthError::TokenRequestFailed(format!("headers creation failed: {e:?}")))?;
    headers
        .append("Content-Type", "application/x-www-form-urlencoded")
        .map_err(|e| OAuthError::TokenRequestFailed(format!("header append failed: {e:?}")))?;
    init.set_headers(&headers.into());
    init.set_mode(RequestMode::Cors);

    Request::new_with_str_and_init(url, &init)
        .map_err(|e| OAuthError::TokenRequestFailed(format!("request creation failed: {e:?}")))
}

/// Call fetch(), detecting service worker vs browser context.
fn js_fetch(req: &Request) -> js_sys::Promise {
    let global = js_sys::global();

    // Check for service worker context (Cloudflare Workers, service workers)
    if let Ok(true) = js_sys::Reflect::has(
        &global,
        &wasm_bindgen::JsValue::from_str("ServiceWorkerGlobalScope"),
    ) {
        global
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
        // Browser window context
        web_sys::window()
            .expect("fetch: no window and no service worker global scope")
            .fetch_with_request(req)
    }
}

/// Parse a successful token response into OAuthToken.
async fn parse_token_response(resp: Response) -> Result<OAuthToken, OAuthError> {
    let status = resp.status();
    if !resp.ok() {
        let body = response_text(&resp).await.unwrap_or_default();
        return Err(OAuthError::TokenEndpointError { status, message: body });
    }

    let body = response_text(&resp).await?;
    let token_response: TokenResponse = serde_json::from_str(&body)
        .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

    Ok(OAuthToken {
        access_token: token_response.access_token,
        token_type: token_response.token_type,
        expires_in: token_response.expires_in,
        refresh_token: token_response.refresh_token,
        scope: token_response.scope,
        id_token: token_response.id_token,
    })
}

/// Parse a refresh token response, preserving the old refresh token if no new one is returned.
async fn parse_refresh_response(resp: Response, old_refresh: &str) -> Result<OAuthToken, OAuthError> {
    let status = resp.status();
    if !resp.ok() {
        let body = response_text(&resp).await.unwrap_or_default();
        return Err(OAuthError::TokenEndpointError { status, message: body });
    }

    let body = response_text(&resp).await?;
    let token_response: TokenResponse = serde_json::from_str(&body)
        .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

    Ok(OAuthToken {
        access_token: token_response.access_token,
        token_type: token_response.token_type,
        expires_in: token_response.expires_in,
        refresh_token: token_response
            .refresh_token
            .or_else(|| Some(old_refresh.to_string())),
        scope: token_response.scope,
        id_token: token_response.id_token,
    })
}

/// Extract response body as text.
async fn response_text(resp: &Response) -> Result<String, OAuthError> {
    let promise = resp.text()
        .map_err(|e| OAuthError::TokenParseError(format!("response.text() failed: {e:?}")))?;
    let js_value = JsFuture::from(promise)
        .await
        .map_err(|e| OAuthError::TokenParseError(format!("reading response body failed: {e:?}")))?;
    js_value
        .as_string()
        .ok_or_else(|| OAuthError::TokenParseError("response body is not text".into()))
}

/// Token response from OAuth server.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    scope: Option<String>,
    id_token: Option<String>,
}
