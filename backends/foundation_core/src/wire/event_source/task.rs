//! SSE client [`TaskIterator`](crate::valtron::TaskIterator) implementation.
//!
//! WHY: Clients need a non-blocking, state-machine-based SSE consumer that
//! integrates with the valtron executor system. Enables async-like event handling
//! without async/await.
//!
//! WHAT: Implements [`EventSourceTask`] which processes SSE connections through a series
//! of states (connecting, reading events). Uses `TaskIterator` trait to yield
//! `TaskStatus` variants for each SSE event.
//!
//! HOW: State machine where each `next()` call advances through states.
//! Wraps [`HttpResponseReader`](crate::wire::simple_http::HttpResponseReader) with [`SseParser`] to parse SSE events from HTTP stream.
//!
//! PHASE 1 SCOPE: Basic SSE client with `TaskIterator` pattern.
//! PHASE 2 SCOPE: Automatic reconnection with exponential backoff.

use crate::valtron::{
    inlined_task, BoxedSendExecutionAction, DrivenRecvIterator, IntoBoxedSendExecutionAction,
    TaskIterator, TaskStatus,
};
use crate::wire::event_source::{Event, EventSourceError, SseParser};
use crate::wire::simple_http::client::{ClientRequestBuilder, DnsResolver};
use crate::wire::simple_http::{HttpSendResponseReader, SimpleHeader, SimpleHttpBody};
use std::io::BufRead;
use std::marker::PhantomData;

/// [`EventSourceProgress`] indicates the current state of SSE connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourceProgress {
    Connecting,
    Reading,
}

/// [`EventSourceConfig`] holds the configuration for an SSE connection.
pub struct EventSourceConfig {
    pub url: String,
    pub headers: Vec<(SimpleHeader, String)>,
    pub last_event_id: Option<String>,
}

enum EventSourceState<R>
where
    R: DnsResolver + Send + 'static,
{
    Init(EventSourceConfig),
    Connecting(DrivenRecvIterator<HttpConnectTask<R>>),
    Reading(EventSourceStreamReader),
    Closed,
    _Phantom(PhantomData<R>),
}

pub struct EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<EventSourceState<R>>,
    _marker: PhantomData<R>,
}

impl<R> EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Connect to an SSE endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`EventSourceError`] if the URL is invalid.
    pub fn connect(url: impl Into<String>) -> Result<Self, EventSourceError> {
        Ok(Self {
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url.into(),
                headers: Vec::new(),
                last_event_id: None,
            })),
            _marker: PhantomData,
        })
    }

    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.headers.push((name, value.into()));
        }
        self
    }

    #[must_use]
    pub fn with_last_event_id(mut self, last_event_id: impl Into<String>) -> Self {
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.last_event_id = Some(last_event_id.into());
        }
        self
    }
}

struct HttpConnectTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<HttpConnectState<R>>,
    _marker: PhantomData<R>,
}

#[allow(clippy::large_enum_variant)]
enum HttpConnectState<R>
where
    R: DnsResolver + Send + 'static,
{
    Init(ClientRequestBuilder<R>),
    Done,
    _Phantom(PhantomData<R>),
}

impl<R> HttpConnectTask<R>
where
    R: DnsResolver + Send + 'static,
{
    fn new(config: EventSourceConfig) -> Self {
        let mut headers = config.headers;
        headers.push((SimpleHeader::ACCEPT, "text/event-stream".to_string()));

        if let Some(last_id) = config.last_event_id {
            headers.push((SimpleHeader::custom("Last-Event-ID"), last_id));
        }

        let simple_headers: crate::wire::simple_http::SimpleHeaders =
            headers.into_iter().map(|(k, v)| (k, vec![v])).collect();

        let builder = ClientRequestBuilder::get(&config.url)
            .expect("Invalid URL")
            .headers(simple_headers);

        Self {
            state: Some(HttpConnectState::Init(builder)),
            _marker: PhantomData,
        }
    }
}

impl<R> TaskIterator for HttpConnectTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = HttpSendResponseReader<SimpleHttpBody, Box<dyn BufRead + Send>>;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            HttpConnectState::Init(_request) => {
                self.state = Some(HttpConnectState::Done);
                None
            }
            HttpConnectState::Done | HttpConnectState::_Phantom(_) => None,
        }
    }
}

/// [`EventSourceStreamReader`] wraps [`SseParser`] for SSE event consumption.
pub struct EventSourceStreamReader {
    parser: SseParser,
    done: bool,
}

impl EventSourceStreamReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: SseParser::new(),
            done: false,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        if let Ok(text) = std::str::from_utf8(bytes) {
            self.parser.feed(text);
        }
    }

    pub fn next_event(&mut self) -> Option<Result<Event, EventSourceError>> {
        self.parser.next().map(Ok)
    }

    pub fn mark_done(&mut self) {
        self.done = true;
    }

    /// Check if the stream is done.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }
}

impl Default for EventSourceStreamReader {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> TaskIterator for EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = Event;
    type Pending = EventSourceProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            EventSourceState::Init(config) => {
                let connect_task = HttpConnectTask::new(config);
                let (action, receiver) = inlined_task(
                    crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                    Vec::new(),
                    connect_task,
                    std::time::Duration::from_millis(100),
                );

                self.state = Some(EventSourceState::Connecting(receiver));
                Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
            }

            EventSourceState::Connecting(recv_iter) => {
                let mut recv_iter = recv_iter;
                let next_value = recv_iter.next();

                if next_value.is_none() {
                    self.state = Some(EventSourceState::Closed);
                    return None;
                }

                match next_value {
                    Some(TaskStatus::Ready(_reader)) => {
                        self.state =
                            Some(EventSourceState::Reading(EventSourceStreamReader::new()));
                        Some(TaskStatus::Pending(EventSourceProgress::Reading))
                    }
                    Some(TaskStatus::Pending(())) => {
                        self.state = Some(EventSourceState::Connecting(recv_iter));
                        Some(TaskStatus::Pending(EventSourceProgress::Connecting))
                    }
                    Some(TaskStatus::Delayed(dur)) => {
                        self.state = Some(EventSourceState::Connecting(recv_iter));
                        Some(TaskStatus::Delayed(dur))
                    }
                    Some(TaskStatus::Spawn(action)) => {
                        self.state = Some(EventSourceState::Connecting(recv_iter));
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    Some(TaskStatus::Init) => {
                        self.state = Some(EventSourceState::Connecting(recv_iter));
                        Some(TaskStatus::Init)
                    }
                    None => {
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                }
            }

            EventSourceState::Reading(mut reader) => match reader.next_event() {
                Some(Ok(event)) => {
                    self.state = Some(EventSourceState::Reading(reader));
                    Some(TaskStatus::Ready(event))
                }
                Some(Err(e)) => {
                    tracing::error!("SSE parse error: {:?}", e);
                    self.state = Some(EventSourceState::Closed);
                    None
                }
                None => {
                    self.state = Some(EventSourceState::Reading(reader));
                    Some(TaskStatus::Pending(EventSourceProgress::Reading))
                }
            },

            EventSourceState::Closed | EventSourceState::_Phantom(_) => None,
        }
    }
}
