//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton).
//!
//! WHY: `Transport` byte-level client-seam contract.
//! WHAT: `open()` creates an `HttpExchangeTask` (native: wraps `SendRequestTask` via
//! `inlined_task`), maps its output into caller-facing pipes via `map_ready`, spawns on
//! the valtron pool via `valtron::send()`, returns `TransportStream` synchronously.
//! HOW: `PreparedRequest` → `HttpExchangeTask::new()` → `.map_ready(|item| match item {
//! Head → head_tx, BodyChunk → recv_tx, Failed → done })` → `valtron::send()`.

use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_core::valtron::{self, TaskIteratorExt};
use foundation_netio::simple_http::client::shared::request_task::HttpExchange;
use foundation_netio::simple_http::client::shared::{PreparedRequest, SystemDnsResolver};
use foundation_netio::simple_http::client::{HttpExchangeTask, SimpleHttpClient};
use foundation_netio::simple_http::shared::Extensions;
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor, SimpleHeaders,
    SimpleMethod, Status, DEFAULT_PUSHABLE_DEPTH,
};

use super::{
    ByteSink, ByteSource, HeadSource, Transport, TransportCapabilities, TransportError,
    TransportStream,
};

#[derive(Clone)]
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

        let (head_tx, head_rx): (
            foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
            HeadSource,
        ) = foundation_core::valtron::Pipe::with_depth(1);
        let (recv_tx, recv_body): (ByteSink, ByteSource) =
            foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

        tracing::info!("GET request to {} and retrieving pool", url_str);
        let pool = self.client.client_pool().ok_or_else(|| {
            TransportError::Connect(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no connection pool configured",
            )))
        })?;
        let config = self.client.client_config();

        tracing::info!("Creating HttpExchangeTask for {}", url_str);
        let pump = HttpExchangeTask::new(prepared, config.max_redirects, pool, config).map_ready(
            move |item| match item {
                HttpExchange::Head { status, headers } => {
                    tracing::info!("Received head: status={:?}, headers={:?}", status, headers);
                    let _ = head_tx.try_send((status, headers));
                }
                HttpExchange::BodyChunk(bytes) => {
                    tracing::info!("Received body chunk: bytes={:?}", bytes);
                    let _ = recv_tx.try_send(bytes);
                }
                HttpExchange::Failed(err) => {
                    tracing::error!("Failed to send request: {err:?}");
                }
            },
        );

        tracing::info!("Sending task for {} for execution", url_str);
        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        Ok(TransportStream {
            send_body,
            head: head_rx,
            recv_body,
        })
    }
}

fn request_url_string(req: &RequestDescriptor) -> String {
    format!(
        "http://{}:{}{}",
        req.request_uri
            .host_str()
            .unwrap_or_else(|| "localhost".to_string()),
        req.request_uri.port_or_default(),
        req.request_uri.path()
    )
}

fn http_to_transport_error(e: HttpClientError) -> TransportError {
    TransportError::Connect(Box::new(e))
}
