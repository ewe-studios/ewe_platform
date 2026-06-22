//! Request builder for HTTP client — native (wasm32-gated).
//!
//! This module provides `ClientRequestBuilder` — the fluent API for building requests.

use foundation_core::url::Uri;
use crate::simple_http::client::shared::{
    ClientConfig, DnsResolver, MiddlewareChain, PreparedRequest, SystemDnsResolver,
};
use crate::simple_http::client::{ClientRequest, HttpConnectionPool};
use crate::simple_http::shared::Extensions;
use crate::simple_http::shared::{
    HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
};
use base64::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Fluent builder for HTTP requests.
pub struct ClientRequestBuilder<R: DnsResolver + 'static> {
    method: SimpleMethod,
    url: Uri,
    headers: SimpleHeaders,
    body: Option<SendSafeBody>,
    config: Option<ClientConfig>,
    pool: Option<Arc<HttpConnectionPool<R>>>,
    middleware_chain: Option<Arc<MiddlewareChain>>,
}

impl<R: DnsResolver + 'static> ClientRequestBuilder<R> {
    #[must_use = "builder must be consumed to produce a PreparedRequest"]
    pub fn build(self) -> Result<PreparedRequest, HttpClientError> {
        Ok(PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        })
    }
}

impl ClientRequestBuilder<SystemDnsResolver> {
    #[must_use = "builder must be consumed to produce a ClientRequest"]
    pub fn system_client(self) -> Result<ClientRequest<SystemDnsResolver>, HttpClientError> {
        self.build_client()
    }
}

impl<R: DnsResolver + Default + 'static> ClientRequestBuilder<R> {
    #[must_use = "builder must be consumed to produce a ClientRequest"]
    pub fn build_client(self) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        };

        let pool = self.pool.unwrap_or_default();
        let config = self.config.unwrap_or_default();
        let middleware_chain = self
            .middleware_chain
            .unwrap_or_else(|| Arc::new(MiddlewareChain::new()));
        Ok(ClientRequest::new(prepared, config, pool, middleware_chain))
    }

    pub fn build_send_request(self) -> Result<super::tasks::SendRequestTask<R>, HttpClientError> {
        let prepared_request = PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        };

        let pool = self.pool.unwrap_or_default();
        let config = self.config.unwrap_or_default();

        Ok(super::tasks::SendRequestTask::new(
            prepared_request,
            config.max_redirects,
            pool,
            config,
        ))
    }
}

impl<R: DnsResolver + 'static> ClientRequestBuilder<R> {
    pub fn new(method: SimpleMethod, url: &str) -> Result<Self, HttpClientError> {
        let parsed_url = Uri::parse(url)?;
        let mut headers = BTreeMap::new();

        let host_str = parsed_url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host in URL".to_string()))?;

        let host = if parsed_url.port().is_some() {
            format!("{}:{}", host_str, parsed_url.port_or_default())
        } else {
            host_str.clone()
        };
        headers.insert(SimpleHeader::HOST, vec![host]);

        Ok(Self {
            method,
            headers,
            url: parsed_url,
            body: None,
            pool: None,
            config: None,
            middleware_chain: None,
        })
    }

    #[must_use]
    pub fn with_pool(mut self, pool: Option<Arc<HttpConnectionPool<R>>>) -> Self {
        self.pool = pool;
        self
    }

    #[must_use]
    pub fn without_pool(mut self) -> Self {
        self.pool = None;
        self
    }

    #[must_use]
    pub fn with_middleware(mut self, chain: Arc<MiddlewareChain>) -> Self {
        self.middleware_chain = Some(chain);
        self
    }

    #[must_use]
    pub fn pool(mut self, pool: Arc<HttpConnectionPool<R>>) -> Self {
        self.pool = Some(pool);
        self
    }

    #[must_use]
    pub fn client_config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_client_confg(mut self, changer: impl FnOnce(&mut ClientConfig)) -> Self {
        let mut current = self.config.unwrap_or_default();
        changer(&mut current);
        self.config = Some(current);
        self
    }

    pub fn client_config_follow_other_redirects_response(
        self,
        follow_other_redirects_response: bool,
    ) -> Self {
        self.with_client_confg(|config| {
            config.follow_other_redirects_response = follow_other_redirects_response;
        })
    }

    pub fn client_config_headers_to_add(self, headers: SimpleHeaders) -> Self {
        self.with_client_confg(|config| {
            config.headers_to_add = Some(headers);
        })
    }

    pub fn client_config_headers_to_pass_on(self, headers: Vec<SimpleHeader>) -> Self {
        self.with_client_confg(|config| {
            config.headers_to_pass_on_redirect = Some(headers);
        })
    }

    #[must_use]
    pub fn header(mut self, key: SimpleHeader, value: impl Into<String>) -> Self {
        self.headers.entry(key).or_default().push(value.into());
        self
    }

    #[must_use]
    pub fn add_header(mut self, key: impl Into<SimpleHeader>, value: impl Into<String>) -> Self {
        self.headers
            .entry(key.into())
            .or_default()
            .push(value.into());
        self
    }

    #[must_use]
    pub fn headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = headers;
        self
    }

    #[must_use]
    pub fn body_text(mut self, text: impl Into<String>) -> Self {
        let text_string = text.into();
        let content_length = text_string.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["text/plain".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(text_string));
        self
    }

    #[must_use]
    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        let content_length = bytes.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["application/octet-stream".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Bytes(bytes));
        self
    }

    pub fn body_json<T: Serialize>(mut self, value: &T) -> Result<Self, HttpClientError> {
        let json_string =
            serde_json::to_string(value).map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;
        let content_length = json_string.len().to_string();

        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/json".to_string()],
        );
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(json_string));
        Ok(self)
    }

    #[must_use]
    pub fn body_form(mut self, params: &[(String, String)]) -> Self {
        fn urlencode(s: &str) -> String {
            s.chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    ' ' => "+".to_string(),
                    _ => {
                        let bytes = c.to_string().into_bytes();
                        bytes
                            .iter()
                            .fold(String::new(), |acc, b| format!("{acc:}%{b:02X}"))
                    }
                })
                .collect()
        }

        let form_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let content_length = form_string.len().to_string();

        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/x-www-form-urlencoded".to_string()],
        );
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(form_string));
        self
    }

    /// Set the request body to any `SendSafeBody` variant.
    ///
    /// Unlike `body_text` / `body_bytes` / `body_json`, this does not
    /// auto-set `Content-Type` or `Content-Length` — the caller is
    /// responsible for headers when using raw body variants (streams,
    /// chunked, SSE, etc.).
    #[must_use]
    pub fn body(mut self, body: SendSafeBody) -> Self {
        self.body = Some(body);
        self
    }

    pub fn get(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::GET, url)
    }

    pub fn post(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::POST, url)
    }

    pub fn put(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PUT, url)
    }

    pub fn delete(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::DELETE, url)
    }

    pub fn patch(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PATCH, url)
    }

    pub fn head(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::HEAD, url)
    }

    pub fn options(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::OPTIONS, url)
    }

    #[must_use]
    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        let credentials = format!("{username}:{password}");
        let encoded = BASE64_STANDARD.encode(credentials.as_bytes());
        self.header(SimpleHeader::AUTHORIZATION, format!("Basic {encoded}"))
    }

    #[must_use]
    pub fn basic_auth_opt(self, username: &str, password: Option<&str>) -> Self {
        self.basic_auth(username, password.unwrap_or(""))
    }

    #[must_use]
    pub fn bearer_token(self, token: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
    }

    #[must_use]
    pub fn bearer_auth(self, token: &str) -> Self {
        self.bearer_token(token)
    }

    #[must_use]
    pub fn api_key(self, header_name: &str, key: &str) -> Self {
        self.add_header(header_name.to_string(), key.to_string())
    }

    #[must_use]
    pub fn x_api_key(self, key: &str) -> Self {
        self.api_key("X-API-Key", key)
    }

    #[must_use]
    pub fn authorization(self, scheme: &str, credentials: &str) -> Self {
        self.header(
            SimpleHeader::AUTHORIZATION,
            format!("{scheme} {credentials}"),
        )
    }
}
