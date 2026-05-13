//! Body limit middleware — enforces maximum request body size.
//!
//! WHY: Prevents denial of service via large request bodies.
//! WHAT: Checks Content-Length header and rejects requests exceeding the limit.

use std::sync::Arc;

use foundation_core::wire::simple_http::{
    SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, SendSafeBody, Status,
};

use crate::context::ContextBag;
use crate::middleware::{MiddlewareResult, RequestMiddleware};

/// Body limit middleware.
///
/// Rejects requests with bodies larger than the configured limit.
pub struct BodyLimitMiddleware {
    /// Maximum body size in bytes.
    max_bytes: usize,
    /// Custom error message.
    error_message: String,
}

impl BodyLimitMiddleware {
    /// Create a new body limit middleware.
    ///
    /// * `max_bytes` — maximum allowed body size in bytes.
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            error_message: format!("Request body too large (max {} bytes)", max_bytes),
        }
    }

    /// Create a body limit middleware with a custom error message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.error_message = message.into();
        self
    }

    /// Extract Content-Length header value.
    fn extract_content_length(req: &SimpleIncomingRequest) -> Option<usize> {
        req.headers
            .iter()
            .find(|(k, _)| format!("{k}").eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.first())
            .and_then(|s| s.parse::<usize>().ok())
    }

    /// Build the 413 Payload Too Large response.
    fn too_large_response(&self) -> SimpleOutgoingResponse {
        SimpleOutgoingResponse::builder()
            .with_status(Status::PayloadTooLarge)
            .add_header(SimpleHeader::CONTENT_TYPE, "text/plain")
            .with_body(SendSafeBody::Text(self.error_message.clone()))
            .build()
            .expect("valid 413 response")
    }
}

impl RequestMiddleware for BodyLimitMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        // Check Content-Length header
        if let Some(content_length) = Self::extract_content_length(req) {
            if content_length > self.max_bytes {
                tracing::warn!(
                    "Request body too large: {} bytes (max: {})",
                    content_length,
                    self.max_bytes
                );
                return MiddlewareResult::Response(self.too_large_response());
            }
        }

        // Also check Transfer-Encoding: chunked (no Content-Length)
        // In this case, we can't know the size upfront, so we let it through
        // and rely on the stream reader to enforce limits.
        // For a stricter implementation, we could reject chunked requests
        // or require a different approach.

        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_limit_middleware_new() {
        let mw = BodyLimitMiddleware::new(1024);
        assert_eq!(mw.max_bytes, 1024);
    }

    #[test]
    fn test_body_limit_middleware_with_message() {
        let mw = BodyLimitMiddleware::new(1024).with_message("Custom error");
        assert_eq!(mw.error_message, "Custom error");
    }
}
