//! `ResourcesHttpServer` tests moved out of `src/netcap/mod.rs` (spec 70
//! feature 59, rule 7). Every item the tests use is public.

use std::{
    io::{Read, Write},
    net::TcpStream,
};

use tracing_test::traced_test;

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::extensions::result_ext::BoxedResult;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    FuncSimpleServer, RequestDescriptor, SendSafeBody, SimpleIncomingRequest, Status,
};

use foundation_netio::shared::http::{
    ServiceAction, SimpleHeader, SimpleMethod, SimpleOutgoingResponse,
};

use foundation_testing::netcap::ResourcesHttpServer;

/// Answers `200` with the request body wrapped in a JSON-like envelope,
/// or `400` when the request carries no body. The server hands sized
/// bodies over as a stream, so the body is collected first.
fn echo_body_handler(req: SimpleIncomingRequest) -> Result<SimpleOutgoingResponse, BoxedError> {
    let body = match req.body {
        None | Some(SendSafeBody::None) => {
            return SimpleOutgoingResponse::builder()
                .with_status(Status::BadRequest)
                .add_header(SimpleHeader::CONTENT_LENGTH, "0")
                .build()
                .map_err(BoxedResult::into_boxed_error);
        }
        Some(body) => body,
    };
    let content =
        String::from_utf8(try_collect_bytes(body)?).map_err(BoxedResult::into_boxed_error)?;
    SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .with_body_bytes(format!(r#"{{"name": "alex", "body": {content} }}"#).into_bytes())
        .build()
        .map_err(BoxedResult::into_boxed_error)
}

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
fn test_can_use_test_server_has_matching_resource() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::POST)
        .with_body(FuncSimpleServer::new(echo_body_handler))
        .build()
        .expect("should generate service action");

    let test_server = ResourcesHttpServer::new(8889, "127.0.0.1".into(), vec![resource]);
    let (handler, requests, workers) = test_server.serve();

    let message = "\
POST /service/endpoint/v1 HTTP/1.1\r
Date: Sun, 10 Oct 2010 23:26:07 GMT\r
Server: Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g\r
Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
ETag: \"45b6-834-49130cc1182c0\"\r
Accept-Ranges: bytes\r
Content-Length: 12\r
Connection: close\r
Content-Type: application/json\r
\r
Hello world!";

    let mut client = t!(TcpStream::connect("127.0.0.1:8889"));
    t!(client.write(message.as_bytes()));

    let mut response = String::new();
    t!(client.read_to_string(&mut response));

    assert_eq!(response, "HTTP/1.1 200 Ok\r\ncontent-length: 39\r\n\r\n{\"name\": \"alex\", \"body\": Hello world! }");

    test_server.close().expect("should close server");

    match handler.join() {
        Ok(result) => match result {
            Ok(_) => {
                for worker_handler in workers.into_iter() {
                    worker_handler.join().expect("should have closed");
                }
                tracing::info!("Cleaned up workers");
            }
            Err(err) => {
                tracing::error!("Failed in serving requests: {:?}", err);
                panic!("Server failed");
            }
        },
        Err(err) => {
            tracing::error!("Failed in serving requests: {:?}", err);
            panic!("Server failed");
        }
    };

    let sent_requests: Vec<RequestDescriptor> = requests.iter().collect();
    assert_eq!(sent_requests.len(), 1);
}

#[test]
#[traced_test]
fn test_can_use_test_server_no_matching_resource() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::POST)
        .with_body(FuncSimpleServer::new(echo_body_handler))
        .build()
        .expect("should generate service action");

    let test_server = ResourcesHttpServer::new(9889, "127.0.0.1".into(), vec![resource]);
    let (handler, requests, workers) = test_server.serve();

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
Hello buster!";

    let mut client = t!(TcpStream::connect("127.0.0.1:9889"));
    t!(client.write(message.as_bytes()));

    let mut response = String::new();
    t!(client.read_to_string(&mut response));

    assert_eq!(
        response,
        "HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n"
    );
    test_server.close().expect("should close server");

    match handler.join() {
        Ok(result) => match result {
            Ok(_) => {
                for worker_handler in workers.into_iter() {
                    worker_handler.join().expect("should have closed");
                }
                tracing::info!("Cleaned up workers");
            }
            Err(err) => {
                tracing::error!("Failed in serving requests: {:?}", err);
                panic!("Server failed");
            }
        },
        Err(err) => {
            tracing::error!("Failed in serving requests: {:?}", err);
            panic!("Server failed");
        }
    };

    let sent_requests: Vec<RequestDescriptor> = requests.iter().collect();
    assert_eq!(sent_requests.len(), 0);
}
