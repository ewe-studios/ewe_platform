//! Token Introspection Client — RFC 7662.
//!
//! Allows a resource server to validate an access token by querying the
//! authorization server's introspection endpoint.

use base64::{engine::general_purpose::STANDARD, Engine};
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
pub struct IntrospectionClient;

impl IntrospectionClient {
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
        introspection_url: &str,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<IntrospectionResult, IntrospectionError> {
        let body = do_introspect(introspection_url, token, client_id, client_secret).await?;
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
// HTTP fetch — uses the cross-platform HttpClient trait (native + wasm).
// ===========================================================================

fn basic_auth_header(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{client_id}:{client_secret}");
    format!("Basic {}", STANDARD.encode(credentials))
}

async fn do_introspect(
    url: &str,
    token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, IntrospectionError> {
    use foundation_core::url::Uri;
    use foundation_netio::http::default_http_client;
    use foundation_netio::shared::client::request::PreparedRequest;
    use foundation_netio::shared::http::{
        SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    };

    let client = default_http_client();
    let uri = Uri::parse(url)
        .map_err(|e| IntrospectionError::ConnectionFailed(format!("invalid URL: {e}")))?;

    let body_text = format!("token={}", urlencoding::encode(token));

    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/x-www-form-urlencoded".into()],
    );
    headers.insert(
        SimpleHeader::AUTHORIZATION,
        vec![basic_auth_header(client_id, client_secret)],
    );

    let req = PreparedRequest {
        method: SimpleMethod::POST,
        url: uri,
        headers,
        body: SendSafeBody::Text(body_text),
        extensions: Default::default(),
    };

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();

    if status == 401 {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(IntrospectionError::Unauthorized(body));
    }

    if !(200..300).contains(&status) {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(IntrospectionError::ServerError {
            status: status as u16,
            message: body,
        });
    }

    match resp.get_body_ref() {
        SendSafeBody::Text(t) => Ok(t.clone()),
        SendSafeBody::Bytes(b) => String::from_utf8(b.clone())
            .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string())),
        _ => Ok(String::new()),
    }
}
