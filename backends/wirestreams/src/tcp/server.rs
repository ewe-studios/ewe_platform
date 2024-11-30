use derive_more::From;
use std::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
};

use crate::simple_http::{ServiceAction, SimpleIncomingRequest};

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

                Self::serve_connection(&stream, actions.clone(), tx.clone());
            }
        }));
    }

    fn serve_connection(
        stream: &TcpStream,
        actions: Vec<ServiceAction>,
        sender: mpsc::Sender<SimpleIncomingRequest>,
    ) {
        todo!()
    }
}
