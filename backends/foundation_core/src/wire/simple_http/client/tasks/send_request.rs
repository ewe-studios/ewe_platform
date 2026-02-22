//! HTTP request task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides a non-blocking, state-machine-based HTTP request executor that
//! integrates with the valtron executor system. Enables async-like request handling
//! without async/await.
//!
//! WHAT: Implements `HttpRequestTask` which processes HTTP requests through a series
//! of states (connecting, sending request, receiving response).
//! Uses `TaskIterator` trait to yield `TaskStatus` variants.
//!
//! HOW: State machine pattern where each `next()` call advances through states.
//! Phase 1 uses blocking connection for simplicity. Future phases will use
//! non-blocking connection spawning and TLS support.
//!
//! PHASE 1 SCOPE: HTTP-only (no HTTPS), blocking connection, basic GET requests.
//! PHASE 2 SCOPE: HTTPS support, non-blocking connection, advanced request handling.

use crate::netcap::RawStream;
use crate::valtron::{
    drive_receiver, inlined_task, BoxedSendExecutionAction, DrivenRecvIterator, InlineSendAction,
    IntoBoxedSendExecutionAction, TaskIterator, TaskStatus,
};
use crate::wire::simple_http::client::{DnsResolver, HttpConnectionPool, PreparedRequest};
use crate::wire::simple_http::{
    HttpReaderError, HttpResponseReader, IncomingResponseParts, SimpleHttpBody,
};
use std::io::Write;
use std::sync::Arc;

use super::{
    GetHttpRequestRedirectTask, GetRequestIntroTask, HttpRequestRedirectResponse, OpTimeout,
    RequestIntro, SendRequest,
};

pub enum SendRequestState<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Init starts out with the request based on the provided [`GetHttpRequestStreamInner`]
    /// which then moves to [`Self::Connecting`] and then [`Self::Pull`]
    /// to get the actual request response.
    Init(Option<Box<SendRequest<R>>>),

    /// [`Connecting`] contains the read iterator to read the  response from the connection.
    Connecting(DrivenRecvIterator<GetHttpRequestRedirectTask<R>>),

    /// [`Reading`] reads the introduction information from (Status + Headers) from the connection.
    Reading(DrivenRecvIterator<GetRequestIntroTask>),

    /// [`SkipReading`] skips the attempt to read the intro from the stream
    /// as indicates the request intro has just being read already
    /// and provides the request response.
    SkipReading(Option<RequestIntro>),

    /// No more work to do
    Done,
}

pub struct SendRequestTask<R>(Option<SendRequestState<R>>)
where
    R: DnsResolver + Send + 'static;

impl<R> SendRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<R>>,
        timeouts: Option<OpTimeout>,
    ) -> Self {
        Self(Some(SendRequestState::Init(Some(Box::new(
            SendRequest::new(request, max_redirects, pool, timeouts.unwrap_or_default()),
        )))))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum HttpRequestPending {
    WaitingForStream,
    WaitingIntroAndHeaders,
}

impl<R> TaskIterator for SendRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            SendRequestState::Init(mut inner) => match inner.take() {
                Some(boxed_req) => {
                    let send_request = *boxed_req;
                    if send_request.request.is_none() {
                        tracing::warn!("Request is missing");

                        self.0 = Some(SendRequestState::Done);

                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    }

                    let incoming_request = send_request.request.expect("Request is missing");
                    let Ok(into_incoming) = incoming_request.into_simple_incoming_request() else {
                        self.0 = Some(SendRequestState::Done);

                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    };

                    let (get_stream_action, get_stream_receiver) = inlined_task(
                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                        Vec::new(),
                        GetHttpRequestRedirectTask::new(
                            into_incoming,
                            Some(send_request.timeouts),
                            send_request.pool.clone(),
                            send_request.remaining_redirects,
                        ),
                        std::time::Duration::from_millis(100),
                    );

                    self.0 = Some(SendRequestState::Connecting(get_stream_receiver));

                    tracing::debug!(
                        "HttpRequestTaskState::Init: Spawned task to get HTTP request stream"
                    );
                    Some(TaskStatus::Spawn(
                        get_stream_action.into_box_send_execution_action(),
                    ))
                }
                None => unreachable!("Task state must never get here"),
            },
            SendRequestState::Connecting(mut recv_iter) => {
                tracing::debug!(
                    "HttpRequestTaskState::Connecting: Reading next http state from receiver"
                );
                let next_value = recv_iter.next();

                self.0 = Some(SendRequestState::Connecting(recv_iter));
                tracing::debug!("HttpRequestTaskState::Connecting: received receiver iterator");

                if next_value.is_none() {
                    self.0.take();

                    tracing::debug!("HttpRequestTaskState::Connecting: failed execution");
                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpReaderError::ReadFailed,
                    )));
                }

                tracing::debug!("HttpRequestTaskState::Connecting: processing next value");
                match next_value.unwrap() {
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(_) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    TaskStatus::Spawn(action) => {
                        tracing::debug!("HttpRequestTaskState::Connecting: spawn new action");
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    TaskStatus::Ready(item) => {
                        tracing::debug!(
                            "HttpRequestTaskState::Connecting: received TaskStats::Ready(_)"
                        );

                        match item {
                            HttpRequestRedirectResponse::Done(
                                stream,
                                reader,
                                optional_starters,
                            ) => {
                                tracing::debug!(
                                    "HttpRequestTaskState::Connecting::HttpStreamReady::Done(stream): Send next action -> GetRequestIntroTask"
                                );

                                if let Some(starter_array) = optional_starters {
                                    tracing::debug!("HttpRequestRedirectResponse::Done: received intro from redirect process");

                                    let [first, second] = starter_array;

                                    let intro = if let IncomingResponseParts::Intro(
                                        status,
                                        proto,
                                        text,
                                    ) = first
                                    {
                                        (status, proto, text)
                                    } else {
                                        self.0.take();

                                        tracing::debug!(
                                            "First item in optional_starters is not an IncomingResponseParts::Intro"
                                        );
                                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                                            HttpReaderError::ReadFailed,
                                        )));
                                    };

                                    let headers = if let IncomingResponseParts::Headers(inner) =
                                        second
                                    {
                                        inner
                                    } else {
                                        self.0.take();

                                        tracing::debug!(
                                            "Second item in optional_starters is not an IncomingResponseParts::Headers"
                                        );
                                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                                            HttpReaderError::ReadFailed,
                                        )));
                                    };

                                    self.0 = Some(SendRequestState::SkipReading(Some(
                                        RequestIntro::Success {
                                            stream: reader,
                                            conn: stream,
                                            intro,
                                            headers,
                                        },
                                    )));

                                    return Some(TaskStatus::Pending(
                                        HttpRequestPending::WaitingIntroAndHeaders,
                                    ));
                                }

                                let (get_intro_stream_action, get_intro_receiver) =
                                    InlineSendAction::boxed_mapper(
                                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                        Vec::new(),
                                        GetRequestIntroTask::new(stream),
                                        std::time::Duration::from_millis(100),
                                    );

                                self.0 = Some(SendRequestState::Reading(drive_receiver(
                                    get_intro_receiver,
                                )));

                                Some(TaskStatus::Spawn(
                                    get_intro_stream_action.into_box_send_execution_action(),
                                ))
                            }
                            HttpRequestRedirectResponse::FlushFailed(mut conn, err) => {
                                tracing::debug!(
                                    "FlushFailed(err={err:?}): Failed and re-attempt and fetch intro"
                                );

                                if let Err(err) = conn.stream_mut().flush() {
                                    tracing::error!("Failed to flush HTTP stream: {}", err);
                                }

                                let (get_intro_stream_action, get_intro_receiver) =
                                    InlineSendAction::boxed_mapper(
                                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                        Vec::new(),
                                        GetRequestIntroTask::new(conn),
                                        std::time::Duration::from_millis(100),
                                    );

                                self.0 = Some(SendRequestState::Reading(drive_receiver(
                                    get_intro_receiver,
                                )));

                                Some(TaskStatus::Spawn(
                                    get_intro_stream_action.into_box_send_execution_action(),
                                ))
                            }
                            HttpRequestRedirectResponse::Error(err) => {
                                self.0.take();

                                tracing::debug!(
                                    "HttpRequestTaskState::Connecting::HttpStreamReady::Error({err:?}): Failed and returning read failure"
                                );
                                Some(TaskStatus::Ready(RequestIntro::Failed(
                                    HttpReaderError::ReadFailed,
                                )))
                            }
                        }
                    }
                }
            }
            SendRequestState::SkipReading(None) => {
                tracing::debug!("HttpRequestTaskState::Reading: received intro already");

                Some(TaskStatus::Ready(RequestIntro::Failed(
                    HttpReaderError::ReadFailed,
                )))
            }
            SendRequestState::SkipReading(Some(container)) => {
                tracing::debug!(
                    "HttpRequestTaskState::Reading: received intro already, forwarding"
                );

                Some(TaskStatus::Ready(container))
            }
            SendRequestState::Reading(mut intro_recv) => {
                tracing::debug!(
                    "HttpRequestTaskState::Reading: Reading next state from http request reciever"
                );
                let next_value = intro_recv.next();

                tracing::debug!("HttpRequestTaskState::Reading: Gotten next state from iterator");
                self.0 = Some(SendRequestState::Reading(intro_recv));

                if next_value.is_none() {
                    self.0.take();

                    tracing::debug!("HttpRequestTaskState::Reading: failed to read from iterator");
                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpReaderError::ReadFailed,
                    )));
                }

                match next_value.unwrap() {
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(()) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    TaskStatus::Spawn(action) => {
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    TaskStatus::Ready(item) => {
                        self.0.take();

                        tracing::debug!(
                            "HttpRequestTaskState::Reading: got ready value returning Ready item"
                        );
                        Some(TaskStatus::Ready(item))
                    }
                }
            }
            SendRequestState::Done => None,
        }
    }
}
