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
use crate::valtron::{NoSpawner, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::HttpClientConnection;
use crate::wire::simple_http::{
    HttpReaderError, HttpResponseIntro, HttpResponseReader, SimpleHeaders, SimpleHttpBody,
};

pub enum RequestIntro {
    Success {
        stream: HttpResponseReader<SimpleHttpBody, RawStream>,
        /// Connection
        conn: HttpClientConnection,
        /// intro options  for a http response
        intro: HttpResponseIntro,
        /// headers retrieved from the stream.
        headers: SimpleHeaders,
    },

    Failed(HttpReaderError),
}

pub enum GetRequestIntroState {
    Init(Option<HttpClientConnection>),
    WithIntro(
        Option<(
            HttpResponseReader<SimpleHttpBody, RawStream>,
            HttpResponseIntro,
            HttpClientConnection,
        )>,
    ),
}

pub struct GetRequestIntroTask(Option<GetRequestIntroState>);

impl GetRequestIntroTask {
    #[must_use]
    pub fn new(stream: HttpClientConnection) -> Self {
        Self(Some(GetRequestIntroState::Init(Some(stream))))
    }
}

impl TaskIterator for GetRequestIntroTask {
    type Pending = ();
    type Ready = RequestIntro;
    type Spawner = NoSpawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            GetRequestIntroState::Init(inner) => match inner {
                Some(stream) => {
                    tracing::info!("Creating http response reader from stream");
                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream.clone_stream(),
                        SimpleHttpBody,
                    );

                    tracing::info!("Get intro from stream");
                    let intro = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => return Some(TaskStatus::Ready(RequestIntro::Failed(err))),
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Intro(status, proto, text) =
                        intro
                    else {
                        tracing::info!("Failed to read intro from stream");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    };

                    tracing::info!("Received intro for request: {:?}", (&status, &proto, &text));

                    let _ = self.0.replace(GetRequestIntroState::WithIntro(Some((
                        reader,
                        (status, proto, text),
                        stream,
                    ))));

                    Some(TaskStatus::Pending(()))
                }
                None => None,
            },
            GetRequestIntroState::WithIntro(inner) => match inner {
                Some((mut reader, intro, conn)) => {
                    tracing::info!("Read request header from stream");
                    let header_response = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => return Some(TaskStatus::Ready(RequestIntro::Failed(err))),
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Headers(headers) =
                        header_response
                    else {
                        tracing::info!("No header received or failed reading");
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    };

                    tracing::info!("Received headers and setting success state");
                    Some(TaskStatus::Ready(RequestIntro::Success {
                        stream: reader,
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
