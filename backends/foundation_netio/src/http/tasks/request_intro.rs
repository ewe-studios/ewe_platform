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

use std::time::Duration;

use derive_more::From;

use super::nonblocking::{self, IdleDeadline, Park};
use crate::native::readiness::IoInterest;
use crate::shared::client::ClientConfig;
use crate::netcap::RawStream;
use crate::shared::client::body_reader::drain_stream_iterator_from_send_safe;
use crate::shared::client::ResponseIntro;
use crate::http::HttpClientConnection;
use crate::shared::http::IncomingResponseParts;
use crate::shared::http::{
    HttpClientError, HttpReaderError, HttpResponseIntro, HttpResponseReader, SimpleHeaders,
    SimpleHttpBody,
};
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};

/// Cloneable subset of `RequestIntro` for observer patterns.
///
/// WHY: `split_collect_until` requires Clone on the data sent to observer,
/// but `RequestIntro::Success` contains non-cloneable stream.
///
/// WHAT: Contains only the cloneable parts: conn, intro, headers.
///
/// HOW: Extracted from `RequestIntro::Success` for observer, keeps stream for continuation.
#[derive(Clone, Debug)]
pub struct RequestIntroData {
    pub conn: HttpClientConnection,
    pub intro: ResponseIntro,
    pub headers: SimpleHeaders,
}

#[derive(From)]
pub enum RequestIntro {
    Success {
        stream: Box<HttpResponseReader<SimpleHttpBody, RawStream>>,
        /// Connection
        conn: HttpClientConnection,
        /// intro options  for a http response
        intro: HttpResponseIntro,
        /// headers retrieved from the stream.
        headers: SimpleHeaders,
    },

    Failed(HttpClientError),
}

impl RequestIntro {
    /// Extract cloneable data from Success variant.
    ///
    /// Returns None for Failed variant or if stream is not available.
    #[must_use]
    pub fn to_cloneable_data(&self) -> Option<RequestIntroData> {
        match self {
            Self::Success {
                conn,
                intro,
                headers,
                ..
            } => Some(RequestIntroData {
                conn: conn.clone(),
                intro: intro.clone().into(),
                headers: headers.clone(),
            }),
            Self::Failed(_) => None,
        }
    }
}

impl From<HttpReaderError> for RequestIntro {
    fn from(error: HttpReaderError) -> Self {
        Self::Failed(HttpClientError::ReaderError(error))
    }
}

// Type alias for complex WithIntro data
type WithIntroData = Box<
    Option<(
        HttpResponseReader<SimpleHttpBody, RawStream>,
        HttpResponseIntro,
        HttpClientConnection,
    )>,
>;

pub enum GetRequestIntroState {
    Init(Option<HttpClientConnection>),
    /// The intro reader was created but the status line hasn't arrived yet
    /// (non-blocking transport returned `WouldBlock`). The reader is resumable, so
    /// it is parked here and re-driven when the read waker fires. See F11
    /// "Non-blocking transport integration".
    IntroReading(Box<(HttpResponseReader<SimpleHttpBody, RawStream>, HttpClientConnection)>),
    /// A 1xx interim response was read; drain the rest of it (headers/body) before
    /// branching the reader and re-reading the next response's intro. Parking-aware.
    DrainingInterim(Box<(HttpResponseReader<SimpleHttpBody, RawStream>, HttpClientConnection)>),
    WithIntro(WithIntroData),
}

pub struct GetRequestIntroTask {
    state: Option<GetRequestIntroState>,
    body_config: Option<SimpleHttpBody>,
    max_loop: usize,
    /// Inactivity bound while parked waiting for the response head on a
    /// non-blocking socket (the blocking path relies on `SO_RCVTIMEO` instead).
    idle: IdleDeadline,
}

impl GetRequestIntroTask {
    #[must_use]
    pub fn new(stream: HttpClientConnection) -> Self {
        Self {
            state: Some(GetRequestIntroState::Init(Some(stream))),
            body_config: None,
            max_loop: 5,
            idle: IdleDeadline::new(ClientConfig::default().response_head_timeout()),
        }
    }

    #[must_use]
    pub fn with_max_loop(mut self, max_loop: usize) -> Self {
        self.max_loop = max_loop;
        self
    }

    #[must_use]
    pub fn with_body_config(mut self, body: SimpleHttpBody) -> Self {
        self.body_config = Some(body);
        self
    }

    /// How long to wait, with no bytes arriving, for the response head on a
    /// non-blocking connection before failing with `HttpClientError::Timeout`.
    #[must_use]
    pub fn with_head_timeout(mut self, timeout: Duration) -> Self {
        self.idle = IdleDeadline::new(timeout);
        self
    }

    /// Park the task on read readiness after a non-blocking `WouldBlock`.
    ///
    /// WHY: the reader kept its resumable state, so it must run again once the
    /// socket is readable. This stores `resume` as the next state and returns
    /// `TaskStatus::Depends` on "readable, or the head deadline passed". The
    /// executor then holds the task off the worker until one of those happens.
    ///
    /// Two `WouldBlock`s are terminal, and in both the task ends because `resume`
    /// is dropped. One comes from a blocking socket whose read timeout already
    /// elapsed (or a transport that cannot report readiness). The other comes once
    /// the inactivity deadline has passed. Keeping `resume` in those cases
    /// re-drove the read after every timeout. Each pass emitted a fresh
    /// `Failed`: an unbounded retry loop that hung the caller forever (#76).
    fn park_on_would_block(
        &mut self,
        conn: &HttpClientConnection,
        resume: GetRequestIntroState,
    ) -> TaskStatus<RequestIntro, ()> {
        match nonblocking::park(conn, IoInterest::Readable, &mut self.idle) {
            Park::On(signal) => {
                tracing::trace!("[INTRO] response not ready; parking on socket readiness");
                self.state = Some(resume);
                TaskStatus::Depends(signal)
            }
            Park::Expired => {
                drop(resume);
                tracing::error!(
                    timeout = ?self.idle.timeout(),
                    "[INTRO] no response head before the deadline"
                );
                TaskStatus::Ready(RequestIntro::Failed(HttpClientError::Timeout))
            }
            Park::Unsupported => {
                drop(resume);
                tracing::error!(
                    "[INTRO] read would block on a blocking transport; treating as failure"
                );
                TaskStatus::Ready(RequestIntro::Failed(HttpClientError::ReaderError(
                    HttpReaderError::WouldBlock,
                )))
            }
        }
    }
}

impl TaskIterator for GetRequestIntroTask {
    type Pending = ();
    type Ready = RequestIntro;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.state.take()? {
            GetRequestIntroState::Init(inner) => match inner {
                Some(stream) => {
                    tracing::trace!("[INTRO] Getting next status for request intro");

                    let body_config = self.body_config.take().unwrap_or_default();
                    let reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream.clone_stream(),
                        body_config,
                    );

                    // Defer the first read to `IntroReading` so a non-blocking
                    // `WouldBlock` there can park and resume on the *same* reader
                    // (F11 non-blocking parking).
                    self.state = Some(GetRequestIntroState::IntroReading(Box::new((reader, stream))));
                    Some(TaskStatus::Pending(()))
                }
                None => None,
            },
            GetRequestIntroState::IntroReading(inner) => {
                let (mut reader, stream) = *inner;

                let intro = match reader.next()? {
                    Ok(inner) => {
                        tracing::info!("Get intro from stream - got: {:?}", inner);
                        self.idle.progress();
                        inner
                    }
                    // No status line yet on a non-blocking transport: the reader kept
                    // its state (nothing consumed), so park and retry when readable.
                    Err(HttpReaderError::WouldBlock) => {
                        let waker_conn = stream.clone();
                        let resume =
                            GetRequestIntroState::IntroReading(Box::new((reader, stream)));
                        return Some(self.park_on_would_block(&waker_conn, resume));
                    }
                    Err(err) => {
                        tracing::error!("Get intro from stream - error: {:?}", err);
                        return Some(TaskStatus::Ready(err.into()));
                    }
                };

                    let IncomingResponseParts::Intro(status, proto, text) = intro
                    else {
                        tracing::info!("Failed to read intro from stream");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed.into(),
                        )));
                    };

                    tracing::info!(
                        "[INTRO] Received intro for request: {:?}",
                        (&status, &proto, &text)
                    );

                    // Skip 1xx interim responses (100 Continue, 102 Processing, 103
                    // Early Hints). RFC 9110 §15.2: a client must parse and discard any
                    // 1xx received before the final response. 101 Switching Protocols is
                    // terminal (a protocol upgrade) and is NOT skipped. The interim is
                    // drained in the parking-aware `DrainingInterim` state, after which
                    // we branch the reader and re-read the next intro — so a not-yet-
                    // arrived interim body or final status simply parks.
                    let code = status.clone().into_usize();
                    if (100..200).contains(&code) && code != 101 {
                        tracing::info!(
                            "[INTRO] skipping 1xx interim response {:?}; draining then re-reading",
                            (&status, &proto, &text)
                        );
                        self.state = Some(GetRequestIntroState::DrainingInterim(Box::new((
                            reader, stream,
                        ))));
                        return Some(TaskStatus::Pending(()));
                    }

                    let _ = self
                        .state
                        .replace(GetRequestIntroState::WithIntro(Box::new(Some((
                            reader,
                            (status, proto, text),
                            stream,
                        )))));

                    Some(TaskStatus::Pending(()))
            }
            GetRequestIntroState::DrainingInterim(inner) => {
                let (mut reader, stream) = *inner;

                // Consume the remainder of the interim (1xx) response — its headers
                // and any body — until the reader signals the response is complete
                // (`None`). A `WouldBlock` here parks and resumes this same drain.
                loop {
                    match reader.next() {
                        Some(Ok(IncomingResponseParts::StreamedBody(body))) => {
                            self.idle.progress();
                            if let Err(err) = drain_stream_iterator_from_send_safe(body) {
                                tracing::error!(
                                    "[INTRO] failed to drain interim (1xx) body: {:?}",
                                    err
                                );
                                return Some(TaskStatus::Ready(RequestIntro::Failed(
                                    HttpReaderError::ReadFailed.into(),
                                )));
                            }
                        }
                        Some(Ok(_)) => {
                            // Interim headers / skipped parts — keep draining.
                            self.idle.progress();
                        }
                        Some(Err(HttpReaderError::WouldBlock)) => {
                            let waker_conn = stream.clone();
                            let resume = GetRequestIntroState::DrainingInterim(Box::new((
                                reader, stream,
                            )));
                            return Some(self.park_on_would_block(&waker_conn, resume));
                        }
                        Some(Err(err)) => {
                            tracing::error!("[INTRO] failed to drain interim (1xx): {:?}", err);
                            return Some(TaskStatus::Ready(RequestIntro::Failed(
                                HttpReaderError::ReadFailed.into(),
                            )));
                        }
                        None => break,
                    }
                }

                // Reset the reader to read the next response's status line, then
                // re-enter the parking-aware intro path.
                let branched = reader.branch_reader();
                self.state = Some(GetRequestIntroState::IntroReading(Box::new((branched, stream))));
                Some(TaskStatus::Pending(()))
            }
            GetRequestIntroState::WithIntro(inner) => match *inner {
                Some((mut reader, intro, conn)) => {
                    let header_response = match reader.next()? {
                        Ok(inner) => inner,
                        // Headers block not fully buffered yet: park and resume.
                        Err(HttpReaderError::WouldBlock) => {
                            let waker_conn = conn.clone();
                            let resume = GetRequestIntroState::WithIntro(Box::new(Some((
                                reader, intro, conn,
                            ))));
                            return Some(self.park_on_would_block(&waker_conn, resume));
                        }
                        Err(err) => {
                            return Some(TaskStatus::Ready(RequestIntro::Failed(err.into())))
                        }
                    };

                    let crate::shared::http::IncomingResponseParts::Headers(headers) =
                        header_response
                    else {
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed.into(),
                        )));
                    };

                    Some(TaskStatus::Ready(RequestIntro::Success {
                        stream: Box::new(reader),
                        conn,
                        intro,
                        headers,
                    }))
                }
                None => None,
            },
        }
    }
}
