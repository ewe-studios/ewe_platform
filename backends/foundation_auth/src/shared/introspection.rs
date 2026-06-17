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
// Platform-specific fetch
// ===========================================================================

/// Build HTTP Basic Auth header value.
fn basic_auth_header(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{client_id}:{client_secret}");
    format!("Basic {}", STANDARD.encode(credentials))
}

#[cfg(not(target_arch = "wasm32"))]
async fn do_introspect(
    url: &str,
    token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, IntrospectionError> {
    use foundation_netio::simple_http::client::SimpleHttpClient;
    use foundation_netio::simple_http::shared::SimpleHeader;

    let body = format!("token={}", urlencoding::encode(token));

    let client = SimpleHttpClient::from_system();
    let resp = client
        .post(url)
        .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))?
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .basic_auth(client_id, client_secret)
        .body_text(body)
        .build_client()
        .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))?
        .send_async()
        .await
        .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();

    if status == 401 {
        let body = match resp.get_body_ref() {
            foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.clone(),
            foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
                String::from_utf8_lossy(b).to_string()
            }
            _ => String::new(),
        };
        return Err(IntrospectionError::Unauthorized(body));
    }

    if !resp.is_success() {
        let body = match resp.get_body_ref() {
            foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.clone(),
            foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
                String::from_utf8_lossy(b).to_string()
            }
            _ => String::new(),
        };
        return Err(IntrospectionError::ServerError {
            status: status as u16,
            message: body,
        });
    }

    match resp.get_body_ref() {
        foundation_netio::simple_http::shared::SendSafeBody::Text(t) => Ok(t.clone()),
        foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
            String::from_utf8(b.clone())
                .map_err(|e| IntrospectionError::ConnectionFailed(e.to_string()))
        }
        _ => Ok(String::new()),
    }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-oauth"))]
async fn do_introspect(
    url: &str,
    token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, IntrospectionError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode};

    let body = format!("token={}", urlencoding::encode(token));

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| IntrospectionError::ConnectionFailed(format!("request failed: {e:?}")))?;
    request
        .headers()
        .set("Content-Type", "application/x-www-form-urlencoded")
        .map_err(|e| {
            IntrospectionError::ConnectionFailed(format!("content-type header failed: {e:?}"))
        })?;
    request
        .headers()
        .set("Authorization", &basic_auth_header(client_id, client_secret))
        .map_err(|e| {
            IntrospectionError::ConnectionFailed(format!("auth header failed: {e:?}"))
        })?;

    let window = web_sys::window()
        .ok_or_else(|| IntrospectionError::ConnectionFailed("no window available".into()))?;
    let resp_value = JsFuture::from(
        window
            .fetch_with_request(request)
            .map_err(|e| IntrospectionError::ConnectionFailed(format!("fetch failed: {e:?}")))?,
    )
    .await
    .map_err(|e| IntrospectionError::ConnectionFailed(format!("await failed: {e:?}")))?;

    let resp: web_sys::Response = resp_value.dyn_into().map_err(|_| {
        IntrospectionError::ConnectionFailed("failed to parse response".into())
    })?;
    let text = JsFuture::from(
        resp.text()
            .map_err(|e| IntrospectionError::ConnectionFailed(format!("text failed: {e:?}")))?,
    )
    .await
    .map_err(|e| IntrospectionError::ConnectionFailed(format!("await text failed: {e:?}")))?;

    Ok(text.as_string().unwrap_or_default())
}

