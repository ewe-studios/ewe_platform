//! Platform HTTP exchange task (native) — yields `HttpExchange` items from a
//! `SendRequestTask<R>` child.
//!
//! WHY: The transport pump in connectrpc needs a platform-agnostic HTTP exchange
//! shape. On native this wraps `SendRequestTask<R>` via `InlineAction::new` and
//! translates `RequestIntro` into `HttpExchange` items.
//!
//! WHAT: [`HttpExchangeTask`] — a `TaskIterator` that owns a child `SendRequestTask`
//! receiver, extracts the response head and body chunks, and holds the
//! `HttpClientConnection` for pool return.
//!
//! HOW: `InlineAction::new(…, SendRequestTask::new(…))` builds the deferred child
//! action. Each `next_status()` call does exactly one step: it polls the child
//! receiver or reads the next body chunk. No internal loop.
//!
//! On a non-blocking connection (the default for TCP/Unix sockets and rustls)
//! the body is streamed without ever blocking a worker. The chunked, line-feed
//! and SSE iterators buffer each frame (a chunk-size line, a line, an event)
//! without consuming it, capped by the client's `FrameLimits`
//! (`shared::http::framing`), and stream chunk data in `batch_size` pieces. If
//! the socket would block first they report `HttpReaderError::WouldBlock`, and
//! the task parks on readiness (`TaskStatus::Depends`) with an inactivity
//! deadline. Raw (`Content-Length` / read-to-EOF) bodies read straight from the
//! socket, and their `WouldBlock` (surfaced as `Data::Retry`) parks the same way.

use std::sync::Arc;

use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::{Ctx, 
    inlined_task, BoxedSendExecutionAction, InlineActionBehaviour, Stream, TaskIterator, TaskStatus,
};

use super::nonblocking::{self, IdleDeadline, Park};
use crate::http::tasks::{RequestIntro, SendRequestTask};
use crate::http::{HttpClientConnection, HttpConnectionPool};
use crate::native::readiness::IoInterest;
use crate::shared::client::body_reader::{SendSafeBodyBytesItem, SendSafeBodyBytesIterator};
use crate::shared::client::request_task::{HttpExchange, HttpExchangePending};
use crate::shared::client::{ClientConfig, DnsResolver, PreparedRequest, SystemDnsResolver};
use crate::shared::http::{HttpClientError, IncomingResponseParts};

type ChildReceiver<R> = foundation_core::valtron::DrivenRecvIterator<SendRequestTask<R>>;

type ExchangeStatus = TaskStatus<HttpExchange, HttpExchangePending>;

/// The body-streaming state, split out so it can be borrowed independently of
/// the pool handle.
struct BodyStream {
    conn: Option<HttpClientConnection>,
    body_reader: Box<
        dyn Iterator<Item = Result<IncomingResponseParts, crate::shared::http::HttpReaderError>>
            + Send,
    >,
    /// Active chunk iterator for the current body payload.
    chunk_iter: Option<SendSafeBodyBytesIterator>,
    /// Whether the socket is non-blocking (parks on readiness) or blocking
    /// (the transport cannot report readiness; kernel read timeouts apply).
    nonblocking: bool,
    idle: IdleDeadline,
}

/// State for the HTTP exchange pump.
///
/// Generic over the DNS resolver `R` so the exchange inherits whatever resolver
/// the client stack was built with — the pump never pins a concrete resolver.
enum State<R: DnsResolver + Send + 'static> {
    /// Awaiting the next result from the spawned `SendRequestTask` child.
    Polling(ChildReceiver<R>),
    /// Head received; draining the body from the `HttpResponseReader`.
    StreamingBody(Box<BodyStream>),
    /// Failed. Holds the error to yield once.
    Failed(Option<SendableBoxedError>),
    /// The body ended and the connection went back to the pool.
    Done,
}

/// Native HTTP exchange task — wraps `SendRequestTask<R>` and yields platform-
/// agnostic `HttpExchange` items.
///
/// Generic over the DNS resolver `R`, defaulting to [`SystemDnsResolver`]. The
/// resolver flows in through the `pool`/child `SendRequestTask`, so a client
/// built with a custom resolver keeps it end-to-end — the exchange pins nothing.
pub struct HttpExchangeTask<R: DnsResolver + Send + 'static = SystemDnsResolver> {
    state: State<R>,
    /// The spawn action, returned once on the first `next_status()` call.
    spawn: Option<BoxedSendExecutionAction>,
    /// Pool reference for returning the connection when the exchange completes.
    pool: Arc<HttpConnectionPool<R>>,
    /// Inactivity bound while streaming the response body on a non-blocking socket.
    body_idle_timeout: std::time::Duration,
}

impl<R: DnsResolver + Send + 'static> HttpExchangeTask<R> {
    /// Create the underlying `SendRequestTask` and prepare the spawn action.
    #[must_use]
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<R>>,
        config: ClientConfig,
    ) -> Self {
        let pool_for_conn = Arc::clone(&pool);
        let body_idle_timeout = config.io_idle_timeout();
        // This task streams the body itself and parks on readiness, so it keeps
        // the socket non-blocking rather than letting the child switch it back
        // for a synchronous reader.
        let child =
            SendRequestTask::new(request, max_redirects, pool, config).with_nonblocking_handoff();
        let (action, receiver) = inlined_task(
            InlineActionBehaviour::LiftWithParent,
            child,
            std::time::Duration::from_millis(0),
        );
        Self {
            state: State::Polling(receiver),
            spawn: Some(Box::new(action)),
            pool: pool_for_conn,
            body_idle_timeout,
        }
    }

    /// Return a finished connection to the pool.
    ///
    /// Leftover bytes are drained first (a non-blocking drain stops at
    /// `WouldBlock` instead of sitting out a read timeout). The socket is then put
    /// back into blocking mode, because pooled connections are handed to callers
    /// that may read synchronously. A connection whose mode cannot be restored is
    /// dropped, not pooled.
    fn release(pool: &HttpConnectionPool<R>, mut conn: HttpClientConnection, nonblocking: bool) {
        conn.drain_stream();
        if nonblocking {
            if let Err(err) = conn.set_nonblocking(false) {
                tracing::warn!(?err, "dropping connection: cannot restore blocking mode");
                return;
            }
        }
        pool.return_to_pool(conn);
    }

    fn fail(
        &mut self,
        err: SendableBoxedError,
        conn: Option<(HttpClientConnection, bool)>,
    ) -> ExchangeStatus {
        self.state = State::Failed(Some(err));
        if let Some((c, nonblocking)) = conn {
            Self::release(&self.pool, c, nonblocking);
        }
        TaskStatus::Pending(HttpExchangePending::Waiting)
    }

    fn start_body(
        &mut self,
        stream: Box<
            crate::shared::http::HttpResponseReader<
                crate::shared::http::SimpleHttpBody,
                crate::netcap::RawStream,
            >,
        >,
        conn: HttpClientConnection,
        intro: crate::shared::http::HttpResponseIntro,
        headers: crate::shared::http::SimpleHeaders,
    ) -> ExchangeStatus {
        let nonblocking = match conn.is_nonblocking() {
            Ok(nb) => nb,
            Err(err) => {
                tracing::error!(?err, "cannot read the socket mode");
                return self.fail(Box::new(HttpClientError::IoError(err)), None);
            }
        };
        // On a non-blocking socket every `WouldBlock` parks until readiness and
        // the idle deadline bounds the wait. So the body readers' consecutive-retry
        // budget (sized for blocking reads, where each retry waited a read
        // timeout) must not cut a slow body short.
        let body_reader: Box<
            dyn Iterator<Item = Result<IncomingResponseParts, crate::shared::http::HttpReaderError>>
                + Send,
        > = if nonblocking {
            Box::new((*stream).with_max_retries(usize::MAX))
        } else {
            Box::new(stream)
        };
        self.state = State::StreamingBody(Box::new(BodyStream {
            conn: Some(conn),
            body_reader,
            chunk_iter: None,
            nonblocking,
            idle: IdleDeadline::new(self.body_idle_timeout),
        }));
        TaskStatus::Ready(HttpExchange::Head {
            status: intro.0,
            headers,
        })
    }

    fn poll_child(&mut self) -> ExchangeStatus {
        let State::Polling(ref mut rx) = self.state else {
            unreachable!("poll_child is only called while polling");
        };
        match rx.next() {
            Some(TaskStatus::Ready(RequestIntro::Success {
                intro,
                headers,
                conn,
                stream,
            })) => self.start_body(stream, conn, intro, headers),
            Some(TaskStatus::Ready(RequestIntro::Failed(e))) => {
                // Carry the concrete `HttpClientError` object — it is
                // `Send + Sync`, so it goes straight into the boxed error
                // with its type and source chain intact.
                self.fail(Box::new(e), None)
            }
            Some(TaskStatus::Spawn(action)) => TaskStatus::Spawn(action),
            // Intermediate states (including a child that parked itself on
            // readiness) — ask the executor to re-poll us.
            None
            | Some(
                TaskStatus::Pending(_)
                | TaskStatus::Depends(_)
                | TaskStatus::Init
                | TaskStatus::Ignore
                | TaskStatus::Wait
                | TaskStatus::Delayed(_)
                | TaskStatus::Spread(_),
            ) => TaskStatus::Pending(HttpExchangePending::Waiting),
        }
    }

    /// Park the body stream on readability after a `WouldBlock`.
    fn park_body(&mut self) -> ExchangeStatus {
        let State::StreamingBody(ref mut body) = self.state else {
            unreachable!("park_body is only called while streaming");
        };
        let Some(conn) = body.conn.as_ref() else {
            unreachable!("the connection is held until the body ends");
        };
        match nonblocking::park(conn, IoInterest::Readable, &mut body.idle) {
            Park::On(signal) => TaskStatus::Depends(signal),
            Park::Expired => {
                tracing::error!(timeout = ?body.idle.timeout(), "response body stalled past the idle deadline");
                let conn = body.conn.take().map(|c| (c, body.nonblocking));
                self.fail(Box::new(HttpClientError::Timeout), conn)
            }
            Park::Unsupported => {
                let conn = body.conn.take().map(|c| (c, body.nonblocking));
                self.fail(
                    Box::new(HttpClientError::ReaderError(
                        crate::shared::http::HttpReaderError::WouldBlock,
                    )),
                    conn,
                )
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn stream_body(&mut self) -> ExchangeStatus {
        let State::StreamingBody(ref mut body) = self.state else {
            unreachable!("stream_body is only called while streaming");
        };

        // 1. If we have an active chunk iterator, drain it first.
        if body.chunk_iter.is_some() {
            let Some(iter) = body.chunk_iter.as_mut() else {
                unreachable!("checked above");
            };
            match iter.next() {
                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                    body.idle.progress();
                    return TaskStatus::Ready(HttpExchange::BodyChunk(bytes));
                }
                Some(Stream::Wait) if body.nonblocking => {
                    // The socket has nothing yet (any body kind): park and
                    // call again when readable.
                    return self.park_body();
                }
                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                    // Unlike the other failure arms, this error cannot be carried
                    // as an object: `SendSafeBodyBytesItem::StreamError` holds a
                    // `BoxedError` (`Box<dyn Error + 'static>`, no `Send + Sync`),
                    // and `HttpExchange::Failed` demands a `Send + Sync` error that
                    // crosses the task/split boundary. We preserve the message via
                    // an `io::Error`.
                    let se: SendableBoxedError = Box::new(std::io::Error::other(e.to_string()));
                    let conn = body.conn.take().map(|c| (c, body.nonblocking));
                    return self.fail(se, conn);
                }
                Some(Stream::Ignore) => {
                    // A non-data item (chunk end marker, trailers, a skipped
                    // line) — re-poll for the next.
                    body.idle.progress();
                    return TaskStatus::Pending(HttpExchangePending::Waiting);
                }
                None => {
                    // This payload exhausted — drop iterator, try next part.
                    body.chunk_iter = None;
                    return TaskStatus::Pending(HttpExchangePending::Waiting);
                }
                Some(_) => return TaskStatus::Pending(HttpExchangePending::Waiting),
            }
        }

        // 2. Read the next `IncomingResponseParts` from the body reader.
        match body.body_reader.next() {
            Some(Ok(
                IncomingResponseParts::SizedBody(payload)
                | IncomingResponseParts::StreamedBody(payload),
            )) => {
                body.chunk_iter = Some(SendSafeBodyBytesIterator::new(payload));
                TaskStatus::Pending(HttpExchangePending::Waiting)
            }
            Some(Ok(
                IncomingResponseParts::Intro(..)
                | IncomingResponseParts::Headers(..)
                | IncomingResponseParts::SKIP,
            )) => TaskStatus::Pending(HttpExchangePending::Waiting),
            Some(Err(e)) if body.nonblocking && e.is_would_block() => self.park_body(),
            Some(Err(e)) => {
                // `HttpReaderError` is `Send + Sync` — carry it intact.
                let conn = body.conn.take().map(|c| (c, body.nonblocking));
                self.fail(Box::new(e), conn)
            }
            Some(Ok(IncomingResponseParts::NoBody)) | None => {
                if let Some(c) = body.conn.take() {
                    let nonblocking = body.nonblocking;
                    Self::release(&self.pool, c, nonblocking);
                }
                self.state = State::Done;
                TaskStatus::Pending(HttpExchangePending::Waiting)
            }
        }
    }
}

impl<R: DnsResolver + Send + 'static> TaskIterator for HttpExchangeTask<R> {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // First call — emit the Spawn so the executor runs the child.
        if let Some(action) = self.spawn.take() {
            return Some(TaskStatus::Spawn(action));
        }

        match self.state {
            State::Polling(_) => Some(self.poll_child()),
            State::StreamingBody(_) => Some(self.stream_body()),
            State::Done => None,
            State::Failed(ref mut opt) => {
                let err = opt.take();
                if err.is_none() {
                    self.state = State::Done;
                }
                // Box → Arc at the public boundary (F45 Resolution 7): the error
                // is fanned out to both split branches, so it must be clonable.
                err.map(|e| TaskStatus::Ready(HttpExchange::Failed(Arc::from(e))))
            }
        }
    }
}

