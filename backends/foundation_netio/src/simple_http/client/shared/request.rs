//! Prepared HTTP request ready to send.
//!
//! Internal type that holds all request data. Convert to `SimpleIncomingRequest`
//! via `into_simple_incoming_request()` to use with HTTP rendering.

use foundation_core::url::Uri;
use crate::simple_http::{
    HttpClientError, Proto, SendSafeBody, SimpleHeaders, SimpleIncomingRequest,
    SimpleMethod, SimpleUrl,
};

pub use crate::simple_http::shared::Extensions;

/// Prepared HTTP request ready to send.
pub struct PreparedRequest {
    pub method: SimpleMethod,
    pub url: Uri,
    pub headers: SimpleHeaders,
    pub body: SendSafeBody,
    pub extensions: Extensions,
}

impl PreparedRequest {
    /// Converts this prepared request into a `SimpleIncomingRequest`.
    ///
    /// The returned request can be rendered using `Http11::request(req).http_render()`.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the request cannot be built.
    pub fn into_simple_incoming_request(self) -> Result<SimpleIncomingRequest, HttpClientError> {
        let simple_url = if let Some(query) = self.url.query() {
            SimpleUrl::url_with_query(format!("{}?{}", self.url.path(), query))
        } else {
            SimpleUrl::url_only(self.url.to_string())
        };

        let request = SimpleIncomingRequest::builder()
            .with_url(simple_url)
            .with_uri(self.url)
            .with_method(self.method)
            .with_proto(Proto::HTTP11)
            .with_headers(self.headers)
            .with_some_body(Some(self.body))
            .with_extensions(self.extensions)
            .build()
            .map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;

        Ok(request)
    }
}
