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
use crate::valtron::{NoSpawner, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{HttpClientConnection, ResponseIntro};
use crate::wire::simple_http::{
    HttpClientError, HttpReaderError, HttpResponseIntro, HttpResponseReader, SimpleHeaders,
    SimpleHttpBody,
};

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

pub struct GetRequestIntroTask(Option<GetRequestIntroState>, Option<SimpleHttpBody>);

impl GetRequestIntroTask {
    #[must_use]
    pub fn new(stream: HttpClientConnection) -> Self {
        Self(Some(GetRequestIntroState::Init(Some(stream))), None)
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
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            GetRequestIntroState::Init(inner) => match inner {
                Some(stream) => {
                    tracing::info!("Creating http response reader from stream");
                    let body_config = self.1.take().unwrap_or_default();
                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream.clone_stream(),
                        body_config,
                    );

                    tracing::info!("Get intro from stream");
                    let intro = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => return Some(TaskStatus::Ready(err.into())),
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Intro(status, proto, text) =
                        intro
                    else {
                        tracing::info!("Failed to read intro from stream");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed.into(),
                        )));
                    };

                    tracing::info!("Received intro for request: {:?}", (&status, &proto, &text));

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
                    tracing::info!("Read request header from stream");
                    let header_response = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => {
                            return Some(TaskStatus::Ready(RequestIntro::Failed(err.into())))
                        }
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Headers(headers) =
                        header_response
                    else {
                        tracing::info!("No header received or failed reading");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed.into(),
                        )));
                    };

                    tracing::info!("Received headers and setting success state");
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
