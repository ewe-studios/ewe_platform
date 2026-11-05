#![allow(clippy::type_complexity)]
#![cfg(not(target_family = "wasm"))]

use derive_more::From;
use foundation_core::extensions::result_ext::{SendableBoxedError, SendableBoxedResult};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    http_streams, is_sub_set_of_other_header, HttpReaderError, SendSafeBody,
};
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
};

use foundation_core::extensions::result_ext::BoxedError;
use foundation_netio::shared::http::{
    Http11, IncomingRequestParts, RenderHttp, RequestDescriptor, ServiceAction, ServiceActionList,
    SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, Status,
};

/// Result type for test server operations.
pub type TestServerResult<T> = std::result::Result<T, TestServerError>;

/// Errors that can occur when setting up a test server.
#[derive(From, Debug)]
pub enum TestServerError {
    /// The TCP listener failed to bind to the requested address.
    FailedListenerSetup,
}

impl std::error::Error for TestServerError {}

impl core::fmt::Display for TestServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/**
 A lightweight HTTP server used in tests that serves a collection of predefined
 `ServiceAction` resources on a given address and port.

 This server is intended for test scenarios where you need to simulate HTTP
 endpoints and capture incoming request descriptors.
*/
pub struct ResourcesHttpServer {
    port: usize,
    address: String,
    actions: Vec<ServiceAction>,
}

impl ResourcesHttpServer {
    /// Create a new test server with the given port, address, and service actions.
    #[must_use]
    pub fn new(port: usize, address: String, actions: Vec<ServiceAction>) -> Self {
        Self {
            port,
            address,
            actions,
        }
    }

    pub fn close(&self) -> Result<(), BoxedError> {
        let port = self.port;
        let address = self.address.clone();
        let mut client = TcpStream::connect(format!("{address}:{port}"))
            .map_err(foundation_core::extensions::result_ext::BoxedResult::into_boxed_error)?;

        client
            .write_all(b"CLOSE\r\n")
            .map_err(foundation_core::extensions::result_ext::BoxedResult::into_boxed_error)
    }

    #[must_use]
    pub fn serve(
        &self,
    ) -> (
        JoinHandle<Result<(), SendableBoxedError>>,
        mpsc::Receiver<RequestDescriptor>,
        mpsc::Receiver<JoinHandle<()>>,
    ) {
        let port = self.port;
        let address = self.address.clone();
        let actions = self.actions.clone();

        let (tx, rx) = mpsc::channel::<RequestDescriptor>();
        let (workers_tx, workers_rx) = mpsc::channel::<JoinHandle<()>>();

        let listener = TcpListener::bind(format!("{address}:{port}")).expect("create tcp listener");

        (
            thread::spawn(move || {
                for stream_result in listener.incoming() {
                    match stream_result {
                        Ok(stream) => {
                            let mut buffer = [0; 512];
                            stream.peek(&mut buffer).unwrap();

                            if buffer.starts_with(b"CLOSE") {
                                break;
                            }

                            workers_tx
                                .send(Self::serve_connection(stream, actions.clone(), tx.clone()))
                                .expect("should save worker handler");
                        }
                        Err(err) => return Err(err.into_sendable_boxed_error()),
                    }
                }
                Ok(())
            }),
            rx,
            workers_rx,
        )
    }

    fn serve_connection(
        read_stream: TcpStream,
        actions: Vec<ServiceAction>,
        sender: mpsc::Sender<RequestDescriptor>,
    ) -> JoinHandle<()> {
        let action_list = ServiceActionList::new(actions);

        thread::spawn(move || {
            let mut write_stream = read_stream
                .try_clone()
                .expect("should be able to clone connection");

            let conn = RawStream::from_tcp(read_stream).expect("should wrap tcp stream");
            let request_streams = http_streams::send::http_streams(conn);

            loop {
                // fetch the intro portion and validate we have resources for processing request
                // if not, just break and return an error
                let request_reader = request_streams.next_request();
                tracing::debug!("Pulled next request");

                let parts: Result<Vec<IncomingRequestParts>, HttpReaderError> = request_reader
                    .into_iter()
                    .filter(|item| match item {
                        Ok(IncomingRequestParts::SKIP) => false,
                        Ok(_) | Err(_) => true,
                    })
                    .collect();

                tracing::debug!("Collected all parts of request");
                if let Err(part_err) = parts {
                    tracing::error!("Failed to read requests from reader due to: {:?}", part_err);
                    break;
                }

                let mut request_parts = parts.unwrap();
                if request_parts.len() != 3 {
                    tracing::error!(
                        "Failed to receive expected request parts of 3: {:?}",
                        &request_parts
                    );
                    break;
                }

                let body_part = request_parts.pop().unwrap();
                let headers_part = request_parts.pop().unwrap();
                let intros_part = request_parts.pop().unwrap();

                let IncomingRequestParts::Intro(method, url, proto) = intros_part else {
                    tracing::error!("Failed to receive a IncomingRequestParts::Intro(_, _, _)");
                    return;
                };

                tracing::info!(
                    "Received new http request for proto: method: {:?}, url: {:?}, proto: {:?}",
                    method,
                    url,
                    proto,
                );

                // allow custom protocols.
                //
                // if proto != Proto::HTTP11 {
                //     break;
                // }

                let Some(resource) = action_list.get_one_matching2(&url, method.clone()) else {
                    tracing::debug!("No matching resource found");
                    break;
                };

                let IncomingRequestParts::Headers(headers) = headers_part else {
                    tracing::error!("Failed to receive a IncomingRequestParts::Headers(_)");
                    break;
                };

                if let Some(resource_headers) = &resource.headers {
                    if !is_sub_set_of_other_header(resource_headers, &headers) {
                        tracing::error!("Headers do not match expected");
                        break;
                    }
                }

                let body = match body_part {
                    IncomingRequestParts::NoBody => SendSafeBody::None,
                    IncomingRequestParts::SizedBody(inner) => inner,
                    IncomingRequestParts::StreamedBody(inner) => inner,
                    _ => unreachable!("should never trigger this clause"),
                };

                if let Ok(request) = SimpleIncomingRequest::builder()
                    .with_headers(headers)
                    .with_url(url.clone())
                    .with_proto(proto.clone())
                    .with_method(method.clone())
                    .with_body(body)
                    .build()
                {
                    sender
                        .send(request.descriptor())
                        .expect("should sent request");

                    let outgoing_response = match resource.body.clone_box().handle(request) {
                        Ok(outgoing) => outgoing,
                        Err(err) => Self::internal_server_error_response(err),
                    };

                    let response = Http11::response(outgoing_response);
                    match response.http_render() {
                        Ok(renderer) => {
                            for part in renderer {
                                match part {
                                    Ok(data) => {
                                        if let Err(err) = write_stream.write_all(&data) {
                                            tracing::error!(
                                                "Failed to write response to {:?}: {:?}",
                                                &url,
                                                err
                                            );
                                            return;
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "Failed to render response to {:?}: {:?}",
                                            &url,
                                            err
                                        );
                                        return;
                                    }
                                }
                            }
                            tracing::info!("Finished sending response to: {:?}", &url);
                            return;
                        }
                        Err(err) => {
                            tracing::info!(
                                "Failed to send response to: {:?} dueto err={:?}",
                                &url,
                                err
                            );
                            return;
                        }
                    }
                }

                // if we ever get here, just break.
                tracing::info!("Request processing finished");
            }

            // A body-less response gets no implicit `Content-Length`, and an
            // HTTP/1.1 response without headers cannot be rendered, so the
            // empty body is declared explicitly.
            let response = Http11::response(
                SimpleOutgoingResponse::builder()
                    .with_status(Status::BadRequest)
                    .add_header(SimpleHeader::CONTENT_LENGTH, "0")
                    .build()
                    .expect("a status and a header are all a response needs"),
            );

            match response.http_render() {
                Ok(renderer) => {
                    for part in renderer {
                        match part {
                            Ok(data) => {
                                if let Err(err) = write_stream.write_all(&data) {
                                    tracing::error!("Failed to write 400 response: {:?}", err);
                                    return;
                                }
                            }
                            Err(err) => {
                                tracing::error!("Failed to render 400 response: {:?}", err);
                                return;
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("Failed to render 400 response: {:?}", err);
                }
            }
        })
    }

    fn internal_server_error_response(
        err: foundation_core::extensions::result_ext::BoxedError,
    ) -> SimpleOutgoingResponse {
        SimpleOutgoingResponse::builder()
            .with_status(Status::InternalServerError)
            .with_body(SendSafeBody::Text(format!("{err:?}")))
            .build()
            .expect("should generate request")
    }
}
