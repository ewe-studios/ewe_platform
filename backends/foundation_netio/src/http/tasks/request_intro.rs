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

use std::sync::atomic::Ordering;
use std::sync::Arc;

use derive_more::From;

use crate::native::connection::ConnWaker;
use crate::netcap::RawStream;
use crate::shared::client::body_reader::drain_stream_iterator_from_send_safe;
use crate::shared::client::ResponseIntro;
use crate::http::HttpClientConnection;
use crate::shared::http::IncomingResponseParts;
use crate::shared::http::{
    HttpClientError, HttpReaderError, HttpResponseIntro, HttpResponseReader, SimpleHeaders,
    SimpleHttpBody,
};
use foundation_core::valtron::{BoolSignal, BoxedSendExecutionAction, TaskIterator, TaskStatus};

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

pub struct GetRequestIntroTask(Option<GetRequestIntroState>, Option<SimpleHttpBody>, usize);

impl GetRequestIntroTask {
    #[must_use]
    pub fn new(stream: HttpClientConnection) -> Self {
        Self(Some(GetRequestIntroState::Init(Some(stream))), None, 5)
    }

    #[must_use]
    pub fn with_max_loop(mut self, max_loop: usize) -> Self {
        self.2 = max_loop;
        self
    }

    #[must_use]
    pub fn with_body_config(mut self, body: SimpleHttpBody) -> Self {
        self.1 = Some(body);
        self
    }

    /// Park the task on a read-readiness signal after a non-blocking `WouldBlock`.
    ///
    /// WHY: the reader kept its (resumable) state; we must re-run it once the socket
    /// is readable. Stores `resume` as the next state, registers a `BoolSignal`-backed
    /// waker on the connection, and returns `TaskStatus::Depends(signal)` — the
    /// `TunnelDriver` flips the signal and the executor unparks us. If the transport
    /// does not support readiness wakeups (a blocking `TcpStream` whose read timed
    /// out), the `WouldBlock` is a genuine failure, so we fail instead of parking
    /// forever — and the task terminates: `resume` is only stored when parking.
    fn park_on_would_block(
        &mut self,
        conn: &HttpClientConnection,
        resume: GetRequestIntroState,
    ) -> TaskStatus<RequestIntro, (), BoxedSendExecutionAction> {
        let signal = BoolSignal::new(false);
        let flag = signal.clone_inner();
        let waker: ConnWaker = Arc::new(move || flag.store(true, Ordering::SeqCst));

        if conn.register_read_waker(waker) {
            tracing::trace!("[INTRO] read would block; parking on connection read waker");
            self.0 = Some(resume);
            TaskStatus::Depends(Arc::new(signal))
        } else {
            // Terminal failure: leave the state empty so the next `next_status`
            // returns `None` and the task ends. Keeping `resume` here re-drove the
            // blocking read after every timeout and emitted a fresh `Failed` each
            // time — an unbounded retry loop that hung the caller forever.
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

impl TaskIterator for GetRequestIntroTask {
    type Pending = ();
    type Ready = RequestIntro;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            GetRequestIntroState::Init(inner) => match inner {
                Some(stream) => {
                    tracing::trace!("[INTRO] Getting next status for request intro");

                    let body_config = self.1.take().unwrap_or_default();
                    let reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream.clone_stream(),
                        body_config,
                    );

                    // Defer the first read to `IntroReading` so a non-blocking
                    // `WouldBlock` there can park and resume on the *same* reader
                    // (F11 non-blocking parking).
                    self.0 = Some(GetRequestIntroState::IntroReading(Box::new((reader, stream))));
                    Some(TaskStatus::Pending(()))
                }
                None => None,
            },
            GetRequestIntroState::IntroReading(inner) => {
                let (mut reader, stream) = *inner;

                let intro = match reader.next()? {
                    Ok(inner) => {
                        tracing::info!("Get intro from stream - got: {:?}", inner);
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
                        self.0 = Some(GetRequestIntroState::DrainingInterim(Box::new((
                            reader, stream,
                        ))));
                        return Some(TaskStatus::Pending(()));
                    }

                    let _ = self
                        .0
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
                self.0 = Some(GetRequestIntroState::IntroReading(Box::new((branched, stream))));
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use foundation_core::io::ioutils::ReadTimeoutOperations;
    use foundation_core::url::Uri;

    use crate::shared::client::SystemDnsResolver;

    /// Regression: a blocking transport whose read times out must yield exactly
    /// one `Ready(Failed(WouldBlock))` and then end (`None`).
    ///
    /// WHY: `park_on_would_block` used to store the resume state *before*
    /// checking whether the transport could park. On a blocking `TcpStream` it
    /// returned `Ready(Failed)` but left `IntroReading` in place, so the executor
    /// drove the task again: another blocking read, another timeout, another
    /// `Failed` — forever (CI `foundation_connectrpc` hung for 60 minutes logging
    /// "[INTRO] read would block on a blocking transport" every 3s).
    ///
    /// HOW: a loopback peer accepts and never writes; the client read timeout is
    /// 100ms. The task is driven with a hard iteration cap and a wall-clock bound,
    /// so a regression fails fast instead of hanging.
    #[test]
    fn blocking_transport_timeout_fails_once_and_terminates() {
        const MAX_STEPS: usize = 16;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");

        // Silent peer: accept, hold the socket open (never respond) until told to stop.
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let peer = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept client");
            // An explicit stop, the sender dropping, or the cap all end the wait;
            // which one it was does not matter to the peer.
            let _stopped = stop_rx.recv_timeout(Duration::from_secs(30));
            drop(socket);
        });

        let uri = Uri::parse(&format!("http://127.0.0.1:{}/", addr.port())).expect("uri");
        let mut conn = HttpClientConnection::connect(
            &uri,
            &SystemDnsResolver::new(),
            Some(Duration::from_secs(5)),
        )
        .expect("connect to silent peer");
        conn.stream_mut()
            .set_read_timeout_as(Duration::from_millis(100))
            .expect("set read timeout");

        let mut task = GetRequestIntroTask::new(conn);

        let started = Instant::now();
        let mut ready_failures = 0usize;
        let mut terminated = false;
        for _ in 0..MAX_STEPS {
            match task.next_status() {
                None => {
                    terminated = true;
                    break;
                }
                Some(TaskStatus::Ready(RequestIntro::Failed(HttpClientError::ReaderError(
                    HttpReaderError::WouldBlock,
                )))) => ready_failures += 1,
                Some(TaskStatus::Ready(RequestIntro::Failed(other))) => {
                    panic!("unexpected failure kind: {other:?}")
                }
                Some(TaskStatus::Ready(RequestIntro::Success { .. })) => {
                    panic!("a silent peer cannot produce a response")
                }
                Some(TaskStatus::Depends(_)) => {
                    panic!("a blocking TcpStream must not park on a read waker")
                }
                Some(_) => {}
            }
        }
        let elapsed = started.elapsed();

        stop_tx.send(()).expect("signal silent peer");
        peer.join().expect("silent peer thread");

        assert!(
            terminated,
            "task kept running after its terminal failure (unbounded retry loop)"
        );
        assert_eq!(
            ready_failures, 1,
            "exactly one terminal failure is reported"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "task took {elapsed:?}; expected a single ~100ms read timeout"
        );
    }
}
