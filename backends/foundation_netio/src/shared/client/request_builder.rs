//! Cross-platform `PreparedRequest` builder (F51).
//!
//! WHY: `ClientRequestBuilder` is native-only (generic over `R: DnsResolver`).
//! WASM users and any caller that just wants to build a `PreparedRequest` for
//! `HttpClient::send()` needs a platform-agnostic builder. This provides the
//! same convenience (verb helpers, body setters, auth headers) without any
//! resolver or pool dependency.
//!
//! WHAT: `PreparedRequestBuilder` with fluent methods for method, URL, headers,
//! body (text, bytes, JSON, form, raw), and auth conveniences. `build()` returns
//! a `PreparedRequest`.

use foundation_core::url::{Query, Uri};
use serde::Serialize;

use super::request::PreparedRequest;
use super::request_task::HttpExchangeClientTask;
use crate::network_client::DynNetClient;
use crate::shared::http::{
    Extensions, HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
};

pub struct PreparedRequestBuilder {
    method: SimpleMethod,
    url: Option<Uri>,
    headers: SimpleHeaders,
    body: Option<SendSafeBody>,
}

impl PreparedRequestBuilder {
    /// Create a builder for `method` targeting `url`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn new(method: SimpleMethod, url: &str) -> Result<Self, HttpClientError> {
        let uri = Uri::parse(url)?;
        let mut headers = SimpleHeaders::new();
        if let Some(host) = uri.host_str() {
            let host_value = match uri.port() {
                Some(p) => format!("{host}:{p}"),
                None => host,
            };
            headers.insert(SimpleHeader::HOST, vec![host_value]);
        }
        Ok(Self {
            method,
            url: Some(uri),
            headers,
            body: None,
        })
    }

    // -- convenience constructors -----------------------------------------

    /// Build a GET request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn get(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::GET, url)
    }

    /// Build a POST request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn post(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::POST, url)
    }

    /// Build a PUT request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn put(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PUT, url)
    }

    /// Build a DELETE request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn delete(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::DELETE, url)
    }

    /// Build a PATCH request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn patch(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PATCH, url)
    }

    /// Build a HEAD request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn head(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::HEAD, url)
    }

    /// Build an OPTIONS request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `url` cannot be parsed.
    pub fn options(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::OPTIONS, url)
    }

    // -- headers ----------------------------------------------------------

    /// Add a header to the request.
    #[must_use]
    pub fn header(mut self, key: impl Into<SimpleHeader>, value: impl Into<String>) -> Self {
        self.headers
            .entry(key.into())
            .or_default()
            .push(value.into());
        self
    }

    /// Replace all headers on the request.
    #[must_use]
    pub fn headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = headers;
        self
    }

    // -- mutating (`&mut self`) variants ----------------------------------
    //
    // The fluent verb methods consume `self`, which is ergonomic for chaining
    // but unusable from a `FnOnce(&mut PreparedRequestBuilder)` closure (the
    // `builder_mod` hook the code generator emits). These `set_*` variants
    // mutate in place so such closures can add headers / bodies.

    /// Append a header, mutating in place (for `&mut` closures).
    pub fn set_header(&mut self, key: impl Into<SimpleHeader>, value: impl Into<String>) {
        self.headers
            .entry(key.into())
            .or_default()
            .push(value.into());
    }

    /// Set a `Bearer` token header, mutating in place.
    pub fn set_bearer_token(&mut self, token: &str) {
        self.set_header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
    }

    /// Serialize `value` as a JSON body, mutating in place. Auto-sets
    /// `Content-Type: application/json` and `Content-Length`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `value` cannot be serialized to JSON.
    pub fn set_body_json<T: Serialize>(&mut self, value: &T) -> Result<(), HttpClientError> {
        let json_string =
            serde_json::to_string(value).map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;
        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/json".to_string()],
        );
        self.headers.insert(
            SimpleHeader::CONTENT_LENGTH,
            vec![json_string.len().to_string()],
        );
        self.body = Some(SendSafeBody::Text(json_string));
        Ok(())
    }

    // -- body -------------------------------------------------------------

    /// Set a plain-text body. Auto-sets `Content-Type: text/plain` and
    /// `Content-Length`.
    #[must_use]
    pub fn body_text(mut self, text: impl Into<String>) -> Self {
        let text_string = text.into();
        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["text/plain".to_string()]);
        self.headers.insert(
            SimpleHeader::CONTENT_LENGTH,
            vec![text_string.len().to_string()],
        );
        self.body = Some(SendSafeBody::Text(text_string));
        self
    }

    /// Set a raw byte body. Auto-sets `Content-Type: application/octet-stream`
    /// and `Content-Length`.
    #[must_use]
    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["application/octet-stream".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![bytes.len().to_string()]);
        self.body = Some(SendSafeBody::Bytes(bytes));
        self
    }

    /// Serialize `value` as a JSON body. Auto-sets `Content-Type: application/json`
    /// and `Content-Length`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError`] if `value` cannot be serialized to JSON.
    pub fn body_json<T: Serialize>(mut self, value: &T) -> Result<Self, HttpClientError> {
        let json_string =
            serde_json::to_string(value).map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;
        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/json".to_string()],
        );
        self.headers.insert(
            SimpleHeader::CONTENT_LENGTH,
            vec![json_string.len().to_string()],
        );
        self.body = Some(SendSafeBody::Text(json_string));
        Ok(self)
    }

    /// Set a URL-encoded form body. Auto-sets
    /// `Content-Type: application/x-www-form-urlencoded` and `Content-Length`.
    #[must_use]
    pub fn body_form(mut self, params: &[(String, String)]) -> Self {
        fn urlencode(s: &str) -> String {
            s.chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    ' ' => "+".to_string(),
                    _ => format!("%{:02X}", c as u8),
                })
                .collect()
        }
        let body = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&");
        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/x-www-form-urlencoded".to_string()],
        );
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![body.len().to_string()]);
        self.body = Some(SendSafeBody::Text(body));
        self
    }

    /// Set a raw body without auto-setting headers.
    ///
    /// Unlike `body_text` / `body_bytes` / `body_json`, this does not
    /// auto-set `Content-Type` or `Content-Length` — the caller is responsible
    /// for headers when using raw body variants (streams, chunked, SSE, etc.).
    #[must_use]
    pub fn body(mut self, body: SendSafeBody) -> Self {
        self.body = Some(body);
        self
    }

    // -- auth -------------------------------------------------------------

    /// Set a `Basic` auth header. Encodes `username:password` as base64.
    #[must_use]
    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        use base64::prelude::*;
        let credentials = format!("{username}:{password}");
        let encoded = BASE64_STANDARD.encode(credentials.as_bytes());
        self.header(SimpleHeader::AUTHORIZATION, format!("Basic {encoded}"))
    }

    /// Set a `Bearer` token header.
    #[must_use]
    pub fn bearer_token(self, token: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
    }

    /// Set an arbitrary API key in the given header.
    #[must_use]
    pub fn api_key(self, header_name: &str, key: &str) -> Self {
        self.header(SimpleHeader::custom(header_name), key)
    }

    /// Set the `X-API-Key` header.
    #[must_use]
    pub fn x_api_key(self, key: &str) -> Self {
        self.api_key("X-API-Key", key)
    }

    /// Set an `Authorization` header with an arbitrary scheme and credentials.
    #[must_use]
    pub fn authorization(self, scheme: &str, credentials: &str) -> Self {
        self.header(
            SimpleHeader::AUTHORIZATION,
            format!("{scheme} {credentials}"),
        )
    }

    // -- query params -----------------------------------------------------

    /// Append a query parameter to the request URL.
    ///
    /// WHY: Generated and hand-written clients build query strings incrementally
    /// from optional args; a `None` value should simply be skipped.
    ///
    /// WHAT: When `value` is `Some`, appends `key=value` to the URL's structured
    /// query (percent-encoded at render time). When `value` is `None`, this is a
    /// no-op.
    ///
    /// HOW: Delegates to [`Uri::append_query`] on the internal URL.
    ///
    /// # Panics
    ///
    /// This function does not panic.
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        if let (Some(v), Some(uri)) = (value, self.url.as_mut()) {
            uri.append_query(key, v);
        }
        self
    }

    /// Set a query parameter, removing any prior values for the same key.
    ///
    /// WHY: Single-valued params must not accumulate duplicates when re-set.
    ///
    /// WHAT: Removes existing pairs for `key`, then appends the new pair.
    ///
    /// HOW: Delegates to [`Uri::set_query`] on the internal URL.
    ///
    /// # Panics
    ///
    /// This function does not panic.
    #[must_use]
    pub fn query_set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(uri) = self.url.as_mut() {
            uri.set_query(key, value);
        }
        self
    }

    /// Remove all query params for the given key.
    ///
    /// # Panics
    ///
    /// This function does not panic.
    #[must_use]
    pub fn query_remove(mut self, key: &str) -> Self {
        if let Some(uri) = self.url.as_mut() {
            uri.remove_query(key);
        }
        self
    }

    /// Replace all query params with the given [`Query`].
    ///
    /// # Panics
    ///
    /// This function does not panic.
    #[must_use]
    pub fn query_params(mut self, query: Query) -> Self {
        if let Some(uri) = self.url.as_mut() {
            *uri = uri.with_query_params(query);
        }
        self
    }

    // -- send -------------------------------------------------------------

    /// Consume the builder, build a [`PreparedRequest`], and open an HTTP
    /// exchange via `client`.
    ///
    /// WHY: The cross-platform equivalent of the native-only
    /// `ClientRequestBuilder::build_send_request()`. Callers get an unsent
    /// [`HttpExchangeClientTask`] (a valtron `TaskIterator`) they can split and
    /// send themselves (see `body_reader::split_exchange`).
    ///
    /// WHAT: Builds the request and calls [`HttpClient::open_exchange`] on the
    /// client.
    ///
    /// HOW: `DynNetClient` derefs to `dyn NetClient : HttpClient`, so
    /// `open_exchange` is directly callable.
    ///
    /// # Panics
    ///
    /// This function does not panic (see [`PreparedRequestBuilder::build`]).
    #[must_use]
    pub fn send(self, client: DynNetClient) -> HttpExchangeClientTask {
        client.open_exchange(self.build())
    }

    // -- build ------------------------------------------------------------

    /// Consume the builder and return a [`PreparedRequest`].
    ///
    /// # Panics
    ///
    /// This method cannot panic in practice — `Self::new()` always stores the
    /// URL. If called on a builder constructed through some other means,
    /// panics with "url must be set".
    #[must_use]
    pub fn build(mut self) -> PreparedRequest {
        let url = self.url.take().expect("url must be set");
        PreparedRequest {
            method: self.method,
            url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        }
    }
}
