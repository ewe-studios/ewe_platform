//! The webhook transport boundary.
//!
//! The core builds the signed body. A host supplies the transport, so the crate reads no global
//! network client and a test can capture the request without a socket.

use core::future::Future;
use core::pin::Pin;

/// One webhook call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookRequest {
    /// The webhook uniform resource locator (URL).
    pub url: String,
    /// The request headers, including `Webhook-Signature`.
    pub headers: Vec<(String, String)>,
    /// The exact request body bytes that the signature covers.
    pub body: Vec<u8>,
}

/// One webhook response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookResponse {
    /// The Hypertext Transfer Protocol (HTTP) status.
    pub status: u16,
    /// The response body, bounded by the configuration.
    pub body: Vec<u8>,
}

/// A boxed webhook call.
pub type WebhookFuture<'a> =
    Pin<Box<dyn Future<Output = Result<WebhookResponse, String>> + Send + 'a>>;

/// The webhook transport that a host supplies.
pub trait WebhookSender: Send + Sync {
    /// Send one webhook call.
    fn send(&self, request: WebhookRequest) -> WebhookFuture<'_>;
}
