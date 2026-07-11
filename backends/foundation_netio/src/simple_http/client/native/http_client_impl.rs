use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{execute, StreamIteratorExt};

use crate::event_source::{ReconnectingEventSourceTask, ReconnectingProgress};
use crate::simple_http::client::shared::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient, SseProgress,
};
use crate::simple_http::client::shared::request::PreparedRequest;
use crate::simple_http::client::shared::request_task::HttpExchangeClientTask;
use crate::simple_http::client::shared::{BoxedDnsResolver, ClientConfig, DnsResolver, SystemDnsResolver};
use crate::simple_http::client::native::pool::ConnectionPool;
use crate::simple_http::client::native::tasks::HttpExchangeTask;
use crate::simple_http::client::{ClientRequestBuilder, HttpConnectionPool, SimpleHttpClient};
use crate::simple_http::shared::{
    HttpClientError, SendSafeBody, SimpleMethod, SimpleResponse,
};

pub struct NativeHttpClient<R: DnsResolver + Clone + Send + 'static = SystemDnsResolver> {
    client: SimpleHttpClient<R>,
    resolver: R,
}

impl Default for NativeHttpClient<SystemDnsResolver> {
    fn default() -> Self {
        Self {
            client: SimpleHttpClient::default(),
            resolver: SystemDnsResolver,
        }
    }
}

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    #[must_use]
    pub fn new(resolver: R) -> Self {
        Self {
            client: SimpleHttpClient::with_resolver(resolver.clone()),
            resolver,
        }
    }

    #[must_use]
    pub fn with_client(client: SimpleHttpClient<R>, resolver: R) -> Self {
        Self { client, resolver }
    }

    #[must_use]
    pub fn with_expect_continue_timeout(resolver: R, timeout: Duration) -> Self {
        let config = ClientConfig {
            expect_continue_read_timeout: timeout,
            ..ClientConfig::default()
        };
        let client = SimpleHttpClient::with_resolver(resolver.clone()).config(config);
        Self { client, resolver }
    }

    fn build_request(
        &self,
        req: &PreparedRequest,
    ) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        let url_str = req.url.to_string();
        let method_fn = match req.method {
            SimpleMethod::GET => SimpleHttpClient::get,
            SimpleMethod::POST => SimpleHttpClient::post,
            SimpleMethod::PUT => SimpleHttpClient::put,
            SimpleMethod::DELETE => SimpleHttpClient::delete,
            SimpleMethod::PATCH => SimpleHttpClient::patch,
            SimpleMethod::HEAD => SimpleHttpClient::head,
            SimpleMethod::OPTIONS => SimpleHttpClient::options,
            _ => return Err(HttpClientError::NotSupported),
        };

        let mut builder: ClientRequestBuilder<R> = method_fn(&self.client, &url_str)?;

        for (key, values) in &req.headers {
            for value in values {
                builder = builder.header(key.clone(), value.clone());
            }
        }

        Ok(builder)
    }

    fn build_sse_task(
        &self,
        req: &PreparedRequest,
    ) -> Result<ReconnectingEventSourceTask<R>, HttpClientError> {
        let url_str = req.url.to_string();
        let mut task = ReconnectingEventSourceTask::connect(self.resolver.clone(), &url_str)
            .map_err(|e| HttpClientError::Reason(format!("SSE connection failed: {e}")))?;

        for (key, values) in &req.headers {
            for value in values {
                task = task.with_header(key.clone(), value.clone());
            }
        }

        Ok(task)
    }
}

impl<R: DnsResolver + Clone + Send + 'static> Clone for NativeHttpClient<R> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            resolver: self.resolver.clone(),
        }
    }
}

#[async_trait::async_trait]
impl<R: DnsResolver + Clone + Default + Send + Sync + 'static> HttpClient for NativeHttpClient<R> {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let mut builder = self.build_request(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            builder = builder.body(req.body);
        }

        let request = self.client.request(builder)?;
        let response = request.send_async().await?;
        let (status, headers, body, _pool, _conn) = response.into_parts();

        Ok(SimpleResponse::new(status, headers, body))
    }

    async fn send_sse_async(
        &self,
        req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        let mut task = self.build_sse_task(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            task = task.with_body(req.body);
        }

        let driven = execute(task, None)
            .map_err(|e| HttpClientError::Reason(format!("SSE executor error: {e}")))?;

        let mapped = driven.map_pending(map_progress);
        let future_stream = mapped.into_future_stream();
        Ok(Box::pin(future_stream))
    }

    fn send(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let mut builder = self.build_request(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            builder = builder.body(req.body);
        }

        let request = self.client.request(builder)?;
        let response = request.send()?;
        let (status, headers, body, _pool, _conn) = response.into_parts();

        Ok(SimpleResponse::new(status, headers, body))
    }

    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        let mut task = self.build_sse_task(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            task = task.with_body(req.body);
        }

        let driven = execute(task, None)
            .map_err(|e| HttpClientError::Reason(format!("SSE executor error: {e}")))?;

        let mapped = driven.map_pending(map_progress);
        Ok(Box::new(mapped))
    }

    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask {
        let config = self.client.client_config();
        // Stage 1: create a fresh pool with a default system resolver erased to
        // `BoxedDnsResolver`. Stage 2 folds `SimpleHttpClient` into
        // `NativeHttpClient` and erases the resolver at construction, making the
        // pool shared across `open_exchange` and the existing surface.
        let resolver: BoxedDnsResolver = Arc::new(SystemDnsResolver::default());
        let pool = Arc::new(HttpConnectionPool::new(
            ConnectionPool::default(),
            resolver,
        ));
        let task = HttpExchangeTask::new(req, config.max_redirects, pool, config);
        HttpExchangeClientTask::native(task)
    }
}

fn map_progress(p: ReconnectingProgress) -> SseProgress {
    match p {
        ReconnectingProgress::Connecting => SseProgress::Connecting,
        ReconnectingProgress::Reading => SseProgress::Reading,
        ReconnectingProgress::Reconnecting => SseProgress::Reconnecting,
    }
}

#[must_use]
pub fn default_http_client() -> Arc<dyn HttpClient> {
    Arc::new(NativeHttpClient::<SystemDnsResolver>::default())
}
