//! OAuth 2.0 token exchange over the platform network client (`DynNetClient`).

use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleResponse};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

type HttpResponse = SimpleResponse<SendSafeBody>;

use crate::shared::oauth::{OAuthConfig, OAuthError, OAuthManager, TokenResponse};
use crate::shared::oauth_token::OAuthToken;

/// Native OAuth client: shared OAuth configuration plus async token exchange.
#[derive(Clone)]
pub struct NativeOAuth {
    http: DynNetClient,
    inner: OAuthManager,
}

impl NativeOAuth {
    /// Create a new native OAuth client that uses the platform's default HTTP
    /// client ([`HttpClientBuilder`]).
    #[must_use]
    pub fn new(config: OAuthConfig) -> Self {
        Self::with_client(HttpClientBuilder::new().build(), config)
    }

    /// Create a new native OAuth client that sends its token requests through
    /// `http` (a shared client, or a test client).
    #[must_use]
    pub fn with_client(http: DynNetClient, config: OAuthConfig) -> Self {
        Self {
            http,
            inner: OAuthManager::new(config),
        }
    }

    /// Get the shared OAuth manager for URL generation.
    #[must_use]
    pub fn manager(&self) -> &OAuthManager {
        &self.inner
    }

    // ========================================================================
    // Token exchange (async, over the DynNetClient)
    // ========================================================================

    /// Exchange an authorization code (with its PKCE verifier) for tokens.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the request fails or the response can't be parsed.
    pub async fn exchange_code_async(
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

        let resp = do_post_async(&self.http, &self.inner.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Client-credentials flow for service-to-service authentication.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the request fails or the response can't be parsed.
    pub async fn client_credentials_async(
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

        let resp = do_post_async(&self.http, &self.inner.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Refresh an access token with a refresh token.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the request fails or the response can't be parsed.
    pub async fn refresh_token_async(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthToken, OAuthError> {
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

        let resp = do_post_async(&self.http, &self.inner.config.token_url, &body).await?;
        parse_refresh_response(resp, refresh_token).await
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Async HTTP POST with form-urlencoded body.
async fn do_post_async(
    client: &DynNetClient,
    url: &str,
    body: &str,
) -> Result<HttpResponse, OAuthError> {
    let request = PreparedRequestBuilder::post(url)
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body_text(body)
        .build();
    let response = client
        .send_async(request)
        .await
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

    Ok(response)
}

/// Extract body text from a response.
fn extract_body_text(body: &SendSafeBody) -> Result<String, OAuthError> {
    match body {
        SendSafeBody::Text(t) => Ok(t.clone()),
        SendSafeBody::Bytes(b) => {
            String::from_utf8(b.clone()).map_err(|e| OAuthError::TokenParseError(e.to_string()))
        }
        _ => Ok(String::new()),
    }
}

/// Get error body text or empty string.
fn error_body_text(body: &SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(t) => t.clone(),
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

/// Parse a successful token response into OAuthToken.
async fn parse_token_response(resp: HttpResponse) -> Result<OAuthToken, OAuthError> {
    let status: usize = resp.get_status().into();
    if !(200..=299).contains(&status) {
        let body = error_body_text(resp.get_body_ref());
        return Err(OAuthError::TokenEndpointError {
            status: status as u16,
            message: body,
        });
    }

    let body_text = extract_body_text(resp.get_body_ref())?;
    let token_response: TokenResponse = serde_json::from_str(&body_text)
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
async fn parse_refresh_response(
    resp: HttpResponse,
    old_refresh: &str,
) -> Result<OAuthToken, OAuthError> {
    let status: usize = resp.get_status().into();
    if !(200..=299).contains(&status) {
        let body = error_body_text(resp.get_body_ref());
        return Err(OAuthError::TokenEndpointError {
            status: status as u16,
            message: body,
        });
    }

    let body_text = extract_body_text(resp.get_body_ref())?;
    let token_response: TokenResponse = serde_json::from_str(&body_text)
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
