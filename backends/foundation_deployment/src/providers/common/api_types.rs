//! Shared API types for all providers.
//!
//! WHY: Avoid duplicating `ApiError`, `ApiPending`, `ApiResponse`, `Empty`, and `Operation`
//!      in every provider. These types are generated identically across all providers.
//!
//! WHAT: Provider-agnostic error and response types used by all generated API clients.
//!
//! HOW: Re-export these from each provider's `shared` module for convenience.

use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeaders};
use foundation_netio::DynNetClient;
use serde::{Deserialize, Serialize};

// Re-export types from foundation_core for convenience
pub use foundation_core::valtron::BoxedSendExecutionAction;
#[deprecated(
    note = "RequestIntro is native-only; use HttpClient::open_exchange() + HttpExchange \
            (or body_reader::split_exchange/collect_exchange) instead"
)]
pub use foundation_netio::http::RequestIntro;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Provider-agnostic error type for API operations.
#[derive(Debug)]
pub enum ApiError {
    RequestBuildFailed(String),
    RequestSendFailed(String),
    HttpStatus {
        code: u16,
        headers: SimpleHeaders,
        body: Option<String>,
    },
    ParseFailed(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::RequestBuildFailed(e) => write!(f, "request build failed: {e}"),
            ApiError::RequestSendFailed(e) => write!(f, "request send failed: {e}"),
            ApiError::HttpStatus { code, body, .. } => {
                write!(f, "HTTP status {code}")?;
                if let Some(b) = body {
                    write!(f, ": {b}")?;
                }
                Ok(())
            }
            ApiError::ParseFailed(e) => write!(f, "parse failed: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Progress states for API operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPending {
    Building,
    Sending,
}

/// Generic API response with status, headers, and parsed body.
#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}

// =============================================================================
// SENDING A GENERATED REQUEST
// =============================================================================

/// Send a generated endpoint's request and parse its JSON body as `T`.
///
/// WHY: every generated `*_request` fn used to carry its own copy of this —
/// send, check the status, read the error body, parse. Cloudflare alone has
/// 2442 endpoints, and type checking and borrow checking 2442 copies of the
/// same async body was a large share of what that crate's `rustc` needed. One
/// generic function is checked once.
///
/// WHAT: the status is checked before the body is read. A non-2xx response
/// becomes [`ApiError::HttpStatus`] carrying the vendor's error body (the
/// diagnosis — "`server_type` cx99 does not exist" — lives there). A 2xx body is
/// parsed as `T`; it is read whole because the parsed value is what the caller
/// needs next.
///
/// # Errors
/// [`ApiError::RequestSendFailed`] when the request cannot be sent,
/// [`ApiError::HttpStatus`] for a non-2xx status, and [`ApiError::ParseFailed`]
/// when the body is not a `T`.
pub async fn send_json<T>(
    client: DynNetClient,
    request: PreparedRequest,
) -> Result<ApiResponse<T>, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let (status, headers, body) = send_checked(client, request).await?;
    let bytes = collect_bytes_from_send_safe(body);
    let parsed: T =
        serde_json::from_slice(&bytes).map_err(|e| ApiError::ParseFailed(e.to_string()))?;
    Ok(ApiResponse {
        status,
        headers,
        body: parsed,
    })
}

/// Send a generated endpoint's request whose success response has no body
/// the spec describes. The body is not read.
///
/// # Errors
/// [`ApiError::RequestSendFailed`] when the request cannot be sent, and
/// [`ApiError::HttpStatus`] for a non-2xx status.
pub async fn send_without_body(
    client: DynNetClient,
    request: PreparedRequest,
) -> Result<ApiResponse<()>, ApiError> {
    let (status, headers, _body) = send_checked(client, request).await?;
    Ok(ApiResponse {
        status,
        headers,
        body: (),
    })
}

/// Send, and turn a non-2xx status into [`ApiError::HttpStatus`] with the
/// error body read (up to [`MAX_ERROR_BODY`](super::body::MAX_ERROR_BODY)); a
/// 2xx response is handed back with its body unread — the streaming
/// endpoints wrap it in a [`ByteStream`](super::body::ByteStream) or
/// [`JsonLinesStream`](super::body::JsonLinesStream).
///
/// # Errors
/// [`ApiError::RequestSendFailed`] when the request cannot be sent, and
/// [`ApiError::HttpStatus`] for a non-2xx status.
pub async fn send_checked(
    client: DynNetClient,
    request: PreparedRequest,
) -> Result<(u16, SimpleHeaders, SendSafeBody), ApiError> {
    let response = client
        .send_async(request)
        .await
        .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;
    let status: usize = response.get_status().into();
    let status = u16::try_from(status)
        .map_err(|_| ApiError::RequestSendFailed(format!("invalid HTTP status {status}")))?;
    let headers = response.get_headers_ref().clone();
    let body = response.take_body();
    if !(200..300).contains(&status) {
        let body = super::body::read_error_body(body).await;
        return Err(ApiError::HttpStatus {
            code: status,
            headers,
            body,
        });
    }
    Ok((status, headers, body))
}

/// Empty response body for endpoints that return `{}`.
#[derive(Debug, Clone, Serialize, Deserialize, foundation_macros::JsonHash)]
pub struct Empty {
    #[serde(flatten)]
    pub data: std::collections::HashMap<String, serde_json::Value>,
}

/// Operation metadata returned by long-running GCP operations.
#[derive(Debug, Clone, Serialize, Deserialize, foundation_macros::JsonHash)]
pub struct Operation {
    #[serde(flatten)]
    pub data: std::collections::HashMap<String, serde_json::Value>,
}
