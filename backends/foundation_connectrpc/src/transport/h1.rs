//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton).
//!
//! WHY: The `Transport` trait defines the byte-level client-seam contract. The
//! HTTP/1.1 implementation is the first concrete transport — it proves the seam (the
//! caller pushes request bytes into `send_body` and receives response bytes from
//! `recv_body`) and is the client connection-owner: `open()` spawns the byte pump on
//! the valtron pool and returns the three caller-facing pipe halves synchronously
//! (Decision 11 §Connection ownership).
//!
//! WHAT: [`H1Transport`] — wraps a netio `SimpleHttpClient` and implements
//! `Transport`. `open()` is synchronous — no `BoxFuture`, no `futures_lite::block_on`,
//! no OS threads. The pump is a valtron task (`from_future` + `valtron::send()`).
//!
//! HOW: The pump task calls `ClientRequest::send_async()` (which internally uses
//! `StreamReadyFuture` self-wake to drive the netio request/response streams). Once
//! `send_async()` completes, the remaining work (`head_tx.send`, `drain_body_into_pipe`)
//! all resolve in the same poll because the pipes have capacity. The caller receives
//! three caller-facing `Pipe` halves and polls them directly.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron::{self, from_future};
use foundation_netio::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor, SendSafeBody,
    SimpleHeaders, Status, DEFAULT_PUSHABLE_DEPTH,
};

use super::{
    ByteSink, ByteSource, HeadSource, Transport, TransportCapabilities, TransportError,
    TransportStream,
};

/// Native HTTP/1.1 transport — the client connection-owner.
#[derive(Clone)]
pub struct H1Transport {
    client: Arc<SimpleHttpClient>,
}

impl H1Transport {
    /// Create from a pre-configured `SimpleHttpClient`.
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

    fn open(
        &self,
        request: RequestDescriptor,
    ) -> Result<TransportStream, TransportError> {
        let url_str = request_url_string(&request);
        let method = request.method.clone();
        let headers = request.headers.clone();

        // 1. Create pushable request body → extract sender as send_body.
        let (pushable, body_stream) =
            pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
        let send_body: ByteSink = pushable.into_sender();

        // 2. Build the client request.
        let builder = ClientRequestBuilder::new(method, &url_str)
            .map_err(http_to_transport_error)?
            .body(body_stream)
            .headers(headers);

        let client_req = self
            .client
            .request(builder)
            .map_err(http_to_transport_error)?;

        // 3. Channels: a 1-slot Pipe for the response head, a regular-depth
        //    Pipe for the response body.
        let (head_tx, head_rx): (
            foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
            HeadSource,
        ) = foundation_core::valtron::Pipe::with_depth(1);
        let (recv_tx, recv_body): (ByteSink, ByteSource) =
            foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

        // 4. Spawn the pump on the valtron pool. It drives send_async() →
        //    feeds head and body into the pipes. Once send_async() completes,
        //    the remaining work (head send + body drain) resolves in the same
        //    poll because both pipes have capacity.
        let pump = from_future(drive_and_drain(client_req, head_tx, recv_tx));
        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        // 5. Return immediately — caller holds the caller-facing pipe halves;
        //    the pump task drains the socket-facing halves in parallel.
        Ok(TransportStream {
            send_body,
            head: head_rx,
            recv_body,
        })
    }
}

// ── pump task ────────────────────────────────────────────────────────────────────

/// Valton task: drive `send_async()` to completion, then feed the response head and
/// body into their respective pipes.
async fn drive_and_drain(
    client_req: foundation_netio::simple_http::client::ClientRequest<
        foundation_netio::simple_http::client::shared::SystemDnsResolver,
    >,
    head_tx: foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
    recv_tx: ByteSink,
) {
    match client_req.send_async().await {
        Ok(finalized) => {
            let (status, headers, body, _pool, _conn) = finalized.into_parts();
            let _ = head_tx.send((status, headers)).await;
            drain_body_into_pipe(body, recv_tx).await;
        }
        Err(_e) => {
            // send_async failed — pipes dropped, caller sees closed/empty pipes.
        }
    }
}

/// Drain a `SendSafeBody` into a `PipeSender<Bytes>`, then close.
async fn drain_body_into_pipe(body: SendSafeBody, tx: ByteSink) {
    use foundation_netio::simple_http::client::shared::body_reader::try_collect_bytes;
    let bytes = try_collect_bytes(body).unwrap_or_default();
    if !bytes.is_empty() {
        let _ = tx.send(Bytes::from(bytes)).await;
    }
    // tx dropped → pipe closed, caller sees EOF.
}

// ── misc helpers ─────────────────────────────────────────────────────────────────

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
