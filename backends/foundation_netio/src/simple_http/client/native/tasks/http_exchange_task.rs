//! Platform HTTP exchange task (native) — yields `HttpExchange` items from a
//! `SendRequestTask<R>` child.
//!
//! WHY: The transport pump in connectrpc needs a platform-agnostic HTTP exchange
//! shape. On native this wraps `SendRequestTask<R>` via `inlined_task` and
//! translates `RequestIntro` into `HttpExchange` items.
//!
//! WHAT: [`HttpExchangeTask`] — a `TaskIterator` that owns a child `SendRequestTask`
//! receiver, extracts the response head and body chunks, and holds the
//! `HttpClientConnection` for pool return.
//!
//! HOW: `inlined_task(…, SendRequestTask::new(…))` spawns the child. Each
//! `next_status()` call does exactly one step — polls the child receiver or reads
//! the next body chunk. No internal loop.

use std::sync::Arc;

use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::{
    inlined_task, BoxedSendExecutionAction, InlineActionBehaviour,
    IntoBoxedSendExecutionAction, Stream, TaskIterator, TaskStatus,
};

use crate::simple_http::client::native::tasks::{RequestIntro, SendRequestTask};
use crate::simple_http::client::shared::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use crate::simple_http::client::shared::request_task::{HttpExchange, HttpExchangePending};
use crate::simple_http::client::shared::{ClientConfig, PreparedRequest, SystemDnsResolver};
use crate::simple_http::client::{HttpClientConnection, HttpConnectionPool};
use crate::simple_http::shared::IncomingResponseParts;

type Child = SendRequestTask<SystemDnsResolver>;
type ChildReceiver = foundation_core::valtron::DrivenRecvIterator<Child>;

/// State for the HTTP exchange pump.
enum State {
    /// Awaiting the next result from the spawned `SendRequestTask` child.
    Polling(ChildReceiver),
    /// Head received; draining the body from the `HttpResponseReader`.  The
    /// sub-state tracks whether we are in the middle of iterating a single
    /// `SizedBody`/`StreamedBody` payload — one `SendSafeBodyBytesIterator`
    /// may produce several chunks across multiple `next_status()` calls.
    StreamingBody {
        conn: Option<HttpClientConnection>,
        body_reader: Box<
            dyn Iterator<
                    Item = Result<
                        IncomingResponseParts,
                        crate::simple_http::shared::HttpReaderError,
                    >,
                > + Send,
        >,
        /// Active chunk iterator for the current body payload.
        chunk_iter: Option<SendSafeBodyBytesIterator>,
    },
    /// Failed before yielding Head. Holds the error to yield once.
    Failed(Option<SendableBoxedError>),
}

/// Native HTTP exchange task — wraps `SendRequestTask<R>` and yields platform-
/// agnostic `HttpExchange` items.
pub struct HttpExchangeTask {
    state: State,
    /// The spawn action, returned once on the first `next_status()` call.
    spawn: Option<BoxedSendExecutionAction>,
    /// Pool reference for returning the connection when the exchange completes.
    pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
}

impl HttpExchangeTask {
    /// Create the underlying `SendRequestTask` and prepare the spawn action.
    #[must_use]
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
        config: ClientConfig,
    ) -> Self {
        let pool_for_conn = Arc::clone(&pool);
        let child = SendRequestTask::new(request, max_redirects, pool, config);
        let (action, receiver) = inlined_task(
            InlineActionBehaviour::LiftWithParent,
            child,
            std::time::Duration::from_millis(0),
        );
        Self {
            state: State::Polling(receiver),
            spawn: Some(action.into_box_send_execution_action()),
            pool: pool_for_conn,
        }
    }

}

impl TaskIterator for HttpExchangeTask {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // First call — emit the Spawn so the executor runs the child.
        tracing::info!("calling next_status");

        if let Some(action) = self.spawn.take() {
            tracing::info!("calling next_status, sending spawn action");
            return Some(TaskStatus::Spawn(action));
        }

        tracing::info!("checking current state");
        match &mut self.state {
            State::Polling(ref mut rx) => {
                tracing::info!("polling state from spawned task under State::Polling");
                match rx.next() {
                    Some(TaskStatus::Ready(RequestIntro::Success {
                        intro,
                        headers,
                        conn,
                        stream,
                    })) => {
                        // intro: (Status, Proto, Option<String>)
                        self.state = State::StreamingBody {
                            conn: Some(conn),
                            body_reader: Box::new(stream),
                            chunk_iter: None,
                        };
                        Some(TaskStatus::Ready(HttpExchange::Head {
                            status: intro.0,
                            headers,
                        }))
                    }
                    Some(TaskStatus::Ready(RequestIntro::Failed(e))) => {
                        let err: SendableBoxedError = Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("{e}"),
                        ));
                        self.state = State::Failed(Some(err));
                        // Yield the error on the next call.
                        Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                    }
                    Some(TaskStatus::Pending(_)) => {
                        Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                    }
                    Some(TaskStatus::Spawn(action)) => Some(TaskStatus::Spawn(action)),
                    Some(TaskStatus::Depends(_)) => {
                        Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                    }
                    // Intermediate states — ask executor to re-poll us.
                    None
                    | Some(
                        TaskStatus::Init
                        | TaskStatus::Ignore
                        | TaskStatus::Wait
                        | TaskStatus::Delayed(_)
                        | TaskStatus::Spread(_),
                    ) => Some(TaskStatus::Pending(HttpExchangePending::Waiting)),
                }
            }
            State::StreamingBody {
                ref mut conn,
                ref mut body_reader,
                ref mut chunk_iter,
            } => {
                tracing::info!("polling state from spawned task under State::StreamingBody");
                // 1. If we have an active chunk iterator, drain it first.
                if let Some(ref mut iter) = chunk_iter {
                    match iter.next() {
                        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                            return Some(TaskStatus::Ready(HttpExchange::BodyChunk(bytes)));
                        }
                        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                            let se: SendableBoxedError = Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            ));
                            let taken_conn = conn.take();
                            self.state = State::Failed(Some(se));
                            if let Some(mut c) = taken_conn {
                                c.drain_stream();
                                self.pool.return_to_pool(c);
                            }
                            return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                        }
                        Some(Stream::Ignore) | None => {
                            // This payload exhausted — drop iterator, try next part.
                            *chunk_iter = None;
                            // Ask for a re-poll so we can read the next `IncomingResponseParts`.
                            return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                        }
                        _ => {
                            return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                        }
                    }
                }

                // 2. Read the next `IncomingResponseParts` from the body reader.
                match body_reader.next() {
                    Some(Ok(IncomingResponseParts::SizedBody(body)))
                    | Some(Ok(IncomingResponseParts::StreamedBody(body))) => {
                        *chunk_iter = Some(SendSafeBodyBytesIterator::new(body));
                        Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                    }
                    Some(Ok(IncomingResponseParts::NoBody)) => {
                        let taken_conn = conn.take();
                        if let Some(mut c) = taken_conn {
                            c.drain_stream();
                            self.pool.return_to_pool(c);
                        }
                        None
                    }
                    Some(Ok(
                        IncomingResponseParts::Intro(..)
                        | IncomingResponseParts::Headers(..)
                        | IncomingResponseParts::SKIP,
                    )) => Some(TaskStatus::Pending(HttpExchangePending::Waiting)),
                    Some(Err(e)) => {
                        let se: SendableBoxedError = Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            e.to_string(),
                        ));
                        let taken_conn = conn.take();
                        self.state = State::Failed(Some(se));
                        if let Some(mut c) = taken_conn {
                            c.drain_stream();
                            self.pool.return_to_pool(c);
                        }
                        Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                    }
                    None => {
                        let taken_conn = conn.take();
                        if let Some(mut c) = taken_conn {
                            c.drain_stream();
                            self.pool.return_to_pool(c);
                        }
                        None
                    }
                }
            }
            State::Failed(ref mut opt) => {
                opt.take()
                    .map(|e| TaskStatus::Ready(HttpExchange::Failed(e)))
            }
        }
    }
}
