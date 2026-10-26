//! Token Introspection Client — RFC 7662.
//!
//! Allows a resource server to validate an access token by querying the
//! authorization server's introspection endpoint.

use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use serde::{Deserialize, Serialize};

/// Token introspection result (RFC 7662).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionResult {
    /// Whether the token is currently active.
    pub active: bool,
    /// Space-separated list of scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Client ID the token was issued to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Subject identifier (user ID).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Human-readable username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Token type (e.g. "Bearer").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// Expiration time (Unix timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    /// Issuance time (Unix timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// Not-before time (Unix timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Subject of the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Intended audience.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    /// Token identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

impl IntrospectionResult {
    /// Check if the token is active and not expired.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        if !self.active {
            return false;
        }
        if let Some(exp) = self.exp {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if exp < now {
                return false;
            }
        }
        true
    }
}

/// Token introspection client.
pub struct IntrospectionClient {
    http: DynNetClient,
}

impl Default for IntrospectionClient {
    fn default() -> Self {
        Self::new()
    }
}

impl IntrospectionClient {
    /// A client that uses the platform's default HTTP client
    /// ([`HttpClientBuilder`]: the native client, or the fetch client on wasm32).
    #[must_use]
    pub fn new() -> Self {
        Self::with_client(HttpClientBuilder::new().build())
    }

    /// A client that sends its requests through `http` (a shared client, or a
    /// test client).
    #[must_use]
    pub fn with_client(http: DynNetClient) -> Self {
        Self { http }
    }

    /// Introspect a token at the given endpoint.
    ///
    /// The resource authenticates with HTTP Basic Auth using `client_id`
    /// and `client_secret`.
    ///
    /// # Errors
    ///
    /// Returns `IntrospectionError` if the request fails or the response
    /// cannot be parsed.
    pub async fn introspect(
        &self,
        introspection_url: &str,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<IntrospectionResult, IntrospectionError> {
        let body =
            do_introspect(&self.http, introspection_url, token, client_id, client_secret).await?;
        serde_json::from_str(&body)
            .map_err(|e| IntrospectionError::ParseError(e.to_string()))
    }
}

/// Introspection-related errors.
#[derive(Debug)]
pub enum IntrospectionError {
    /// Connection failed.
    ConnectionFailed(String),
    /// 401 — resource server credentials invalid.
    Unauthorized(String),
    /// Server error with status and message.
    ServerError { status: u16, message: String },
    /// JSON parse failed.
    ParseError(String),
}

impl core::fmt::Display for IntrospectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IntrospectionError::ConnectionFailed(s) => {
                write!(f, "Introspection connection failed: {s}")
            }
            IntrospectionError::Unauthorized(s) => {
                write!(f, "Introspection unauthorized: {s}")
            }
            IntrospectionError::ServerError { status, message } => {
                write!(f, "Introspection server error ({status}): {message}")
            }
            IntrospectionError::ParseError(s) => {
                write!(f, "Introspection parse error: {s}")
            }
        }
    }
}

impl std::error::Error for IntrospectionError {}

// ===========================================================================
// HTTP fetch — over the platform network client, `DynNetClient` (native + wasm).
// ===========================================================================

fn basic_auth_header(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{client_id}:{client_secret}");
    format!("Basic {}", STANDARD.encode(credentials))
}

async fn do_introspect(
    client: &DynNetClient,
    url: &str,
    token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, IntrospectionError> {
    let body_text = format!("token={}", urlencoding::encode(token));
    let req = PreparedRequestBuilder::post(url)
        .map_err(|e| IntrospectionError::ConnectionFailed(format!("invalid URL: {e}")))?
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(SimpleHeader::AUTHORIZATION, basic_auth_header(client_id, client_secret))
        .body_text(body_text)
        .build();

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();
    // Response bodies arrive as a lazy `SendSafeBody::Stream`, so they must be
    // drained with `try_collect_bytes` (`get_body_ref` would observe them empty).
    let body = try_collect_bytes(resp.take_body())
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| IntrospectionError::ConnectionFailed(format!("read body: {e}")))?;

    if status == 401 {
        return Err(IntrospectionError::Unauthorized(body));
    }
    if !(200..300).contains(&status) {
        return Err(IntrospectionError::ServerError {
            status: status as u16,
            message: body,
        });
    }
    Ok(body)
}
