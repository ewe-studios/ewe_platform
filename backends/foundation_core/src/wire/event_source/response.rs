//! SSE HTTP response helper.
//!
//! WHY: Servers need to build SSE HTTP responses with correct headers.
//! WHAT: [`SseResponse`] builder for creating SSE-ready HTTP responses.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleOutgoingResponse,
    SimpleOutgoingResponseBuilder, Status,
};
use std::collections::BTreeMap;

/// [`SseResponse`] builds SSE HTTP responses with correct headers.
pub struct SseResponse {
    status: Status,
    headers: BTreeMap<SimpleHeader, String>,
}

impl SseResponse {
    #[must_use]
    pub fn new() -> Self {
        let mut headers = BTreeMap::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, "text/event-stream".to_string());
        headers.insert(SimpleHeader::CACHE_CONTROL, "no-cache".to_string());
        headers.insert(SimpleHeader::CONNECTION, "keep-alive".to_string());

        Self {
            status: Status::OK,
            headers,
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        self.headers.insert(name, value.into());
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn headers(&self) -> &BTreeMap<SimpleHeader, String> {
        &self.headers
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.status.clone()
    }

    /// Build the SSE HTTP response.
    ///
    /// # Panics
    ///
    /// Panics if the response builder fails (should never happen with valid headers).
    #[must_use]
    pub fn build(self) -> SimpleOutgoingResponse {
        let simple_headers: SimpleHeaders = self
            .headers
            .into_iter()
            .map(|(key, value)| (key, vec![value]))
            .collect();

        SimpleOutgoingResponseBuilder::default()
            .with_proto(Proto::HTTP11)
            .with_status(self.status)
            .with_headers(simple_headers)
            .with_body(SendSafeBody::None)
            .build()
            .expect("Failed to build SSE response")
    }
}

impl Default for SseResponse {
    fn default() -> Self {
        Self::new()
    }
}
