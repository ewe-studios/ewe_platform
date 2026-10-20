//! HTTP/2 proxy — terminates H2 from clients, forwards to backends.
//!
//! WHY: gRPC (and Connect-over-gRPC) requires HTTP/2 *end-to-end* — a proxy that
//! downgrades H2→H1, strips `te: trailers`, or drops trailing HEADERS silently
//! breaks it (the gRPC status rides `grpc-status` trailers, and grpc-go treats a
//! stream that ends without them as an error). Plain HTTP/2 requests, by
//! contrast, are served fine by the existing HTTP/1.1 upstream path.
//!
//! WHAT: Two upstream legs, chosen by request content type. gRPC requests
//! (`application/grpc` / `application/grpc+*`) are forwarded over HTTP/2 via
//! connectrpc's [`H2Transport`] — the only H2 client in the tree that streams
//! bodies and surfaces trailing HEADERS — with `te: trailers` preserved and the
//! response trailers relayed as an `H2Frame::Trailers`. Everything else keeps the
//! existing `NativeHttpClient` HTTP/1.1 path.
//!
//! HOW: Both legs route and pick a backend on the valtron worker. The H1 leg's
//! upstream exchange is blocking, so it runs on a dedicated OS thread as before.
//! The H2 leg is async all the way: two relays run concurrently — request DATA
//! frames into the transport's request body, and the transport's head/body/
//! trailers back out as response frames — so a streaming call makes progress in
//! both directions. No whole-message buffering, no hand-rolled framing.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{PipeReceiver, PipeSender, Stream, TryRecvError, TrySendError};
use foundation_netio::http::ClientRequestBuilder;
use foundation_netio::http2::frame::ErrorCode;
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{
    pushable_request_body, Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod,
    SimpleUrl,
};

use foundation_http::native::serve::{BoxFuture, H2Serve};
use foundation_http::shared::context::ContextBag;

use foundation_connectrpc::shared::transport::{BodyStream, HeadStream, SendBody};
use foundation_connectrpc::{H2Transport, Transport};
use futures_util::StreamExt;

use crate::forward::{forward_request_headers, strip_hop_by_hop, upstream_url, SharedHttpClient};
use crate::runtime::BackendLease;
use crate::state::ProxyState;

/// How long a full pipe / empty pipe is polled before retrying on the blocking
/// forward path. Small enough to add negligible latency, large enough to avoid a
/// hot spin while the concurrent connection task drains the other end.
const POLL_BACKOFF: Duration = Duration::from_millis(1);

pub struct H2ProxyHandler {
    state: Arc<ProxyState>,
}

impl H2ProxyHandler {
    #[must_use]
    pub fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }
}

impl H2Serve for H2ProxyHandler {
    fn serve_h2(
        &self,
        _bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let host = header.authority.clone();
        let path = header.url.url.clone();
        let method = header.method.clone();
        let orig_headers = header.headers.clone();
        let client_ip = header
            .connection
            .peer_addr
            .as_ref()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let scheme = self.state.scheme().to_string();
        let client = self.state.client().clone();
        let state = self.state.clone();

        Box::pin(async move {
            // `PipeSender::send` is a future — dropping it (a bare `let _ = tx.send(..)`)
            // sends nothing and the client waits forever, so await these.
            let Some(service) = state.router().route(&host, &path) else {
                let _ = tx.send(h2_status(404, true)).await;
                return Ok(());
            };
            let Some((lease, _idx)) = service.pick_sticky(None) else {
                let _ = tx.send(h2_status(503, true)).await;
                return Ok(());
            };

            if is_grpc_request(&orig_headers) {
                // gRPC: HTTP/2 end-to-end, no downgrade. This leg is async all the
                // way (streaming relays) and runs on the worker; trailers flow back
                // through `tx` as an `H2Frame::Trailers`. `host`/`client` are unused.
                forward_upstream_h2(
                    lease, method, path, orig_headers, client_ip, scheme, body, &tx,
                )
                .await;
            } else {
                // The upstream exchange blocks (the shared client drives its own I/O),
                // so run it off the worker pool on a dedicated thread. The lease moves
                // in so the backend's in-flight count is held for the whole exchange.
                // The future returns immediately: the response flows back through `tx`,
                // which the h2 connection task drains and writes to the socket.
                std::thread::spawn(move || {
                    forward_upstream(
                        lease, method, host, path, orig_headers, client_ip, scheme, client, body,
                        &tx,
                    );
                });
            }
            Ok(())
        })
    }
}

/// Forward one H2 stream to its backend over HTTP/1.1 via the shared client and
/// relay the response back as H2 frames. Runs on a dedicated OS thread.
#[allow(clippy::too_many_arguments)]
fn forward_upstream(
    lease: BackendLease,
    method: SimpleMethod,
    host: String,
    path: String,
    orig_headers: foundation_netio::shared::http::SimpleHeaders,
    client_ip: String,
    scheme: String,
    client: SharedHttpClient,
    body: PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) {
    let url = upstream_url(lease.backend().url(), &path);

    // Hop-by-hop stripped + forwarding headers added, identical to the H1 path.
    // The H2 `:authority` becomes the upstream `Host`.
    let mut headers = forward_request_headers(&orig_headers, &client_ip, &scheme);
    headers.insert(SimpleHeader::HOST, vec![host]);

    // Stream the request body: H2 DATA frames feed a pushable body that the
    // client pulls as it renders the request. A dedicated feeder thread keeps the
    // producer (this pipe) and the consumer (the client's request renderer)
    // running concurrently.
    let (pushable, body_stream) = pushable_request_body();
    let feeder = std::thread::spawn(move || {
        loop {
            match body.try_recv() {
                Ok(H2IncomingFrame::Data(bytes)) => {
                    let mut chunk = bytes;
                    loop {
                        match pushable.try_push(chunk) {
                            Ok(()) => break,
                            Err(TrySendError::Full(returned)) => {
                                chunk = returned;
                                std::thread::sleep(POLL_BACKOFF);
                            }
                            Err(TrySendError::Closed(_)) => return,
                        }
                    }
                }
                Ok(H2IncomingFrame::Reset(_)) => break,
                Err(TryRecvError::Empty) => std::thread::sleep(POLL_BACKOFF),
                Err(TryRecvError::Closed) => break,
            }
        }
        pushable.close();
    });

    let builder = match ClientRequestBuilder::<SystemDnsResolver>::new(method, &url) {
        Ok(b) => b.headers(headers).body(body_stream),
        Err(e) => {
            tracing::warn!(%url, "H2 build upstream request failed: {e}");
            let _ = blocking_send(tx, h2_status(502, true));
            let _ = feeder.join();
            return;
        }
    };

    let response = match client.request(builder).and_then(|req| req.send()) {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(%url, "H2 upstream request failed: {e}");
            let _ = blocking_send(tx, h2_status(502, true));
            let _ = feeder.join();
            return;
        }
    };

    let (status, resp_headers, resp_body, pool, conn) = response.into_parts();

    // Response HEADERS: strip hop-by-hop, then hand every value to the encoder,
    // which adds `:status` from the numeric code.
    let out_headers = strip_hop_by_hop(&resp_headers);
    let h2_hdrs = simple_headers_to_h2(&out_headers);
    let code = u16::try_from(Into::<usize>::into(status)).unwrap_or(502);
    if blocking_send(
        tx,
        H2Frame::Headers {
            status: code,
            headers: h2_hdrs,
            end_stream: false,
        },
    )
    .is_err()
    {
        let _ = feeder.join();
        return;
    }

    // Response body → H2 DATA frames, chunk at a time.
    let mut chunks = SendSafeBodyBytesIterator::new(resp_body);
    loop {
        match chunks.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                if bytes.is_empty() {
                    continue;
                }
                if blocking_send(
                    tx,
                    H2Frame::Data {
                        payload: bytes,
                        end_stream: false,
                    },
                )
                .is_err()
                {
                    let _ = feeder.join();
                    return;
                }
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                tracing::warn!(%url, "H2 upstream body stream error: {e}");
                break;
            }
            // Honour an explicit delay hint; every other non-data signal
            // (`Ignore`/`Wait`/`Init`/…) is transient "no data yet" — back off.
            Some(Stream::Delayed(d)) => std::thread::sleep(d),
            Some(_) => std::thread::sleep(POLL_BACKOFF),
            None => break,
        }
    }

    // Terminate the response stream with an empty END_STREAM DATA frame.
    let _ = blocking_send(
        tx,
        H2Frame::Data {
            payload: Bytes::new(),
            end_stream: true,
        },
    );

    // Return the upstream socket to the pool for reuse (drain residue first),
    // mirroring the H1 forward path.
    if let (Some(pool), Some(mut stream)) = (pool, conn) {
        stream.drain_stream();
        pool.return_to_pool(stream);
    }

    let _ = feeder.join();
    drop(lease);
}

/// Whether a request is a gRPC (or Connect-over-gRPC) call, by `Content-Type`.
///
/// gRPC requires HTTP/2 end-to-end; a proxy that downgrades it to HTTP/1.1 breaks
/// it (no trailing HEADERS, so `grpc-status` is lost). We detect the protocol by
/// the `application/grpc` / `application/grpc+*` media type. `application/grpc-web`
/// is deliberately excluded: gRPC-Web is HTTP/1.1-transparent and is not the H2
/// end-to-end shape this leg serves.
fn is_grpc_request(headers: &SimpleHeaders) -> bool {
    headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|values| values.first())
        .is_some_and(|ct| ct == "application/grpc" || ct.starts_with("application/grpc+"))
}

/// Forward one H2 stream to its backend over HTTP/2 (no downgrade) via connectrpc's
/// [`H2Transport`], relaying the response head, body, and trailing HEADERS back as
/// H2 frames. Runs on the valtron worker — async all the way down.
///
/// The request body is streamed into the transport's request half and the response
/// is drained back out concurrently, so a streaming/bidi call makes progress in
/// both directions without buffering the whole message.
#[allow(clippy::too_many_arguments)]
async fn forward_upstream_h2(
    lease: BackendLease,
    method: SimpleMethod,
    path: String,
    orig_headers: SimpleHeaders,
    client_ip: String,
    scheme: String,
    body: PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) {
    let url = upstream_url(lease.backend().url(), &path);

    // Same hop-by-hop + forwarding-header policy as the H1 path.
    // `forward_request_headers` already omits `Connection: close` for gRPC
    // (`is_streaming_request`), and H2 has no hop-by-hop close anyway;
    // `:authority` comes from the URI, so there is no `Host` header to synthesize.
    let headers = forward_request_headers(&orig_headers, &client_ip, &scheme);

    let uri = match Uri::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(%url, "H2 upstream URI parse failed: {e}");
            let _ = tx.send(h2_status(502, true)).await;
            return;
        }
    };

    let request = RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(&url),
        request_uri: uri,
        headers,
        method,
    };

    let stream = match H2Transport::new().open(request) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(%url, "H2 upstream open failed: {e}");
            let _ = tx.send(h2_status(502, true)).await;
            return;
        }
    };

    let send_body = stream.send_body;
    let head = stream.head;
    let recv_body = stream.recv_body;
    let trailers = stream.trailers;

    futures_util::future::join(
        request_relay(body, send_body),
        response_relay(tx, head, recv_body, trailers, &url),
    )
    .await;

    drop(lease);
}

/// Stream the client's H2 DATA frames into the upstream request body, then close
/// the request direction. Returns early if the upstream body pipe closes first
/// (the backend stopped reading).
async fn request_relay(body: PipeReceiver<H2IncomingFrame>, send_body: Arc<dyn SendBody>) {
    while let Some(frame) = body.receive().await {
        match frame {
            H2IncomingFrame::Data(bytes) => {
                if bytes.is_empty() {
                    continue;
                }
                if send_body.send_async(bytes).await.is_err() {
                    return;
                }
            }
            H2IncomingFrame::Reset(_) => break,
        }
    }
    send_body.close();
}

/// Relay the upstream response head (skipping interim `1xx`), body DATA frames,
/// and trailing HEADERS back through `tx`. A stream that ends with no trailers is
/// terminated with an empty `END_STREAM` DATA frame, mirroring the H1 path.
async fn response_relay(
    tx: &PipeSender<H2Frame>,
    mut head: HeadStream,
    mut recv_body: BodyStream,
    trailers: PipeReceiver<SimpleHeaders>,
    url: &str,
) {
    // Head: skip interim `1xx` responses, forward the final head.
    let (status, resp_headers) = loop {
        match head.next().await {
            Some(Ok((status, headers))) => {
                if u16::try_from(Into::<usize>::into(status.clone())).unwrap_or(0) < 200 {
                    continue;
                }
                break (status, headers);
            }
            Some(Err(e)) => {
                tracing::warn!(%url, "H2 upstream head error: {e}");
                let _ = tx.send(h2_status(502, true)).await;
                return;
            }
            None => {
                let _ = tx.send(h2_status(502, true)).await;
                return;
            }
        }
    };

    let code = u16::try_from(Into::<usize>::into(status)).unwrap_or(502);
    if tx
        .send(H2Frame::Headers {
            status: code,
            headers: simple_headers_to_h2(&strip_hop_by_hop(&resp_headers)),
            end_stream: false,
        })
        .await
        .is_err()
    {
        return;
    }

    // Body → H2 DATA frames, chunk at a time.
    while let Some(chunk) = recv_body.next().await {
        match chunk {
            Ok(bytes) => {
                if bytes.is_empty() {
                    continue;
                }
                if tx
                    .send(H2Frame::Data {
                        payload: bytes,
                        end_stream: false,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(e) => {
                tracing::warn!(%url, "H2 upstream body error: {e}");
                let _ = tx
                    .send(H2Frame::Reset {
                        error_code: ErrorCode::InternalError,
                    })
                    .await;
                return;
            }
        }
    }

    // Trailers → `H2Frame::Trailers`, else a clean empty END_STREAM DATA frame.
    match trailers.receive().await {
        Some(t) if !t.is_empty() => {
            let _ = tx
                .send(H2Frame::Trailers {
                    headers: simple_headers_to_h2(&t),
                })
                .await;
        }
        _ => {
            let _ = tx
                .send(H2Frame::Data {
                    payload: Bytes::new(),
                    end_stream: true,
                })
                .await;
        }
    }
}

/// Render `headers` as HPACK-ready `(name, value)` byte pairs for an `H2Frame`.
fn simple_headers_to_h2(headers: &SimpleHeaders) -> Vec<(Bytes, Bytes)> {
    let mut out: Vec<(Bytes, Bytes)> = Vec::new();
    for (name, values) in headers.iter() {
        let nb = Bytes::from(name.to_string().into_bytes());
        for value in values {
            out.push((nb.clone(), Bytes::from(value.clone().into_bytes())));
        }
    }
    out
}

/// Push one response frame into the pipe from the blocking forward thread.
///
/// `PipeSender::send` is async and cannot be awaited here, so use `try_send` and
/// park the thread briefly while the bounded pipe is full — the h2 connection
/// task drains `resp_rx` concurrently, so a full pipe is always transient.
/// Returns `Err(())` once the receiver is gone (the client stream closed).
fn blocking_send(tx: &PipeSender<H2Frame>, frame: H2Frame) -> Result<(), ()> {
    let mut item = frame;
    loop {
        match tx.try_send(item) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                item = returned;
                std::thread::sleep(POLL_BACKOFF);
            }
            Err(TrySendError::Closed(_)) => return Err(()),
        }
    }
}

/// A status-only response frame (no headers, optionally ending the stream).
pub fn h2_status(code: u16, end_stream: bool) -> H2Frame {
    H2Frame::Headers {
        status: code,
        headers: vec![],
        end_stream,
    }
}
