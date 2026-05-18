//! OAuth 2.0 token exchange using browser/worker fetch API.

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::shared::oauth::{OAuthConfig, OAuthError, OAuthManager, TokenResponse};
use crate::shared::oauth_token::OAuthToken;

/// Wasm OAuth client wrapping shared OAuth configuration with async token exchange.
pub struct WasmOAuth {
    inner: OAuthManager,
}

impl WasmOAuth {
    /// Create a new wasm OAuth client.
    #[must_use]
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            inner: OAuthManager::new(config),
        }
    }

    /// Get the shared OAuth manager for URL generation.
    #[must_use]
    pub fn manager(&self) -> &OAuthManager {
        &self.inner
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
        self.inner.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("authorization_code")),
            format!("code={}", urlencoding::encode(code)),
            format!(
                "redirect_uri={}",
                urlencoding::encode(&self.inner.config.redirect_uri)
            ),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
        ];

        if let Some(ref secret) = self.inner.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        if let Some(verifier) = code_verifier {
            body_parts.push(format!("code_verifier={}", urlencoding::encode(verifier)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.inner.config.token_url, &body).await?;
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
        self.inner.config.validate()?;

        let Some(ref client_secret) = self.inner.config.client_secret else {
            return Err(OAuthError::MissingClientSecret);
        };

        let scope_str;
        if let Some(ref s) = scopes {
            scope_str = s.join(" ");
        } else if !self.inner.config.scopes.is_empty() {
            scope_str = self.inner.config.scopes.join(" ");
        } else {
            scope_str = String::new();
        }

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("client_credentials")),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
            format!("client_secret={}", urlencoding::encode(client_secret)),
        ];

        if !scope_str.is_empty() {
            body_parts.push(format!("scope={}", urlencoding::encode(&scope_str)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.inner.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Refresh an access token using a refresh token.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the refresh request fails or the response cannot be parsed.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, OAuthError> {
        self.inner.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("refresh_token")),
            format!("refresh_token={}", urlencoding::encode(refresh_token)),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
        ];

        if let Some(ref secret) = self.inner.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        let body = body_parts.join("&");

        let resp = fetch_post(&self.inner.config.token_url, &body).await?;
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
    let init = RequestInit::new();
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

    if let Ok(true) = js_sys::Reflect::has(
        &global,
        &wasm_bindgen::JsValue::from_str("ServiceWorkerGlobalScope"),
    ) {
        global
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
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
