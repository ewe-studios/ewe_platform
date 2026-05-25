//! High-level HTTP client with clean public API — native (wasm32-gated).
//!
//! WHY: Provides user-facing API that hides all `TaskIterator` complexity.
//! Users interact with a simple, ergonomic interface.
//!
//! WHAT: Implements `SimpleHttpClient` with generic DNS resolver parameter,
//! configurable timeouts/redirects/pooling, and convenience methods for all HTTP verbs.
//!
//! HOW: Wraps `ClientRequestBuilder` and `TaskIterator` execution. Builder pattern
//! for configuration. Generic type parameter for DNS resolver flexibility.

use crate::wire::simple_http::client::{
    ClientConfig, ClientRequest, ClientRequestBuilder, ConnectionPool, DnsResolver,
    HttpConnectionPool, MiddlewareChain, SystemDnsResolver,
};
use crate::wire::simple_http::timeout::TimeoutCalculator;
use crate::wire::simple_http::HttpClientError;
use std::sync::Arc;
use std::time::Duration;

/// High-level HTTP client with clean API.
///
/// WHY: Provides user-facing API that completely hides `TaskIterator` complexity.
/// Users work with simple methods like `.get(url).send()`.
///
/// WHAT: Generic HTTP client with pluggable DNS resolver. Supports all HTTP methods,
/// configurable timeouts/redirects, optional connection pooling.
///
/// HOW: Wraps `ClientRequestBuilder` and `TaskIterator` execution. Builder pattern
/// for configuration. Generic type parameter for DNS resolver flexibility.
///
/// # Type Parameters
///
/// * `R` - DNS resolver type implementing `DnsResolver` trait. Defaults to `SystemDnsResolver`.
pub struct SimpleHttpClient<R: DnsResolver = SystemDnsResolver> {
    config: ClientConfig,
    pool: Option<Arc<HttpConnectionPool<R>>>,
    middleware_chain: Arc<MiddlewareChain>,
}

impl SimpleHttpClient<SystemDnsResolver> {
    #[must_use]
    pub fn from_system() -> Self {
        Self::new(
            ClientConfig::default(),
            Arc::new(HttpConnectionPool::default()),
        )
    }
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    #[must_use]
    pub fn new(config: ClientConfig, pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            config,
            pool: Some(pool),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }

    #[must_use]
    pub fn middleware(mut self, chain: MiddlewareChain) -> Self {
        self.middleware_chain = Arc::new(chain);
        self
    }
}

impl<R: DnsResolver + Default> Default for SimpleHttpClient<R> {
    fn default() -> Self {
        Self::new(
            ClientConfig::default(),
            Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                R::default(),
            )),
        )
    }
}

impl<R: DnsResolver + Clone> Clone for SimpleHttpClient<R> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool: self.pool.clone(),
            middleware_chain: self.middleware_chain.clone(),
        }
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn client_config(&self) -> ClientConfig {
        self.config.clone()
    }

    #[must_use]
    pub fn client_pool(&self) -> Option<Arc<HttpConnectionPool<R>>> {
        self.pool.clone()
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver,
            ))),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }
}

impl<R: DnsResolver + Default + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn with_connection_pool(mut self) -> Self {
        self.pool = Some(Arc::new(HttpConnectionPool::new(
            ConnectionPool::default(),
            R::default(),
        )));
        self
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn with_tls_connector(mut self, connector: crate::netcap::ssl::SSLConnector) -> Self {
        if let Some(pool) = self.pool.take() {
            let pool_inner = Arc::try_unwrap(pool).unwrap_or_else(|arc| (*arc).clone());
            self.pool = Some(Arc::new(pool_inner.with_tls_connector(connector)));
        }
        self
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    pub fn get(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::get(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn post(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::post(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn put(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::put(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn delete(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::delete(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn patch(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::patch(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn head(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::head(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    pub fn options(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::options(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    #[must_use]
    pub fn with_pool(pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            pool: Some(pool),
            config: ClientConfig::default(),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }

    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = config;
        self
    }

    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.config.timeout_calculator.config();
        config.connect_timeout = timeout;
        self.config.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.config.timeout_calculator.config();
        config.min_read_timeout = timeout;
        self.config.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.config.timeout_calculator.config();
        config.write_timeout_per_kb = timeout;
        self.config.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.config.max_redirects = max;
        self
    }

    pub fn proxy(mut self, proxy_url: &str) -> Result<Self, HttpClientError> {
        use crate::wire::simple_http::client::shared::proxy::ProxyConfig;
        let proxy_config = ProxyConfig::parse(proxy_url)?;
        self.config.proxy = Some(proxy_config);
        Ok(self)
    }

    #[must_use]
    pub fn proxy_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        use crate::wire::simple_http::client::shared::proxy::ProxyAuth;
        if let Some(ref mut proxy) = self.config.proxy {
            proxy.auth = Some(ProxyAuth::new(username, password));
        }
        self
    }

    #[must_use]
    pub fn proxy_from_env(mut self) -> Self {
        self.config.proxy_from_env = true;
        self
    }

    #[must_use]
    pub fn max_body_size(mut self, max_body_size: Option<u64>) -> Self {
        self.config.max_body_size = max_body_size;
        self
    }

    #[must_use]
    pub fn full_body_threshold(mut self, threshold: u64) -> Self {
        self.config.full_body_threshold = threshold;
        self
    }

    #[must_use]
    pub fn batch_size(mut self, batch_size: usize) -> Self {
        self.config.batch_size = batch_size;
        self
    }

    #[must_use]
    pub fn max_retries(mut self, max_retries: usize) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    #[must_use]
    pub fn preserve_auth_on_redirect(mut self, preserve: bool) -> Self {
        self.config.preserve_auth_on_redirect = preserve;
        self
    }

    #[must_use]
    pub fn preserve_cookies_on_redirect(mut self, preserve: bool) -> Self {
        self.config.preserve_cookies_on_redirect = preserve;
        self
    }

    pub fn request(
        &self,
        builder: ClientRequestBuilder<R>,
    ) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = builder.build()?;
        let pool = self.pool.clone().ok_or(HttpClientError::NoPool)?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            pool,
            self.middleware_chain.clone(),
        ))
    }
}
