//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton; F45 Part D seam shrink).
//!
//! WHY: `Transport` byte-level client-seam contract.
//! WHAT: `open()` creates an `HttpExchangeTask` (native: wraps `SendRequestTask`)
//! and fans its `HttpExchange` output out with valtron's native splits — the head
//! into a `HeadStream`, the body into a `BodyStream` — spawns the drive task on the
//! valtron pool via `valtron::send()`, and returns `TransportStream` synchronously.
//! HOW: `PreparedRequest` → `HttpExchangeTask::new()` →
//! `.split_collect_until_map(head)` → `.split_collector_map(body)` →
//! `body_cont.map_ready(|_| ())` → `valtron::send()`; each observer is bridged into
//! its erased stream via `into_next_stream()`. `send_body` is the only `Pipe`.

use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_core::valtron::{self, CollectionState, StreamIteratorExt, TaskIteratorExt};
use foundation_netio::simple_http::client::shared::request_task::HttpExchange;
use foundation_netio::simple_http::client::shared::PreparedRequest;
use foundation_netio::simple_http::client::{HttpExchangeTask, SimpleHttpClient};
use foundation_netio::simple_http::shared::Extensions;
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor,
    DEFAULT_PUSHABLE_DEPTH,
};

use super::{
    BodyStream, ByteSink, HeadStream, Transport, TransportCapabilities, TransportError,
    TransportStream,
};

#[derive(Clone, Default)]
pub struct H1Transport {
    client: Arc<SimpleHttpClient>,
}

impl H1Transport {
    #[must_use]
    pub fn new(client: SimpleHttpClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl Transport for H1Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            full_duplex: false,
            h2_trailers: false,
            http_versions: &[Proto::HTTP11],
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let url_str = request_url_string(&request);
        let method = request.method.clone();
        let headers = request.headers.clone();

        let (pushable, body_stream) = pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
        let send_body: ByteSink = pushable.into_sender();

        let uri = Uri::parse(&url_str).map_err(|e| {
            http_to_transport_error(HttpClientError::Reason(format!("invalid URL: {e}")))
        })?;
        let prepared = PreparedRequest {
            method,
            url: uri,
            headers,
            body: body_stream,
            extensions: Extensions::new(),
        };

        let pool = self.client.client_pool().ok_or_else(|| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no connection pool configured",
            )))
        })?;
        let config = self.client.client_config();

        let pump = HttpExchangeTask::new(prepared, config.max_redirects, pool, config);

        // Peel the response head (or a *pre-head* failure) into an observer whose
        // item is `Result<(Status, SimpleHeaders), TransportError>`, closing after
        // the first; the continuation carries body chunks (F45 Part D). A `Failed`
        // rides the payload as `Err` — no silent drop (the original F23 objection).
        let (head_obs, head_cont) = pump.split_collect_until_map(
            |item: &HttpExchange| match item {
                HttpExchange::Head { status, headers } => (
                    CollectionState::Close(true),
                    Some(Ok((status.clone(), headers.clone()))),
                ),
                HttpExchange::Failed(err) => (
                    CollectionState::Close(true),
                    Some(Err(failed_to_transport_error(err))),
                ),
                HttpExchange::BodyChunk(_) => (CollectionState::Skip, None),
            },
            1,
        );

        // Body chunks (or a *mid-body* failure) into a second observer whose item
        // is `Result<Bytes, TransportError>`.
        let (body_obs, body_cont) = head_cont.split_collector_map(
            |item: &HttpExchange| match item {
                HttpExchange::BodyChunk(bytes) => (true, Some(Ok(bytes.clone()))),
                HttpExchange::Failed(err) => (true, Some(Err(failed_to_transport_error(err)))),
                HttpExchange::Head { .. } => (false, None),
            },
            DEFAULT_PUSHABLE_DEPTH,
        );

        // Drive task: the final continuation still yields the original `HttpExchange`
        // values, so terminate it with `map_ready(|_| ())` before spawning — body
        // chunks must not be buffered a second time into an undrained delivery queue
        // (F45 Resolution 8). This is the only task spawned; the two observers are
        // drained by the caller through the erased `HeadStream`/`BodyStream`.
        let drive = body_cont.map_ready(|_| ());
        valtron::send(drive).map_err(|e| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        // Bridge the split observers into the erased FutureStream handles. Any
        // `StreamIterator` (a split observer here) becomes a `Stream` via the
        // `StreamIteratorExt` adapters, so the split never leaks past
        // `TransportStream` (F45 Part D, design D1). The head split's
        // `CollectionState::Close(true)` closes the head observer once the head is
        // delivered, so `head` yields the one head then ends.
        let head: HeadStream = Box::pin(head_obs.into_next_stream());
        let recv_body: BodyStream = Box::pin(body_obs.into_next_stream());

        Ok(TransportStream {
            send_body,
            head,
            recv_body,
        })
    }
}

/// Map an `HttpExchange::Failed` payload into a [`TransportError`] (F45 Part D).
///
/// WHY: a request/response failure carries an arbitrary `dyn Error`; `Connect` is
/// the only `TransportError` variant that preserves an arbitrary error object, and
/// it maps to `Code::Unavailable` — the correct code for a transport-level failure.
/// HOW: shares the `Arc` (no stringification, no detail lost).
///
/// # Panics
/// Never panics.
fn failed_to_transport_error(
    err: &Arc<dyn std::error::Error + Send + Sync + 'static>,
) -> TransportError {
    TransportError::Connect(Arc::clone(err))
}

fn request_url_string(req: &RequestDescriptor) -> String {
    let query = req
        .request_uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    format!(
        "http://{}:{}{}{}",
        req.request_uri
            .host_str()
            .unwrap_or_else(|| "localhost".to_string()),
        req.request_uri.port_or_default(),
        req.request_uri.path(),
        query,
    )
}

fn http_to_transport_error(e: HttpClientError) -> TransportError {
    TransportError::Connect(Arc::new(e))
}
