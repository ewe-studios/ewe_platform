//! HTTP/2 proxy — terminates H2 from clients, forwards to backends over HTTP/1.1.
//!
//! WHY: `foundation_netio` already owns a complete HTTP/1.1 client — request
//! rendering, response parsing for every body framing (`Content-Length`,
//! chunked, and connection-close), and upstream connection pooling. The proxy
//! must not re-implement any of that. This module only bridges the two protocol
//! shapes: an H2 request header + body pipe becomes a `NativeHttpClient` request,
//! and the client's response becomes H2 response frames. It is the exact same
//! forwarding stack the HTTP/1.1 handler ([`crate::forward`]) uses.
//!
//! HOW: Routing and backend selection run on the valtron worker (cheap, async).
//! The upstream exchange itself is blocking (the shared client drives its own
//! I/O to completion), so it runs on a dedicated OS thread — one per stream —
//! keeping the worker pool free, exactly as the HTTP/1.1 handler does. The
//! request body streams in through a pushable body fed from the H2 DATA pipe; the
//! response body streams out through the response pipe as H2 DATA frames. No
//! whole-message buffering, no hand-rolled framing.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::{PipeReceiver, PipeSender, Stream, TryRecvError, TrySendError};
use foundation_netio::http::ClientRequestBuilder;
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{pushable_request_body, SimpleHeader, SimpleMethod};

use foundation_http::native::serve::{BoxFuture, H2Serve};
use foundation_http::shared::context::ContextBag;

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

            // The upstream exchange blocks (the shared client drives its own I/O),
            // so run it off the worker pool on a dedicated thread. The lease moves
            // in so the backend's in-flight count is held for the whole exchange.
            // The future returns immediately: the response flows back through `tx`,
            // which the h2 connection task drains and writes to the socket.
            std::thread::spawn(move || {
                forward_upstream(
                    lease, method, host, path, orig_headers, client_ip, scheme, client, body, &tx,
                );
            });
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
    let mut h2_hdrs: Vec<(Bytes, Bytes)> = Vec::new();
    for (name, values) in out_headers.iter() {
        let nb = Bytes::from(name.to_string().into_bytes());
        for v in values {
            h2_hdrs.push((nb.clone(), Bytes::from(v.clone().into_bytes())));
        }
    }
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
fn h2_status(code: u16, end_stream: bool) -> H2Frame {
    H2Frame::Headers {
        status: code,
        headers: vec![],
        end_stream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h2_status_404() {
        match h2_status(404, true) {
            H2Frame::Headers { status, end_stream, .. } => {
                assert_eq!(status, 404);
                assert!(end_stream);
            }
            _ => panic!("expected Headers frame"),
        }
    }

    #[test]
    fn h2_status_503() {
        match h2_status(503, false) {
            H2Frame::Headers { status, end_stream, .. } => {
                assert_eq!(status, 503);
                assert!(!end_stream);
            }
            _ => panic!("expected Headers frame"),
        }
    }
}
