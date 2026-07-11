//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton; F45 Part D seam shrink; F51 Stage 3 de-leak).
//!
//! WHY: `Transport` byte-level client-seam contract. F51 de-leaked this module:
//! it no longer names `SimpleHttpClient`, `HttpExchangeTask`, or
//! `HttpConnectionPool` — the client owns its pool/resolver and exposes an
//! `open_exchange` task. This module is now platform-agnostic in its client
//! dependency (holds `Arc<dyn HttpClient>`).
//!
//! WHAT: `open()` calls `self.client.open_exchange(prepared)` to get a
//! `HttpExchangeClientTask`, then fans its `HttpExchange` output with valtron
//! splits — identical to before, just through the trait method.
//!
//! HOW: `PreparedRequest` → `self.client.open_exchange(prepared)` →
//! `.split_collect_until_map(head)` → `.split_collector_map(body)` →
//! `body_cont.map_ready(|_| ())` → `valtron::send()`.

use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_core::valtron::{self, CollectionState, Pipe, StreamIteratorExt, TaskIteratorExt};
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::Extensions;
use foundation_netio::shared::http::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor,
    SimpleHeaders, DEFAULT_PUSHABLE_DEPTH,
};
use foundation_netio::HttpClientBuilder;

use super::{
    BodyStream, ByteSink, HeadStream, Transport, TransportCapabilities, TransportError,
    TransportStream,
};

#[derive(Clone)]
pub struct H1Transport {
    client: Arc<dyn HttpClient>,
}

impl H1Transport {
    /// Create an H1 transport backed by any `HttpClient`.
    ///
    /// The transport is platform-agnostic — it holds an `Arc<dyn HttpClient>`
    /// and never names a concrete client type. Callers that want the platform
    /// default can use `H1Transport::default()`.
    #[must_use]
    pub fn new(client: Arc<dyn HttpClient>) -> Self {
        Self { client }
    }
}

impl Default for H1Transport {
    fn default() -> Self {
        // `HttpClientBuilder::build()` returns the platform's default
        // `Arc<dyn HttpClient>` (native pool client or wasm fetch client). The
        // transport stays free of any native/wasm cfg gating (F51 de-leak).
        Self::new(HttpClientBuilder::new().build())
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

        // F51: the client owns the pool, resolver, and config — the transport
        // just asks for the task. No more reaching into SimpleHttpClient internals.
        let pump = self.client.open_exchange(prepared);

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

        // HTTP/1.1 has no trailing headers — close immediately.
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        trailer_tx.close();

        Ok(TransportStream {
            send_body: Arc::new(send_body),
            head,
            recv_body,
            trailers: trailer_rx,
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
