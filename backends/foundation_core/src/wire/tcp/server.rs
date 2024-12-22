use derive_more::From;
use std::{
    io::{BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
};

use crate::{
    extensions::result_ext::{BoxedError, BoxedResult},
    wire::simple_http::{
        self, Http11, IncomingRequestParts, Proto, RenderHttp, ServiceAction, ServiceActionList,
        SimpleIncomingRequest, SimpleOutgoingResponse, SimpleServer, Status, WrappedTcpStream,
    },
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
    actions: Vec<ServiceAction>,
}

impl TestServer {
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
        let mut client = TcpStream::connect(format!("{}:{}", address, port))
            .map_err(|err| err.into_boxed_error())?;

        client
            .write(b"CLOSE\r\n")
            .map_err(|err| err.into_boxed_error())
            .map(|_| ())
    }

    pub fn serve(&self) -> (JoinHandle<()>, mpsc::Receiver<SimpleIncomingRequest>) {
        let port = self.port;
        let address = self.address.clone();
        let actions = self.actions.clone();

        let (tx, rx) = mpsc::channel::<SimpleIncomingRequest>();

        (
            thread::spawn(move || {
                let listener = TcpListener::bind(format!("{}:{}", address, port))
                    .expect("create tcp listener");
                for stream_result in listener.incoming() {
                    let stream = stream_result.unwrap();

                    let mut buffer = [0; 512];
                    stream.peek(&mut buffer).unwrap();

                    if buffer.starts_with(b"CLOSE") {
                        break;
                    }

                    Self::serve_connection(stream, actions.clone(), tx.clone());
                }
            }),
            rx,
        )
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

            let item = request_reader.next();
            tracing::info!("Request received: {:?}", &item);
            if let Some(Ok(IncomingRequestParts::Intro(method, url, proto))) = item {
                tracing::info!("Received new http request for proto: {}", proto);
                if proto != Proto::HTTP11 {
                    break;
                }

                let target_url = url.clone();
                let target_resource = action_list.get_one_matching2(&url, method.clone());
                tracing::info!(
                    "Checking match servicer for URL: {:?} -> {:?}",
                    &target_url,
                    target_resource
                );
                if let Some(resource) = action_list.get_one_matching2(&url, method.clone()) {
                    if let Some(Ok(IncomingRequestParts::Headers(headers))) = request_reader.next()
                    {
                        if let Some(Ok(IncomingRequestParts::Body(body))) = request_reader.next() {
                            if let Ok(request) = SimpleIncomingRequest::builder()
                                .with_headers(headers)
                                .with_url(url)
                                .with_proto(proto.clone())
                                .with_method(method)
                                .build()
                            {
                                let mut cloned_request = request.clone();
                                cloned_request.body = body;

                                sender.send(request).expect("should sent request");

                                let outgoing_response =
                                    match resource.body.clone_box().handle(cloned_request) {
                                        Ok(outgoing) => outgoing,
                                        Err(err) => Self::internal_server_error_response(err),
                                    };

                                let response = Http11::response(outgoing_response);
                                match response.http_render() {
                                    Ok(renderer) => {
                                        for part in renderer {
                                            match part {
                                                Ok(data) => match write_stream.write(&data) {
                                                    Ok(_) => continue,
                                                    Err(_) => return,
                                                },
                                                Err(_) => return,
                                            }
                                        }

                                        tracing::info!("Finished responding to http request");
                                    }
                                    Err(_) => {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }

                tracing::info!(
                    "Failed to find matching resource for URL: {:?}",
                    &target_url
                );
            };

            // if we ever get here, just break.
            tracing::warn!("Request processing failed for client");
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
                        Ok(_) => return,
                        Err(_) => return,
                    },
                    Err(_) => return,
                }
            }
        }
    }

    fn internal_server_error_response(
        err: crate::extensions::result_ext::BoxedError,
    ) -> SimpleOutgoingResponse {
        SimpleOutgoingResponse::builder()
            .with_status(Status::InternalServerError)
            .with_body(simple_http::SimpleBody::Text(format!("{:?}", err)))
            .build()
            .expect("should generate request")
    }
}

#[cfg(test)]
mod test_server_tests {
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };

    use tracing_test::traced_test;

    use crate::{
        extensions::result_ext::BoxedResult,
        wire::simple_http::{FuncSimpleServer, SimpleBody, Status},
    };

    use super::{
        simple_http::{ServiceAction, SimpleHeader, SimpleMethod, SimpleOutgoingResponse},
        TestServer,
    };

    macro_rules! t {
        ($e:expr) => {
            match $e {
                Ok(t) => t,
                Err(e) => panic!("received error for `{}`: {}", stringify!($e), e),
            }
        };
    }

    #[test]
    #[traced_test]
    fn test_can_use_test_server() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_method(SimpleMethod::GET)
            .with_body(FuncSimpleServer::new(|req| {
                if let Some(SimpleBody::Text(body)) = req.body {
                    return SimpleOutgoingResponse::builder()
                        .with_body_string(format!(r#"{{"name": "alex", "body": {} }}"#, body))
                        .build()
                        .map_err(|err| err.into_boxed_error());
                }
                SimpleOutgoingResponse::builder()
                    .with_status(Status::BadRequest)
                    .build()
                    .map_err(|err| err.into_boxed_error())
            }))
            .build()
            .expect("should generate service action");

        let test_server = TestServer::new(8899, "127.0.0.1".into(), vec![resource]);
        let (handler, requests) = test_server.serve();

        let message = "\
POST /service/endpoint/v1 HTTP/1.1\r
Date: Sun, 10 Oct 2010 23:26:07 GMT\r
Server: Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g\r
Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
ETag: \"45b6-834-49130cc1182c0\"\r
Accept-Ranges: bytes\r
Content-Length: 12\r
Connection: close\r
Content-Type: text/plain\r
\r
Hello world!";

        let mut client = t!(TcpStream::connect("127.0.0.1:8899"));
        t!(client.write(message.as_bytes()));

        let mut response = String::new();
        t!(client.read_to_string(&mut response));

        assert_eq!(response, "HTTP/1.1 200 Ok");

        handler.join().expect("should work");
    }
}
