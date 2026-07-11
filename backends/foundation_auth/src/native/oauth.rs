//! OAuth 2.0 token exchange using native HTTP client.

use foundation_core::valtron::{from_future, execute, ShortCircuit, Stream, StreamIteratorExt};
use foundation_db::{StorageError, StorageItemStream};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_netio::http::FinalizedResponse;

type HttpResponse = FinalizedResponse<SendSafeBody, SystemDnsResolver>;

use crate::shared::oauth::{OAuthConfig, OAuthError, OAuthManager, TokenResponse};
use crate::shared::oauth_token::OAuthToken;

/// Native OAuth client wrapping shared OAuth configuration with sync token exchange.
#[derive(Clone)]
pub struct NativeOAuth {
    inner: OAuthManager,
}

impl NativeOAuth {
    /// Create a new native OAuth client.
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

    // ========================================================================
    // Async API (source of truth — uses SimpleHttpClient::send_async)
    // ========================================================================

    /// Async version of [`Self::exchange_code`].
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

        let resp = do_post_async(&self.inner.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Async version of [`Self::client_credentials`].
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

        let resp = do_post_async(&self.inner.config.token_url, &body).await?;
        parse_token_response(resp).await
    }

    /// Async version of [`Self::refresh_token`].
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

        let resp = do_post_async(&self.inner.config.token_url, &body).await?;
        parse_refresh_response(resp, refresh_token).await
    }

    // ========================================================================
    // Sync API (wraps async via valtron from_future)
    // ========================================================================

    /// Exchange authorization code for tokens.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation)]
    pub fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthToken, OAuthError> {
        let this = self.clone();
        let code = code.to_string();
        let code_verifier = code_verifier.map(String::from);
        let stream = Self::wrap_oauth_async(async move {
            this.exchange_code_async(&code, code_verifier.as_deref()).await
        })
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;
        collect_one_result(stream)
    }

    /// Client credentials flow for service-to-service authentication.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation, clippy::needless_pass_by_value)]
    pub fn client_credentials(
        &self,
        scopes: Option<Vec<String>>,
    ) -> Result<OAuthToken, OAuthError> {
        let this = self.clone();
        let scopes = scopes.clone();
        let stream = Self::wrap_oauth_async(async move {
            this.client_credentials_async(scopes).await
        })
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;
        collect_one_result(stream)
    }

    /// Refresh an access token using a refresh token.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the refresh request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation)]
    pub fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, OAuthError> {
        let this = self.clone();
        let refresh_token = refresh_token.to_string();
        let stream = Self::wrap_oauth_async(async move {
            this.refresh_token_async(&refresh_token).await
        })
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;
        collect_one_result(stream)
    }

    /// Wrap an OAuth async future into a valtron `StorageItemStream`.
    fn wrap_oauth_async<T: Send + 'static>(
        future: impl std::future::Future<Output = Result<T, OAuthError>> + Send + 'static,
    ) -> Result<StorageItemStream<'static, T>, StorageError> {
        let task = from_future(async move {
            future.await.map_err(|e| StorageError::Backend(e.to_string()))
        });
        let stream = execute(task, None)
            .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
        Ok(Box::new(
            stream
                .map_circuit(|item| match item {
                    Stream::Next(result) => match result {
                        Ok(v) => ShortCircuit::Continue(Stream::Next(Ok(v))),
                        Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
                    },
                    _ => ShortCircuit::Continue(Stream::Ignore),
                })
                .map_pending(|_| ()),
        ))
    }
}

/// Helper: collect one result from a valtron stream.
fn collect_one_result<T>(
    stream: StorageItemStream<'_, T>,
) -> Result<T, OAuthError> {
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map_err(|e| OAuthError::TokenRequestFailed(e.to_string()));
        }
    }
    Err(OAuthError::TokenRequestFailed("stream ended without result".into()))
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Async HTTP POST with form-urlencoded body.
async fn do_post_async(url: &str, body: &str) -> Result<HttpResponse, OAuthError> {
    let client = foundation_netio::http::SimpleHttpClient::from_system();
    let response = client
        .post(url)
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body_text(body.to_string())
        .build_client()
        .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
        .send_async()
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
    if !resp.is_success() {
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
    if !resp.is_success() {
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
