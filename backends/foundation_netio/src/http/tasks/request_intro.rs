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

use derive_more::From;

use crate::netcap::RawStream;
use crate::shared::client::body_reader::drain_stream_iterator_from_send_safe;
use crate::shared::client::ResponseIntro;
use crate::simple_http::client::HttpClientConnection;
use crate::simple_http::shared::IncomingResponseParts;
use crate::simple_http::shared::{
    HttpClientError, HttpReaderError, HttpResponseIntro, HttpResponseReader, SimpleHeaders,
    SimpleHttpBody, Status,
};
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};

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
            RequestIntro::Success {
                conn,
                intro,
                headers,
                ..
            } => Some(RequestIntroData {
                conn: conn.clone(),
                intro: intro.clone().into(),
                headers: headers.clone(),
            }),
            RequestIntro::Failed(_) => None,
        }
    }
}

impl From<HttpReaderError> for RequestIntro {
    fn from(error: HttpReaderError) -> Self {
        RequestIntro::Failed(HttpClientError::ReaderError(error))
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
                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream.clone_stream(),
                        body_config,
                    );

                    let intro = match reader.next()? {
                        Ok(inner) => {
                            tracing::info!("Get intro from stream - got: {:?}", inner);
                            inner
                        }
                        Err(err) => {
                            tracing::error!("Get intro from stream - error: {:?}", err);
                            return Some(TaskStatus::Ready(err.into()));
                        }
                    };

                    let IncomingResponseParts::Intro(mut status, mut proto, mut text) = intro
                    else {
                        tracing::info!("Failed to read intro from stream");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed.into(),
                        )));
                    };

                    tracing::info!(
                        "[PROCESSING CHECK] Received intro for request: {:?}",
                        (&status, &proto, &text)
                    );

                    // if we see Processing then, lets pull the body then re-run the pull step
                    if status == Status::Processing {
                        tracing::info!(
                            "[PROCESSING CHECK] Entering state of Status::Processing: {:?}",
                            (&status, &proto, &text)
                        );

                        // loop and collect the next until you see another intro
                        // and if its not a Status::Processing, then stop.
                        for next_state in &mut reader {
                            tracing::trace!(
                                "[PROCESSING CHECK] Got next state of request: {:?}",
                                &next_state
                            );

                            let next_item = match next_state {
                                Ok(item) => item,
                                Err(err) => {
                                    tracing::error!(
                                        "[PROCESSING CHECK] Failed to read next body from 102 status due to: {:?}",
                                        err
                                    );
                                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                                        HttpReaderError::ReadFailed.into(),
                                    )));
                                }
                            };

                            if let IncomingResponseParts::StreamedBody(stream) = next_item {
                                tracing::info!(
                                    "[PROCESSING CHECK] Saw next body under Status::Processing state: {:?}",
                                    &stream,
                                );

                                if let Err(err) = drain_stream_iterator_from_send_safe(stream) {
                                    tracing::error!(
                                        "[PROCESSING CHECK] Failed to drain body from 102 status due to: {:?}",
                                        err
                                    );
                                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                                        HttpReaderError::ReadFailed.into(),
                                    )));
                                }
                            } else {
                                tracing::trace!(
                                    "[PROCESSING CHECK] Skipping body state from request under Status::Processing"
                                );
                                continue;
                            }
                        }

                        tracing::trace!("[PROCESSING CHECK] branching reader");

                        reader = reader.branch_reader();

                        tracing::trace!("[PROCESSING CHECK] read new intro from branched reader");
                        match reader.next()? {
                            Ok(IncomingResponseParts::Intro(
                                next_status,
                                next_proto,
                                next_text,
                            )) => {
                                tracing::info!(
                                    "Get intro from stream - got: {:?}",
                                    (&next_status, &next_proto, &next_text),
                                );
                                // if we've reached max or not Status::Processing then stop
                                status = next_status;
                                proto = next_proto;
                                text = next_text;
                            }
                            Ok(_) => {
                                return Some(TaskStatus::Ready(RequestIntro::Failed(
                                    HttpClientError::ReadError,
                                )));
                            }
                            Err(err) => {
                                tracing::error!("Get intro from stream - error: {:?}", err);
                                return Some(TaskStatus::Ready(err.into()));
                            }
                        };

                        tracing::trace!("[PROCESSING CHECK] Finished Status::Processing");
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
                None => None,
            },
            GetRequestIntroState::WithIntro(inner) => match *inner {
                Some((mut reader, intro, conn)) => {
                    let header_response = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => {
                            return Some(TaskStatus::Ready(RequestIntro::Failed(err.into())))
                        }
                    };

                    let crate::simple_http::shared::IncomingResponseParts::Headers(headers) =
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
