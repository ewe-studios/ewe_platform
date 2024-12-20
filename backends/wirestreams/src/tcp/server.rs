use derive_more::From;
use std::{
    io::{BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
};

use crate::simple_http::{
    self, Http11, IncomingRequestParts, RenderHttp, ServiceAction, ServiceActionList, SimpleHeader,
    SimpleIncomingRequest, SimpleOutgoingResponse, Status, WrappedTcpStream,
};

pub type TestServerResult<T> = std::result::Result<T, TestServerError>;

#[derive(From, Debug)]
pub enum TestServerError {
    FailedListenerSetup,
}

impl std::error::Error for TestServerError {}

impl core::fmt::Display for TestServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct TestServer {
    port: usize,
    address: String,
    actions: Vec<crate::simple_http::ServiceAction>,
    listener: Option<JoinHandle<()>>,
    requests: Option<mpsc::Receiver<crate::simple_http::SimpleIncomingRequest>>,
}

impl TestServer {
    pub fn serve(&mut self) {
        if let Some(_) = &self.listener {
            return;
        }

        let port = self.port;
        let address = self.address.clone();
        let actions = self.actions.clone();

        let (tx, rx) = mpsc::channel();
        self.requests = Some(rx);

        self.listener = Some(thread::spawn(move || {
            let listener =
                TcpListener::bind(format!("{}:{}", address, port)).expect("create tcp listener");
            for stream_result in listener.incoming() {
                let stream = stream_result.unwrap();

                let mut buffer = [0; 512];
                stream.peek(&mut buffer).unwrap();

                if buffer.starts_with(b"CLOSE") {
                    break;
                }

                Self::serve_connection(stream, actions.clone(), tx.clone());
            }
        }));
    }

    fn serve_connection(
        read_stream: TcpStream,
        actions: Vec<ServiceAction>,
        sender: mpsc::Sender<SimpleIncomingRequest>,
    ) {
        let action_list = ServiceActionList::new(actions);

        let mut write_stream = read_stream
            .try_clone()
            .expect("should be able to clone connection");

        let mut request_reader = simple_http::HttpReader::simple_tcp_stream(BufReader::new(
            WrappedTcpStream::new(read_stream),
        ));

        loop {
            // fetch the intro portion and validate we have resources for processing request
            // if not, just break and return an error
            if let Some(Ok(IncomingRequestParts::Intro(method, url, proto))) = request_reader.next()
            {
                if let Some(resource) = action_list.get_one_matching2(&url, method.clone()) {
                    if let Some(Ok(IncomingRequestParts::Headers(headers))) = request_reader.next()
                    {
                        if let Some(Ok(IncomingRequestParts::Body(body))) = request_reader.next() {
                            if let Ok(request) = SimpleIncomingRequest::builder()
                                .with_headers(headers)
                                .with_some_body(body)
                                .with_url(url)
                                .with_proto(proto)
                                .with_method(method)
                                .build()
                            {
                                continue;
                            }
                        }
                    }
                }
            };

            // if we ever get here, just break.
            break;
        }

        let response = Http11::response(
            SimpleOutgoingResponse::builder()
                .with_status(Status::BadRequest)
                .build()
                .unwrap(),
        );

        if let Ok(renderer) = response.http_render() {
            for part in renderer {
                match part {
                    Ok(data) => match write_stream.write(&data) {
                        Ok(_) => continue,
                        Err(_) => return,
                    },
                    Err(_) => return,
                }
            }
        }
    }
}
