//! Shared API types for all providers.
//!
//! WHY: Avoid duplicating `ApiError`, `ApiPending`, `ApiResponse`, `Empty`, and `Operation`
//!      in every provider. These types are generated identically across all providers.
//!
//! WHAT: Provider-agnostic error and response types used by all generated API clients.
//!
//! HOW: Re-export these from each provider's `shared` module for convenience.

use foundation_netio::shared::client::body_reader::{collect_bytes_async, AsyncSendSafeBody};
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::SimpleHeaders;
use foundation_netio::DynNetClient;
use serde::{Deserialize, Serialize};

use super::body::{read_error_body, BodyLimits, ByteStream, JsonLinesStream};

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

/// The client a generated `*_request` function sends with: the transport,
/// and the [`BodyLimits`] its responses are read under.
///
/// WHY: How much of a response body may be held in memory is per-client
/// configuration, not a constant, and a generated function's only handle on
/// its caller's configuration is the client it is given — so the limits
/// travel with the client.
///
/// WHAT: Every generated function, and the shared senders it calls, takes
/// `client: impl Into<ApiClient>`. A bare [`DynNetClient`] converts with
/// [`BodyLimits::default`]; a client that configures its own limits passes
/// `ApiClient::new(http).with_limits(..)`.
///
/// HOW: Cheap to clone — the transport is an `Arc`, the limits are `Copy`.
#[derive(Clone)]
pub struct ApiClient {
    http: DynNetClient,
    limits: BodyLimits,
}

impl ApiClient {
    /// A client over `http`, reading responses under [`BodyLimits::default`].
    #[must_use]
    pub fn new(http: DynNetClient) -> Self {
        Self {
            http,
            limits: BodyLimits::default(),
        }
    }

    /// Read responses under `limits` instead.
    #[must_use]
    pub fn with_limits(mut self, limits: BodyLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The transport requests are sent over.
    #[must_use]
    pub fn http(&self) -> &DynNetClient {
        &self.http
    }

    /// The limits responses are read under.
    #[must_use]
    pub fn limits(&self) -> BodyLimits {
        self.limits
    }
}

impl From<DynNetClient> for ApiClient {
    fn from(http: DynNetClient) -> Self {
        Self::new(http)
    }
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

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
/// diagnosis — "`server_type` cx99 does not exist" — lives there), read up to
/// the client's [`BodyLimits::max_error_body`]. A 2xx body is parsed as `T`;
/// it is read whole because the parsed value is what the caller needs next.
///
/// # Errors
/// [`ApiError::RequestSendFailed`] when the request cannot be sent,
/// [`ApiError::HttpStatus`] for a non-2xx status, and [`ApiError::ParseFailed`]
/// when the body is not a `T`.
pub async fn send_json<T>(
    client: impl Into<ApiClient>,
    request: PreparedRequest,
) -> Result<ApiResponse<T>, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let client = client.into();
    let (status, headers, body) = send_checked(&client, request).await?;
    let bytes = collect_bytes_async(body)
        .await
        .map_err(|e| ApiError::RequestSendFailed(format!("reading the response body: {e}")))?;
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
    client: impl Into<ApiClient>,
    request: PreparedRequest,
) -> Result<ApiResponse<()>, ApiError> {
    let (status, headers, _body) = send_checked(&client.into(), request).await?;
    Ok(ApiResponse {
        status,
        headers,
        body: (),
    })
}

/// Send a generated endpoint's request whose success response is a
/// JSON-lines stream, and hand the body back unread as a [`JsonLinesStream`]
/// of `T` whose lines are capped at the client's
/// [`BodyLimits::max_json_line`].
///
/// # Errors
/// As [`send_checked`]; errors reading the body come out of the stream.
pub async fn send_json_lines<T>(
    client: impl Into<ApiClient>,
    request: PreparedRequest,
) -> Result<ApiResponse<JsonLinesStream<T>>, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let client = client.into();
    let (status, headers, body) = send_checked(&client, request).await?;
    Ok(ApiResponse {
        status,
        headers,
        body: JsonLinesStream::new(body, client.limits().max_json_line),
    })
}

/// Send a generated endpoint's request whose success response is raw bytes
/// (a tar archive, a multiplexed log), and hand the body back unread as a
/// [`ByteStream`].
///
/// # Errors
/// As [`send_checked`]; errors reading the body come out of the stream.
pub async fn send_bytes(
    client: impl Into<ApiClient>,
    request: PreparedRequest,
) -> Result<ApiResponse<ByteStream>, ApiError> {
    let client = client.into();
    let (status, headers, body) = send_checked(&client, request).await?;
    Ok(ApiResponse {
        status,
        headers,
        body: ByteStream::new(body),
    })
}

/// Send, and turn a non-2xx status into [`ApiError::HttpStatus`] with the
/// error body read up to the client's [`BodyLimits::max_error_body`]; a 2xx
/// response is handed back with its body unread.
///
/// # Errors
/// [`ApiError::RequestSendFailed`] when the request cannot be sent, and
/// [`ApiError::HttpStatus`] for a non-2xx status.
pub async fn send_checked(
    client: &ApiClient,
    request: PreparedRequest,
) -> Result<(u16, SimpleHeaders, AsyncSendSafeBody), ApiError> {
    let response = client
        .http
        .send_async(request)
        .await
        .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;
    let status: usize = response.get_status().into();
    let status = u16::try_from(status)
        .map_err(|_| ApiError::RequestSendFailed(format!("invalid HTTP status {status}")))?;
    let headers = response.get_headers_ref().clone();
    let body = response.take_async_body();
    if !(200..300).contains(&status) {
        let body = read_error_body(body, client.limits.max_error_body).await;
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
