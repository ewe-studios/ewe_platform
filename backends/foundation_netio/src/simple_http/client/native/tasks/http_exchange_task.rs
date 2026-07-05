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
//! HOW: `inlined_task(…, SendRequestTask::new(…))` spawns the child. The receiver
//! yields `TaskStatus<RequestIntro, HttpRequestPending, …>`. On `Ready(Success)`,
//! extract `(status, headers)` → `HttpExchange::Head`, wrap the body in a
//! `SendSafeBodyBytesIterator` → `HttpExchange::BodyChunk` for each chunk.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::{
    inlined_task, BoxedSendExecutionAction, InlineSendActionBehaviour, IntoBoxedSendExecutionAction,
    Stream, TaskIterator, TaskStatus,
};

use crate::simple_http::client::shared::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use crate::simple_http::client::shared::request_task::{HttpExchange, HttpExchangePending};
use crate::simple_http::client::shared::{ClientConfig, PreparedRequest, SystemDnsResolver};
use crate::simple_http::client::{HttpConnectionPool, HttpClientConnection};
use crate::simple_http::client::native::tasks::{HttpRequestPending, RequestIntro, SendRequestTask};
use crate::simple_http::shared::{IncomingResponseParts, SimpleHeaders, Status};

type Child = SendRequestTask<SystemDnsResolver>;
type ChildStatus = TaskStatus<RequestIntro, HttpRequestPending, BoxedSendExecutionAction>;
type ChildReceiver = foundation_core::valtron::DrivenRecvIterator<Child>;

/// State for the HTTP exchange pump.
enum State {
    /// Haven't received the first result from the child yet.
    AwaitingChild,
    /// Awaiting the next `TaskStatus` from the spawned `SendRequestTask` child.
    Polling(ChildReceiver),
    /// Head received; draining the body from the `HttpResponseReader`.
    StreamingBody {
        conn: Option<HttpClientConnection>,
        body_reader: Box<
            dyn Iterator<
                    Item = Result<IncomingResponseParts, crate::simple_http::shared::HttpReaderError>,
                > + Send,
        >,
    },
    /// Failed before yielding Head.
    Failed(Option<SendableBoxedError>),
    /// All output yielded.
    Done,
}

/// Native HTTP exchange task — wraps `SendRequestTask<R>` and yields platform-
/// agnostic `HttpExchange` items.
pub struct HttpExchangeTask {
    state: State,
    /// The spawn action, returned once on the first `next_status()` call.
    spawn: Option<BoxedSendExecutionAction>,
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
        let child = SendRequestTask::new(request, max_redirects, pool, config);
        let (action, receiver) = inlined_task(
            InlineSendActionBehaviour::LiftWithParent,
            Vec::new(),
            child,
            std::time::Duration::from_millis(0),
        );
        Self {
            state: State::Polling(receiver),
            spawn: Some(action.into_box_send_execution_action()),
        }
    }
}

impl TaskIterator for HttpExchangeTask {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // First call: emit the Spawn so the executor runs the child.
        if let Some(action) = self.spawn.take() {
            return Some(TaskStatus::Spawn(action));
        }

        loop {
            match &mut self.state {
                State::AwaitingChild => unreachable!("spawn already emitted"),
                State::Polling(ref mut rx) => {
                    return match rx.next() {
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
                            };
                            Some(TaskStatus::Ready(HttpExchange::Head {
                                status: intro.0,
                                headers,
                            }))
                        }
                        Some(TaskStatus::Ready(RequestIntro::Failed(e))) => {
                            self.state =
                                State::Failed(Some(Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("{e}"),
                                ))));
                            continue;
                        }
                        Some(TaskStatus::Pending(_)) => {
                            Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                        }
                        Some(TaskStatus::Spawn(action)) => {
                            Some(TaskStatus::Spawn(action))
                        }
                        Some(
                            TaskStatus::Init
                            | TaskStatus::Ignore
                            | TaskStatus::Wait
                            | TaskStatus::Delayed(_),
                        ) => continue,
                        Some(TaskStatus::Depends(_)) => {
                            Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                        }
                        Some(TaskStatus::Spread(_)) => continue,
                        None => {
                            let e: SendableBoxedError = Box::new(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "SendRequestTask exhausted without producing RequestIntro",
                            ));
                            self.state = State::Failed(Some(e));
                            continue;
                        }
                    };
                }
                State::StreamingBody {
                    ref mut conn,
                    ref mut body_reader,
                } => {
                    return match body_reader.next() {
                        Some(Ok(IncomingResponseParts::SizedBody(body)))
                        | Some(Ok(IncomingResponseParts::StreamedBody(body))) => {
                            let mut iter = SendSafeBodyBytesIterator::new(body);
                            match iter.next() {
                                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                                    Some(TaskStatus::Ready(HttpExchange::BodyChunk(bytes)))
                                }
                                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                                    let _ = conn.take();
                                    let e: SendableBoxedError =
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::Other,
                                            e.to_string(),
                                        ));
                                    Some(TaskStatus::Ready(HttpExchange::Failed(e)))
                                }
                                Some(Stream::Ignore) | None => continue,
                                _ => continue,
                            }
                        }
                        Some(Ok(IncomingResponseParts::NoBody)) => {
                            let _ = conn.take();
                            None
                        }
                        Some(Ok(
                            IncomingResponseParts::Intro(..)
                            | IncomingResponseParts::Headers(..)
                            | IncomingResponseParts::SKIP,
                        )) => continue,
                        Some(Err(e)) => {
                            let _ = conn.take();
                            let se: SendableBoxedError =
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ));
                            Some(TaskStatus::Ready(HttpExchange::Failed(se)))
                        }
                        None => {
                            let _ = conn.take();
                            None
                        }
                    };
                }
                State::Failed(ref mut opt) => {
                    return opt
                        .take()
                        .map(|e| TaskStatus::Ready(HttpExchange::Failed(e)));
                }
                State::Done => return None,
            }
        }
    }
}
