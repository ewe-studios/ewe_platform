use derive_more::From;
use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
};

use crate::simple_http::{
    self, IncomingRequestParts, ServiceAction, ServiceActionList, SimpleIncomingRequest,
    WrappedTcpStream,
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

        let request_reader = simple_http::HttpReader::simple_tcp_stream(BufReader::new(
            WrappedTcpStream::new(read_stream),
        ));
        for incoming_request_result in request_reader {
            // attempt to pull request_head

            if let Ok(IncomingRequestParts::Intro(method, url, proto)) = incoming_request_result {};
            // let (head, resources): (IncomingRequestParts, Option<Vec<ServiceAction>>) =
            //     match incoming_request_result.expect("should be a valid request") {
            //         IncomingRequestParts::Intro(method, url, proto) => (
            //             IncomingRequestParts::Intro(method.clone(), url.clone(), proto.clone()),
            //             action_list.get_matching2(&url, method.clone()),
            //         ),
            //         IncomingRequestParts::Headers(_) => break,
            //         IncomingRequestParts::Body(_) => break,
            //     };

            // if we get here then something is totally wrong, kill the contention
            break;
        }
    }
}
