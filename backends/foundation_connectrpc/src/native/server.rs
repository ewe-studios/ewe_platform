//! Server connection-owner adapter (Decision 11 §Connection ownership, Decision 08
//! §0) — native only.
//!
//! WHY: The router (Decision 08) decides and executes an RPC, but something has to
//! own the raw connection and move bytes to/from the socket. On foundation_netio's
//! server that owner is the **per-connection task the listener spawns on `accept`**
//! (`Serve`): it has read the request head off the wire and hands us a
//! `SimpleIncomingRequest` (whose body is still on the socket) plus the
//! connection to write the response to. This adapter is the bridge — it makes a
//! [`ConnectRpcHandler`] a mountable `Serve` handler, so the connection owner
//! drives dispatch and renders the response.
//!
//! WHAT: [`ConnectRpcServe`] — wraps a frozen [`ConnectRpcHandler`] and implements
//! foundation_http's `Serve`. [`ServeTimeouts`] bounds how long it waits on a
//! stalled peer.
//!
//! HOW: `serve` runs the dispatch future to completion on the calling thread
//! (it is self-contained — the streaming path joins its own tasks
//! cooperatively, Decision 08 — so it needs no executor pool) through a
//! [`ConnIo`] driver. The socket is non-blocking: the request body is read
//! as an async [`RequestBody`] and the response is written through an
//! [`H1Sink`]; both wait on the socket through the driver when it would block.
//! Whole responses (unary, errors) carry a `Content-Length`; a stream sends its
//! head once the first frame exists, then each frame as an HTTP/1.1 chunk as
//! soon as it is produced, then the terminating chunk.
//!
//! The driver also watches the socket for the client going away and enforces
//! the call's deadline: either cancels the call's `Ctx`. A client that closes
//! (or half-closes) the connection mid-call gets the call cancelled and
//! dropped and the connection closed, as nginx and Go's `net/http` do.

use std::future::Future;
use std::io::{self, Write};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve};
use foundation_netio::native::readiness::IoInterest;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    ensure_chunked_transfer_encoding, Http11, RenderHttp, SendSafeBody, SimpleHeader,
    SimpleIncomingRequest, SimpleOutgoingResponse,
};

use super::conn_io::{ConnIo, IoReady};
use crate::shared::context::CancelSignal;
use crate::shared::router::dispatch::{BodyReader, BodyStep};
use crate::shared::router::{CallHooks, ConnectRpcHandler, RequestBody, ResponseSink};
use crate::shared::transport::BoxFuture;
use futures::future::{select, Either};

/// How long [`ConnectRpcServe`] waits on a stalled peer before giving up on the
/// request and closing the connection. Each timeout bounds one wait (the time
/// between two successful reads or writes), not the whole RPC, so a long
/// stream that keeps moving is never cut off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServeTimeouts {
    /// Longest wait for the next piece of the request body. Default 60s, as
    /// nginx's `client_body_timeout`.
    pub body_read: Duration,
    /// Longest wait for the peer to accept more response bytes. Default 60s,
    /// as nginx's `send_timeout`.
    pub write: Duration,
}

impl Default for ServeTimeouts {
    fn default() -> Self {
        Self {
            body_read: Duration::from_secs(60),
            write: Duration::from_secs(60),
        }
    }
}

impl ServeTimeouts {
    /// Set [`Self::body_read`].
    #[must_use]
    pub fn with_body_read(mut self, timeout: Duration) -> Self {
        self.body_read = timeout;
        self
    }

    /// Set [`Self::write`].
    #[must_use]
    pub fn with_write(mut self, timeout: Duration) -> Self {
        self.write = timeout;
        self
    }
}

/// A [`ConnectRpcHandler`] mounted as a `foundation_http` `Serve` handler — the
/// server-side connection-owner bridge (Decision 11 §Connection ownership).
pub struct ConnectRpcServe {
    handler: Arc<ConnectRpcHandler>,
    timeouts: ServeTimeouts,
}

impl ConnectRpcServe {
    /// Wrap a frozen handler (from [`Router::into_handler`](crate::Router::into_handler)).
    #[must_use]
    pub fn new(handler: ConnectRpcHandler) -> Self {
        Self::from_arc(Arc::new(handler))
    }

    /// Share an already-`Arc`'d handler across mounts.
    #[must_use]
    pub fn from_arc(handler: Arc<ConnectRpcHandler>) -> Self {
        Self {
            handler,
            timeouts: ServeTimeouts::default(),
        }
    }

    /// Replace the [`ServeTimeouts`].
    #[must_use]
    pub fn with_timeouts(mut self, timeouts: ServeTimeouts) -> Self {
        self.timeouts = timeouts;
        self
    }
}

impl Serve for ConnectRpcServe {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        mut req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let io = match ConnIo::new(&conn) {
            Ok(io) => io,
            Err(e) => {
                tracing::error!(error = %e, "could not set up the connection driver; closing");
                return ConnectionResult::Close(None);
            }
        };
        let body = req.body.take();
        let drained = AtomicBool::new(matches!(body, None | Some(SendSafeBody::None)));
        let body: RequestBody<'_> = Box::pin(H1Body {
            reader: BodyReader::new(body),
            io: &io,
            timeout: self.timeouts.body_read,
            wait: None,
            drained: &drained,
        });
        let mut sink = H1Sink {
            conn: &mut conn,
            io: &io,
            timeout: self.timeouts.write,
        };
        let cancel = CancelSignal::new();
        let hooks = CallHooks::new(cancel.clone()).with_timer(&io);
        let dispatch = std::pin::pin!(self.handler.dispatch_to(bag, req, body, &mut sink, hooks));
        // Watch for the client going away while the call runs: nothing else
        // reads the socket once the body is in, so without this a handler
        // waiting on something other than the peer never learns of it.
        let hangup = std::pin::pin!(async {
            match io.hangup().await {
                Ok(()) => {}
                Err(e) => {
                    // This transport cannot report it; the call still ends
                    // when a read or write on the connection fails.
                    tracing::debug!(error = %e, "cannot watch the connection for a hangup");
                    std::future::pending::<()>().await;
                }
            }
        });
        let outcome = match io.block_on(select(dispatch, hangup)) {
            Either::Left((outcome, _)) => outcome,
            Either::Right(((), _dispatch)) => {
                tracing::debug!("the client closed the connection; cancelling the call");
                cancel.cancel();
                return ConnectionResult::Close(None);
            }
        };
        match outcome {
            // The next request starts where this body ends, so the connection
            // can only be reused once the body was read to its end.
            Ok(()) if drained.load(Ordering::Acquire) => ConnectionResult::Keep,
            Ok(()) => {
                tracing::debug!(
                    "the request body was not read to its end; closing the connection"
                );
                ConnectionResult::Close(None)
            }
            Err(e) => {
                tracing::debug!(error = %e, "writing the response failed; closing the connection");
                ConnectionResult::Close(None)
            }
        }
    }
}

/// The request body as it arrives on the connection: reads what is there and
/// waits for the socket when nothing is.
struct H1Body<'a> {
    reader: BodyReader,
    io: &'a ConnIo,
    timeout: Duration,
    /// The wait in progress after the body reported `WouldBlock`.
    wait: Option<IoReady<'a>>,
    /// Set once the body has been read to its end.
    drained: &'a AtomicBool,
}

impl futures::Stream for H1Body<'_> {
    type Item = io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(wait) = this.wait.as_mut() {
                match Pin::new(wait).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(result) => {
                        this.wait = None;
                        if let Err(e) = result {
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
            }
            match this.reader.step() {
                BodyStep::Chunk(bytes) => return Poll::Ready(Some(Ok(bytes))),
                BodyStep::End => {
                    this.drained.store(true, Ordering::Release);
                    return Poll::Ready(None);
                }
                BodyStep::Failed(e) => return Poll::Ready(Some(Err(e))),
                BodyStep::WouldBlock => {
                    this.wait = Some(this.io.ready(IoInterest::Readable, this.timeout));
                }
            }
        }
    }
}

/// Writes a dispatched response to an HTTP/1.1 connection, waiting for the
/// socket when the peer is slow to read.
struct H1Sink<'a> {
    conn: &'a mut SharedByteBufferStream<RawStream>,
    io: &'a ConnIo,
    timeout: Duration,
}

impl H1Sink<'_> {
    async fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.conn.write(buf) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "the connection accepted no bytes",
                    ));
                }
                Ok(n) => buf = &buf[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.io.ready(IoInterest::Writable, self.timeout).await?;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn flush(&mut self) -> io::Result<()> {
        loop {
            match self.conn.flush() {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.io.ready(IoInterest::Writable, self.timeout).await?;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Write one HTTP/1.1 chunk (never a zero-length one, which would end the
    /// body).
    async fn write_chunk(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())
            .await?;
        self.write_all(bytes).await?;
        self.write_all(b"\r\n").await
    }

    async fn write_whole(&mut self, mut response: SimpleOutgoingResponse) -> io::Result<()> {
        match response.body.take() {
            None | Some(SendSafeBody::None) => {
                set_content_length(&mut response, 0);
                self.write_all(&render_head(response)?).await?;
            }
            Some(SendSafeBody::Bytes(bytes)) => {
                set_content_length(&mut response, bytes.len());
                self.write_all(&render_head(response)?).await?;
                self.write_all(&bytes).await?;
            }
            Some(SendSafeBody::Text(text)) => {
                set_content_length(&mut response, text.len());
                self.write_all(&render_head(response)?).await?;
                self.write_all(text.as_bytes()).await?;
            }
            // A streamed body has no length up front: send it chunked.
            Some(streamed) => {
                ensure_chunked_transfer_encoding(&mut response.headers);
                self.write_all(&render_head(response)?).await?;
                let mut reader = BodyReader::new(Some(streamed));
                loop {
                    match reader.step() {
                        BodyStep::Chunk(bytes) => self.write_chunk(&bytes).await?,
                        BodyStep::End => break,
                        BodyStep::Failed(e) => return Err(e),
                        // Response bodies are produced in memory by dispatch;
                        // there is no socket to wait on for them.
                        BodyStep::WouldBlock => {
                            return Err(io::Error::new(
                                io::ErrorKind::WouldBlock,
                                "a response body reported it would block",
                            ));
                        }
                    }
                }
                self.write_all(b"0\r\n\r\n").await?;
            }
        }
        self.flush().await
    }
}

impl ResponseSink for H1Sink<'_> {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(self.write_whole(response))
    }

    fn open(&mut self, mut head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            // The frames follow as chunks, so the head declares chunked framing
            // instead of a length.
            ensure_chunked_transfer_encoding(&mut head.headers);
            head.body = None;
            self.write_all(&render_head(head)?).await?;
            self.flush().await
        })
    }

    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            self.write_chunk(&bytes).await?;
            self.flush().await
        })
    }

    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            self.write_all(b"0\r\n\r\n").await?;
            self.flush().await
        })
    }
}

/// Render the status line and headers of `response` (its body is not sent).
fn render_head(mut response: SimpleOutgoingResponse) -> io::Result<Vec<u8>> {
    response.body = Some(SendSafeBody::None);
    let parts = Http11::response(response)
        .http_render()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mut head = Vec::new();
    for part in parts {
        head.extend_from_slice(&part.map_err(|e| io::Error::other(e.to_string()))?);
    }
    Ok(head)
}

/// Frame a sized body with `Content-Length` (netio's renderer does not compute
/// it). A length the response already declares is kept.
fn set_content_length(response: &mut SimpleOutgoingResponse, len: usize) {
    response
        .headers
        .entry(SimpleHeader::CONTENT_LENGTH)
        .or_insert_with(|| vec![len.to_string()]);
}
