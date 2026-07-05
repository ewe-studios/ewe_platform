//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton).
//!
//! WHY: The `Transport` trait defines the byte-level client-seam contract. The
//! HTTP/1.1 implementation is the first concrete transport — it proves the seam (the
//! caller pushes request bytes into `send_body` and receives response bytes from
//! `recv_body`) and is the client connection-owner: `open()` spawns the request-drive
//! so the pushable request-body pipe is drained concurrently with the caller's pushes
//! (half-duplex deadlock avoidance, Decision 11).
//!
//! WHAT: [`H1Transport`] — wraps a netio `SimpleHttpClient` and implements
//! `Transport`.
//!
//! HOW: A spawned valtron task calls `ClientRequest::send_async()`, which internally
//! spawns the network I/O, drives the intro + body streams to completion, and
//! collects the full response. The task then feeds the response head into a oneshot
//! pipe and drains the body into the recv-body pipe. The caller receives
//! `TransportStream` immediately and can push body bytes concurrently.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron;
use foundation_netio::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor, SendSafeBody,
    SimpleHeaders, SimpleResponse, Status, DEFAULT_PUSHABLE_DEPTH,
};

use super::{
    BoxFuture, ByteSink, ByteSource, Transport, TransportCapabilities, TransportError,
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
    ) -> BoxFuture<'static, Result<TransportStream, TransportError>> {
        let url_str = request_url_string(&request);
        let client = self.client.clone();
        let method = request.method.clone();
        let headers = request.headers.clone();

        Box::pin(async move {
            // 1. Create pushable request body → extract sender as send_body.
            let (pushable, body_stream) = pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
            let send_body: ByteSink = pushable.into_sender();

            // 2. Build and send through the configured client (inherits its pool,
            //    config, and middleware).
            let builder = ClientRequestBuilder::new(method, &url_str)
                .map_err(http_to_transport_error)?
                .body(body_stream)
                .headers(headers);

            let client_req = client.request(builder).map_err(http_to_transport_error)?;

            // 3. Channels: a 1-element Pipe for the response head (oneshot), a
            //    regular-depth Pipe for the response body.
            let (head_tx, head_rx) =
                foundation_core::valtron::Pipe::<(Status, SimpleHeaders)>::with_depth(1);
            let (recv_tx, recv_body): (ByteSink, ByteSource) =
                foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

            // 4. Spawn valtron task: drive send_async() → drain head + body into
            //    the pipes.
            let drain = valtron::from_future(drive_and_drain(client_req, head_tx, recv_tx));
            let _ = valtron::execute(drain, None);

            // 5. Return immediately — caller pushes body bytes while valtron task
            //    drains the request in parallel.
            Ok(TransportStream {
                send_body,
                response: Box::pin(async move {
                    let (status, headers) = head_rx.receive().await.ok_or(TransportError::Reset)?;
                    Ok(SimpleResponse::no_body(status, headers))
                }),
                recv_body,
            })
        })
    }
}

// ── valtron task ─────────────────────────────────────────────────────────────────

/// Spawned valtron task: drive `send_async()` to completion, then feed the response
/// head and body into their respective pipes.
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
