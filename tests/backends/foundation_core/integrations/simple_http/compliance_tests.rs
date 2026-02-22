#![allow(unused)]

#[cfg(test)]
mod test_http_reader {

    use foundation_core::io::ioutils;
    use foundation_core::netcap::RawStream;
    use foundation_core::panic_if_failed;
    // Or comment out if not present in foundation_core
    use foundation_core::wire::simple_http::{
        http_streams, ChunkedData, HTTPStreams, HttpReaderError, HttpResponseReader,
        IncomingRequestParts, IncomingResponseParts, SendSafeBody, SimpleHeader, SimpleMethod,
        SimpleUrl, Status,
    };
    use regex::Regex; // add to Cargo.toml if missing

    use std::collections::BTreeMap;
    use std::io::Write;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    #[test]
    fn test_can_read_http_post_request() {
        let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:4888"));

        let message = "\
POST /users HTTP/1.1\r
Date: Sun, 10 Oct 2010 23:26:07 GMT\r
Server: Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g\r
Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
ETag: \"45b6-834-49130cc1182c0\"\r
Accept-Ranges: bytes\r
Content-Length: 12\r
Connection: close\r
Content-Type: text/html\r
\r
Hello world!";

        dbg!(&message);

        let req_thread = thread::spawn(move || {
            let mut client = panic_if_failed!(TcpStream::connect("localhost:4888"));
            panic_if_failed!(client.write(message.as_bytes()))
        });

        let (client_stream, _) = panic_if_failed!(listener.accept());
        let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        let request_reader = http_streams::no_send::request_reader(reader);

        let request_parts = request_reader
            .into_iter()
            .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
            .expect("should generate output");

        dbg!(&request_parts);

        let expected_parts: Vec<IncomingRequestParts> = vec![
            IncomingRequestParts::Intro(
                SimpleMethod::POST,
                SimpleUrl {
                    url: "/users".into(),
                    url_only: false,
                    matcher: Some(panic_if_failed!(Regex::new("/users"))),
                    params: None,
                    queries: None,
                },
                "HTTP/1.1".into(),
            ),
            IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                (SimpleHeader::ACCEPT_RANGES, vec!["bytes".into()]),
                (SimpleHeader::CONNECTION, vec!["close".into()]),
                (SimpleHeader::CONTENT_LENGTH, vec!["12".into()]),
                (SimpleHeader::CONTENT_TYPE, vec!["text/html".into()]),
                (
                    SimpleHeader::DATE,
                    vec!["Sun, 10 Oct 2010 23:26:07 GMT".into()],
                ),
                (
                    SimpleHeader::ETAG,
                    vec!["\"45b6-834-49130cc1182c0\"".into()],
                ),
                (
                    SimpleHeader::LAST_MODIFIED,
                    vec!["Sun, 26 Sep 2010 22:04:35 GMT".into()],
                ),
                (
                    SimpleHeader::SERVER,
                    vec!["Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g".into()],
                ),
            ])),
            IncomingRequestParts::SizedBody(SendSafeBody::Bytes(vec![
                72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33,
            ])),
        ];

        assert_eq!(request_parts, expected_parts);
        req_thread.join().expect("should be closed");
    }

    #[test]
    fn test_can_read_http_body_from_reqwest_http_message() {
        let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:5889"));

        let message = "POST /form HTTP/1.1\r\ncontent-type: application/x-www-form-urlencoded\r\ncontent-length: 24\r\naccept: */*\r\nhost: 127.0.0.1:7889\r\n\r\nhello=world&sean=monstar";

        let req_thread = thread::spawn(move || {
            let mut client = panic_if_failed!(TcpStream::connect("localhost:5889"));
            panic_if_failed!(client.write(message.as_bytes()))
        });

        let (client_stream, _) = panic_if_failed!(listener.accept());
        let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        let request_reader = http_streams::send::request_reader(reader);

        let request_parts = request_reader
            .into_iter()
            .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
            .expect("should generate output");

        dbg!(&request_parts);

        let expected_parts: Vec<IncomingRequestParts> = vec![
            IncomingRequestParts::Intro(
                SimpleMethod::POST,
                SimpleUrl {
                    url: "/form".into(),
                    url_only: false,
                    matcher: Some(panic_if_failed!(Regex::new("/form"))),
                    params: None,
                    queries: None,
                },
                "HTTP/1.1".into(),
            ),
            IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                (SimpleHeader::CONTENT_LENGTH, vec!["24".into()]),
                (
                    SimpleHeader::CONTENT_TYPE,
                    vec!["application/x-www-form-urlencoded".into()],
                ),
                (SimpleHeader::HOST, vec!["127.0.0.1:7889".into()]),
            ])),
            IncomingRequestParts::SizedBody(SendSafeBody::Bytes(vec![
                104, 101, 108, 108, 111, 61, 119, 111, 114, 108, 100, 38, 115, 101, 97, 110, 61,
                109, 111, 110, 115, 116, 97, 114,
            ])),
        ];

        assert_eq!(request_parts, expected_parts);
        req_thread.join().expect("should be closed");
    }

    #[test]
    fn test_can_read_http_body_from_reqwest_client() {
        let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:7887"));

        let req_thread = thread::spawn(move || {
            use reqwest;

            let form = &[("hello", "world"), ("sean", "monstar")];
            let _ = reqwest::blocking::Client::new()
                .post("http://127.0.0.1:7887/form")
                .form(form)
                .send();
        });

        let (client_stream, _) = panic_if_failed!(listener.accept());
        let reader = RawStream::from_tcp(client_stream).expect("check reader");
        let request_reader = http_streams::send::request_reader(reader);

        let request_parts = request_reader
            .into_iter()
            .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
            .expect("should generate output");

        dbg!(&request_parts);

        let expected_parts: Vec<IncomingRequestParts> = vec![
            IncomingRequestParts::Intro(
                SimpleMethod::POST,
                SimpleUrl {
                    url: "/form".into(),
                    url_only: false,
                    matcher: Some(panic_if_failed!(Regex::new("/form"))),
                    params: None,
                    queries: None,
                },
                "HTTP/1.1".into(),
            ),
            IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                (SimpleHeader::CONTENT_LENGTH, vec!["24".into()]),
                (
                    SimpleHeader::CONTENT_TYPE,
                    vec!["application/x-www-form-urlencoded".into()],
                ),
                (SimpleHeader::HOST, vec!["127.0.0.1:7887".into()]),
            ])),
            IncomingRequestParts::SizedBody(SendSafeBody::Bytes(vec![
                104, 101, 108, 108, 111, 61, 119, 111, 114, 108, 100, 38, 115, 101, 97, 110, 61,
                109, 111, 110, 115, 116, 97, 114,
            ])),
        ];

        assert_eq!(request_parts, expected_parts);
        req_thread.join().expect("should be closed");
    }
}

#[cfg(test)]
mod http_response_compliance {
    use super::*;
    use foundation_core::extensions::result_ext::BoxedError;
    use foundation_core::io::ioutils;
    use foundation_core::netcap::RawStream;
    // use foundation_core::panic_if_failed;
    // Or comment out if not present in foundation_core
    use foundation_core::wire::simple_http::{
        http_streams, ChunkedData, HTTPStreams, HttpReaderError, HttpResponseReader,
        IncomingRequestParts, IncomingResponseParts, SendSafeBody, SimpleHeader, SimpleMethod,
        SimpleUrl, Status,
    };
    use regex::Regex; // add to Cargo.toml if missing

    use std::collections::BTreeMap;
    use std::io::Write;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    mod transfer_encoding {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        // Test for "Parse chunks with lowercase size"
        #[test]
        #[traced_test]
        fn parse_chunks_with_lowercase_size() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\na\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            // println!("ChunkedContent: {:?}", contents);
            // [Data([48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None), DataEnded]
            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_chunks_with_uppercase_size() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\nA\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            // println!("ChunkedContent: {:?}", contents);
            // [Data([48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None), DataEnded]
            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_chunked() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n1e\nall your base are belong to us\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("all your base are belong to us".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn two_chunks_and_triple_zero_prefixed_end_chunk() {
            let message =
                "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n000\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers_with_multiple_newline_endings() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\n\nContent-Type: text/plain\n\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers_with_multiple_clrf() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\r\nContent-Type: text/plain\r\n\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\nContent-Type: text/plain\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_extensions_noquoting() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5;ilovew3;somuchlove=aretheseparametersfor;another=withvalue\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(
                        "hello".as_bytes().to_vec(),
                        Some(vec![
                            ("ilovew3".into(), None),
                            ("somuchlove".into(), Some("aretheseparametersfor".into())),
                            ("another".into(), Some("withvalue".into())),
                        ])
                    ),
                    ChunkedData::Data(
                        " world".as_bytes().to_vec(),
                        Some(vec![("blahblah".into(), None), ("blah".into(), None)])
                    ),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_semicolon_before_chunk_extensions() {
            let message = "HTTP/1.1 200 OK\nHost: localhost\nTransfer-encoding: chunked\n\n2 erfrferferf\naa\n0 rrrr\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::HOST, vec!["localhost".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_extension_after_semicolon() {
            let message =
                "HTTP/1.1 200 OK\nHost: localhost\nTransfer-encoding: chunked\n\n2;\naa\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::HOST, vec!["localhost".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_extensions_quoting() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5;ilovew3=\"I \\\"love\\\"; \\extensions\\\";somuchlove=\"aretheseparametersfor\";blah;foo=bar\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(
                        "hello".as_bytes().to_vec(),
                        Some(vec![
                            (
                                "ilovew3".into(),
                                Some("\"I \\\"love\\\"; \\extensions\\\"".into())
                            ),
                            (
                                "somuchlove".into(),
                                Some("\"aretheseparametersfor\"".into())
                            ),
                            ("blah".into(), None),
                            ("foo".into(), Some("bar".into())),
                        ])
                    ),
                    ChunkedData::Data(
                        " world".as_bytes().to_vec(),
                        Some(vec![("blahblah".into(), None), ("blah".into(), None)])
                    ),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn unbalanced_chunk_extensions_quoting() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n5;ilovew3=\"abc\";somuchlove=\"def; ghi\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // Requests cannot have invalid `Transfer-Encoding`. It is impossible to determine
        // their body size. Not erroring would make HTTP smuggling attacks possible.
        #[test]
        #[traced_test]
        fn ignoring_pigeons_we_do_not_allow_request_smuggling() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: pigeons\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_and_content_length() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: identity\nContent-Length: 5\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_and_content_length_lenient() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: identity\nContent-Length: 1\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_empty_transfer_encoding_and_content_length_lenient() {
            let message = "HTTP/1.1 200 OK\nHost: foo\nContent-Length: 10\nTransfer-Encoding:\nTransfer-Encoding:\nTransfer-Encoding:\n\n2\nAA\n0\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_before_other_transfer_coding_names() {
            let message =
                "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: chunked, deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_and_duplicate_transfer_encoding() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: chunked\nTransfer-Encoding: deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_before_other_transfer_coding_lenient() {
            let message =
                "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: chunked, deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_and_duplicate_transfer_encoding_lenient() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: chunked\nTransfer-Encoding: deflate\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_as_last_transfer_encoding() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: deflate, chunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                    (
                        SimpleHeader::TRANSFER_ENCODING,
                        vec!["deflate".into(), "chunked".into()],
                    ),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_as_last_transfer_encoding_multiple_headers() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: deflate\nTransfer-Encoding: chunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                    (
                        SimpleHeader::TRANSFER_ENCODING,
                        vec!["deflate".into(), "chunked".into()],
                    ),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunkedchunked_as_transfer_encoding() {
            let message = "HTTP/1.1 200 OK\nAccept: */*\nTransfer-Encoding: chunkedchunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn missing_last_chunk() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n3\nfoo\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn validate_chunk_parameters() {
            let message =
                "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\n3 \\n  \\r\\n\\nfoo\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_obs_fold_after_chunked_value() {
            let message = "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n  abc\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_header_not_terminated_by_crlf() {
            let message = "HTTP/1.1 200 OK\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n5\\r\\r;ABCD\n34\nE\n0\n\nGET / HTTP/1.1 \nHost: a\nContent-Length: 5\n\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_header_not_terminated_by_crlf_lenient() {
            let message = "HTTP/1.1 200 OK\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n6\\r\\r;ABCD\n33\nE\n0\n\nGET / HTTP/1.1 \nHost: a\nContent-Length: 5\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_data_not_terminated_by_crlf() {
            let message = "HTTP/1.1 200 OK\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n5\nWorld0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn space_after_chunk_header() {
            let message =
                "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\na \\r\\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("0123456789".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn space_after_chunk_header_lenient() {
            let message =
                "HTTP/1.1 200 OK\nTransfer-Encoding: chunked\n\na \\r\\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("0123456789".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod text_event_stream {
        use tracing_test::traced_test;

        use foundation_core::{
            panic_if_failed,
            wire::simple_http::{ChunkStateError, LineFeed},
        };

        use super::*;

        #[test]
        #[traced_test]
        fn parse_stream_with_multiple_lines_with_newlines_ending_with_double_newlines() {
            let message =
                "HTTP/1.1 200 OK\nContent-Type: text/event-stream\n\nevent: 0123456789\n\nevent2: 0123456789\n\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![
                    LineFeed::Line("event: 0123456789".into()),
                    LineFeed::Line("event2: 0123456789".into()),
                    LineFeed::SKIP,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_multiple_lines_with_crlf() {
            let message =
                "HTTP/1.1 200 OK\nContent-Type: text/event-stream\n\nevent: 0123456789\r\nevent2: 0123456789\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![
                    LineFeed::Line("event: 0123456789".into()),
                    LineFeed::Line("event2: 0123456789".into()),
                    LineFeed::SKIP,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_crlf() {
            let message =
                "HTTP/1.1 200 OK\nContent-Type: text/event-stream\n\nevent: 0123456789\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![LineFeed::Line("event: 0123456789".into()), LineFeed::SKIP,],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_double_line_endings() {
            let message =
                "HTTP/1.1 200 OK\nContent-Type: text/event-stream\n\nevent: 0123456789\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::response_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingResponseParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![LineFeed::Line("event: 0123456789".into()), LineFeed::SKIP,],
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod sample_responses {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn simple_response() {
            let message = "HTTP/1.1 200 OK\r\nHeader1: Value1\r\nHeader2:\t Value2\r\nContent-Length: 0\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn rtsp_response() {
            let message = "RTSP/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn ice_response() {
            let message = "ICE/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn error_on_invalid_response_start() {
            let message = "HTTPER/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn empty_body_should_not_trigger_spurious_span_callbacks() {
            let message = "HTTP/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn google_301() {
            let message = "HTTP/1.1 301 Moved Permanently\r\nLocation: http://www.google.com/\r\nContent-Type: text/html; charset=UTF-8\r\nDate: Sun, 26 Apr 2009 11:11:49 GMT\r\nExpires: Tue, 26 May 2009 11:11:49 GMT\r\nX-$PrototypeBI-Version: 1.6.0.3\r\nCache-Control: public, max-age=2592000\r\nServer: gws\r\nContent-Length:  210\r\n\r\n<HTML><HEAD><meta http-equiv=content-type content=text/html;charset=utf-8>\n<TITLE>301 Moved</TITLE></HEAD><BODY>\n<H1>301 Moved</H1>\nThe document has moved\n<A HREF=\"http://www.google.com/\">here</A>.\r\n</BODY></HTML>";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn google_301_with_bad_content_length() {
            let message = "HTTP/1.1 301 Moved Permanently\r\nLocation: http://www.google.com/\r\nContent-Type: text/html; charset=UTF-8\r\nDate: Sun, 26 Apr 2009 11:11:49 GMT\r\nExpires: Tue, 26 May 2009 11:11:49 GMT\r\nX-$PrototypeBI-Version: 1.6.0.3\r\nCache-Control: public, max-age=2592000\r\nServer: gws\r\nContent-Length:  219\r\n\r\n<HTML><HEAD><meta http-equiv=content-type content=text/html;charset=utf-8>\n<TITLE>301 Moved</TITLE></HEAD><BODY>\n<H1>301 Moved</H1>\nThe document has moved\n<A HREF=\"http://www.google.com/\">here</A>.\r\n</BODY></HTML>";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn amazon_com() {
            let message = "HTTP/1.1 301 MovedPermanently\r\nDate: Wed, 15 May 2013 17:06:33 GMT\r\nServer: Server\r\nx-amz-id-1: 0GPHKXSJQ826RK7GZEB2\r\np3p: policyref=\"http://www.amazon.com/w3c/p3p.xml\",CP=\"CAO DSP LAW CUR ADM IVAo IVDo CONo OTPo OUR DELi PUBi OTRi BUS PHY ONL UNI PUR FIN COM NAV INT DEM CNT STA HEA PRE LOC GOV OTC \"\r\nx-amz-id-2: STN69VZxIFSz9YJLbz1GDbxpbjG6Qjmmq5E3DxRhOUw+Et0p4hr7c/Q8qNcx4oAD\r\nLocation: http://www.amazon.com/Dan-Brown/e/B000AP9DSU/ref=s9_pop_gw_al1?_encoding=UTF8&refinementId=618073011&pf_rd_m=ATVPDKIKX0DER&pf_rd_s=center-2&pf_rd_r=0SHYY5BZXN3KR20BNFAY&pf_rd_t=101&pf_rd_p=1263340922&pf_rd_i=507846\r\nVary: Accept-Encoding,User-Agent\r\nContent-Type: text/html; charset=ISO-8859-1\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n\n\r\n0\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_headers_and_no_body() {
            let message = "HTTP/1.1 404 Not Found\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_reason_phrase() {
            let message = "HTTP/1.1 301\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn empty_reason_phrase_after_space() {
            let message = "HTTP/1.1 200 \r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_carriage_ret() {
            let message = "HTTP/1.1 200 OK\nContent-Type: text/html; charset=utf-8\nConnection: close\n\nthese headers are from http://news.ycombinator.com/";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_carriage_ret_lenient() {
            let message = "HTTP/1.1 200 OK\nContent-Type: text/html; charset=utf-8\nConnection: close\n\nthese headers are from http://news.ycombinator.com/";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn underscore_in_header_key() {
            let message = "HTTP/1.1 200 OK\r\nServer: DCLK-AdSvr\r\nContent-Type: text/xml\r\nContent-Length: 0\r\nDCLK_imp: v7;x;114750856;0-0;0;17820020;0/0;21603567/21621457/1;;~okv=;dcmt=text/xml;;~cs=o\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn bonjourmadame_fr() {
            let message = "HTTP/1.0 301 Moved Permanently\r\nDate: Thu, 03 Jun 2010 09:56:32 GMT\r\nServer: Apache/2.2.3 (Red Hat)\r\nCache-Control: public\r\nPragma: \r\nLocation: http://www.bonjourmadame.fr/\r\nVary: Accept-Encoding\r\nContent-Length: 0\r\nContent-Type: text/html; charset=UTF-8\r\nConnection: keep-alive\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_in_header_value() {
            let message = "HTTP/1.1 200 OK\r\nDate: Tue, 28 Sep 2010 01:14:13 GMT\r\nServer: Apache\r\nCache-Control: no-cache, must-revalidate\r\nExpires: Mon, 26 Jul 1997 05:00:00 GMT\r\n.et-Cookie: PlaxoCS=1274804622353690521; path=/; domain=.plaxo.com\r\nVary: Accept-Encoding\r\n_eep-Alive: timeout=45\r\n_onnection: Keep-Alive\r\nTransfer-Encoding: chunked\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n0\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_in_header_name() {
            let message = "HTTP/1.1 200 OK\r\nServer: Microsoft-IIS/6.0\r\nX-Powered-By: ASP.NET\r\nen-US Content-Type: text/xml\r\nContent-Type: text/xml\r\nContent-Length: 16\r\nDate: Fri, 23 Jul 2010 18:45:38 GMT\r\nConnection: keep-alive\r\n\r\n<xml>hello</xml>";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn non_ascii_in_status_line() {
            let message = "HTTP/1.1 500 Oriëntatieprobleem\r\nDate: Fri, 5 Nov 2010 23:07:12 GMT+2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn http_version_0_9() {
            let message = "HTTP/0.9 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_content_length_no_transfer_encoding() {
            let message = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello world";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn response_starting_with_crlf() {
            let message = "\r\nHTTP/1.1 200 OK\r\nHeader1: Value1\r\nHeader2:\t Value2\r\nContent-Length: 0\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod pipelining {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn should_parse_multiple_events() {
            let message = "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nAAA\r\nHTTP/1.1 201 Created\r\nContent-Length: 4\r\n\r\nBBBB\r\nHTTP/1.1 202 Accepted\r\nContent-Length: 5\r\n\r\nCCCC";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod finish {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn it_should_be_safe_to_finish_with_cb_after_empty_response() {
            let message = "HTTP/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod content_length_header {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn response_without_content_length_but_with_body() {
            let message = "HTTP/1.1 200 OK\r\nDate: Tue, 04 Aug 2009 07:59:32 GMT\r\nServer: Apache\r\nX-Powered-By: Servlet/2.5 JSP/2.1\r\nContent-Type: text/xml; charset=utf-8\r\nConnection: close\r\n\r\n<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\">\n  <SOAP-ENV:Body>\n    <SOAP-ENV:Fault>\n       <faultcode>SOAP-ENV:Client</faultcode>\n       <faultstring>Client Error</faultstring>\n    </SOAP-ENV:Fault>\n  </SOAP-ENV:Body>\n</SOAP-ENV:Envelope>";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn content_length_x() {
            let message = "HTTP/1.1 200 OK\r\nContent-Length-X: 0\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nOK\r\n0\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn content_length_reset_when_no_body_is_received() {
            let message = "HTTP/1.1 200 OK\r\nContent-Length: 123\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 456\r\n\r\n";
            // Test implementation can be added here

            dbg!(&message);
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod invalid_responses {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn incomplete_http_protocol() {
            let message = "HTP/1.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn extra_digit_in_http_major_version() {
            let message = "HTTP/01.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn extra_digit_in_http_major_version_2() {
            let message = "HTTP/11.1 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn extra_digit_in_http_minor_version() {
            let message = "HTTP/1.01 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn tab_after_http_version() {
            let message = "HTTP/1.1\t200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn cr_before_response_and_tab_after_http_version() {
            let message = "\rHTTP/1.1\t200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_cr() {
            let message = "HTTP/1.1 200 OK\r\nFoo: 1\rBar: 2\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_http_version() {
            let message = "HTTP/5.6 200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_space_after_start_line() {
            let message = "HTTP/1.1 200 OK\r\n Host: foo\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&message);
            dbg!(&request_one);

            assert!(matches!(&request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn extra_space_between_http_version_and_status_code() {
            let message = "HTTP/1.1  200 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn extra_space_between_status_code_and_reason() {
            let message = "HTTP/1.1 200  OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn one_digit_status_code() {
            let message = "HTTP/1.1 2 OK\r\n\r\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn only_lfs_present_and_no_body() {
            let message = "HTTP/1.1 200 OK\nContent-Length: 0\n\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn only_lfs_present_and_no_body_lenient() {
            let message = "HTTP/1.1 200 OK\nContent-Length: 0\n\n";
            // Test implementation can be added here

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn only_lfs_present() {
            let message = "HTTP/1.1 200 OK\nFoo: abc\nBar: def\n\nBODY\n";
            // Test implementation can be added here
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            let response_parts = request_one.unwrap();

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::Custom("Foo".into()), vec!["abc".into()]),
                    (SimpleHeader::Custom("Bar".into()), vec!["def".into()]),
                ])),
                IncomingResponseParts::SizedBody(SendSafeBody::Bytes(vec![66, 79, 68, 89, 10])),
            ];

            assert_eq!(response_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn only_lfs_present_lenient() {
            let message = "HTTP/1.1 200 OK\nFoo: abc\nBar: def\n\nBODY\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_response()
                .into_iter()
                .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>();

            dbg!(&request_one);

            assert!(matches!(&request_one, Ok(_)));

            let response_parts = request_one.unwrap();

            let expected_parts: Vec<IncomingResponseParts> = vec![
                IncomingResponseParts::Intro(Status::OK, "HTTP/1.1".into(), Some("OK".into())),
                IncomingResponseParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::Custom("Foo".into()), vec!["abc".into()]),
                    (SimpleHeader::Custom("Bar".into()), vec!["def".into()]),
                ])),
                IncomingResponseParts::SizedBody(SendSafeBody::Bytes(vec![66, 79, 68, 89, 10])),
            ];

            assert_eq!(response_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }
    }
}

#[cfg(test)]
mod http_requests_compliance {
    use super::*;
    use foundation_core::extensions::result_ext::BoxedError;
    use foundation_core::io::ioutils;
    use foundation_core::netcap::RawStream;
    // use foundation_core::panic_if_failed;
    // Or comment out if not present in foundation_core
    use foundation_core::wire::simple_http::{
        http_streams, ChunkedData, HTTPStreams, HttpReaderError, HttpResponseReader,
        IncomingRequestParts, IncomingResponseParts, SendSafeBody, SimpleHeader, SimpleMethod,
        SimpleUrl, Status,
    };
    use regex::Regex; // add to Cargo.toml if missing

    use std::collections::BTreeMap;
    use std::io::Write;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    mod hello_request {

        use foundation_core::panic_if_failed;

        use super::*;

        #[test]
        fn plain_text_content_with_headers() {
            let message = "\
POST /users HTTP/1.1\r
Date: Sun, 10 Oct 2010 23:26:07 GMT\r
Server: Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g\r
Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
ETag: \"45b6-834-49130cc1182c0\"\r
Accept-Ranges: bytes\r
Content-Length: 12\r
Connection: close\r
Content-Type: text/html\r
\r
Hello world!";

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");
            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/users".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/users"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::ACCEPT_RANGES, vec!["bytes".into()]),
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::CONTENT_LENGTH, vec!["12".into()]),
                    (SimpleHeader::CONTENT_TYPE, vec!["text/html".into()]),
                    (
                        SimpleHeader::DATE,
                        vec!["Sun, 10 Oct 2010 23:26:07 GMT".into()],
                    ),
                    (
                        SimpleHeader::ETAG,
                        vec!["\"45b6-834-49130cc1182c0\"".into()],
                    ),
                    (
                        SimpleHeader::LAST_MODIFIED,
                        vec!["Sun, 26 Sep 2010 22:04:35 GMT".into()],
                    ),
                    (
                        SimpleHeader::SERVER,
                        vec!["Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g".into()],
                    ),
                ])),
                IncomingRequestParts::SizedBody(SendSafeBody::Bytes(vec![
                    72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33,
                ])),
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }
    }

    mod uri {
        use foundation_core::panic_if_failed;

        use super::*;

        // Test function for "Quotes in URI"
        #[test]
        fn test_quotes_in_uri() {
            let message = "GET /with_\"lovely\"_quotes?foo=\\\"bar\\\" HTTP/1.1\n\n\n";

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/with_\"lovely\"_quotes?foo=\\\"bar\\\"".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/with_\"lovely\"_quotes"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "foo".into(),
                            "\\\"bar\\\"".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "Query URL with question mark"
        #[test]
        fn test_query_url_with_question_mark() {
            let message = "GET /test.cgi?foo=bar?baz HTTP/1.1\n\n\n";

            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/test.cgi?foo=bar?baz".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/test.cgi?foo=bar?baz"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "foo".into(),
                            "bar".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "Host terminated by a query string"
        #[test]
        fn test_host_terminated_by_query_string() {
            let message = "GET http://hypnotoad.org?hail=all HTTP/1.1\r\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "http://hypnotoad.org?hail=all".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("http://hypnotoad.org"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "hail".into(),
                            "all".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "`host:port` terminated by a query string"
        #[test]
        fn test_host_port_terminated_by_query_string() {
            let message = "GET http://hypnotoad.org:1234?hail=all HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "http://hypnotoad.org:1234?hail=all".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("http://hypnotoad.org:1234"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "hail".into(),
                            "all".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "Query URL with vertical bar character"
        #[test]
        fn test_query_url_with_vertical_bar_character() {
            let message = "GET /test.cgi?query=| HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/test.cgi?query=|".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/test.cgi?query=|"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "query".into(),
                            "1".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "`host:port` terminated by a space"
        #[test]
        fn test_host_port_terminated_by_space() {
            let message = "GET http://hypnotoad.org:1234 HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "http://hypnotoad.org:1234".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("http://hypnotoad.org:1234"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "Disallow UTF-8 in URI path in strict mode"
        #[test]
        fn test_allow_utf8_in_uri_path() {
            let message = "GET /δ¶/δt/pope?q=1#narf HTTP/1.1\nHost: github.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/δ¶/δt/pope?q=1#narf".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/δ¶/δt/pope"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "q".into(),
                            "1#narf".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::HOST,
                    vec!["github.com".into()],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }

        // Test function for "Fragment in URI"
        #[test]
        fn test_fragment_in_uri() {
            let message = "GET /forums/1/topics/2375?page=1#posts-17408 HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/forums/1/topics/2375?page=1#posts-17408".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/forums/1/topics/2375"))),
                        params: None,
                        queries: Some(BTreeMap::<String, String>::from([(
                            "page".into(),
                            "1#posts-17408".into(),
                        )])),
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::new()),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }
    }

    mod text_event_stream {
        use tracing_test::traced_test;

        use foundation_core::{
            panic_if_failed,
            wire::simple_http::{ChunkStateError, LineFeed},
        };

        use super::*;

        #[test]
        #[traced_test]
        fn parse_stream_with_multiple_lines_with_newlines_ending_with_double_newlines() {
            let message =
                "PUT /url HTTP/1.1\nContent-Type: text/event-stream\n\nevent: 0123456789\n\nevent2: 0123456789\n\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![
                    LineFeed::Line("event: 0123456789".into()),
                    LineFeed::Line("event2: 0123456789".into()),
                    LineFeed::SKIP,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_multiple_lines_with_crlf() {
            let message =
                "PUT /url HTTP/1.1\nContent-Type: text/event-stream\n\nevent: 0123456789\r\nevent2: 0123456789\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![
                    LineFeed::Line("event: 0123456789".into()),
                    LineFeed::Line("event2: 0123456789".into()),
                    LineFeed::SKIP,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_crlf() {
            let message =
                "PUT /url HTTP/1.1\nContent-Type: text/event-stream\n\nevent: 0123456789\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![LineFeed::Line("event: 0123456789".into()), LineFeed::SKIP,],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_stream_with_double_line_endings() {
            let message =
                "PUT /url HTTP/1.1\nContent-Type: text/event-stream\n\nevent: 0123456789\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::CONTENT_TYPE,
                    vec!["text/event-stream".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut feed_stream = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &feed_stream,
                IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::LineFeedStream(Some(body_iter))) =
                feed_stream
            else {
                panic!("Not a LineFeedStream")
            };

            let content_result: Result<Vec<LineFeed>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all feeds");

            println!("LineFeeds: {:?}", contents);
            assert_eq!(
                contents,
                vec![LineFeed::Line("event: 0123456789".into()), LineFeed::SKIP,],
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod transfer_encoding {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        // Test for "Parse chunks with lowercase size"
        #[test]
        #[traced_test]
        fn parse_chunks_with_lowercase_size() {
            let message = "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\na\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            // println!("ChunkedContent: {:?}", contents);
            // [Data([48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None), DataEnded]
            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn parse_chunks_with_uppercase_size() {
            let message = "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\nA\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            // println!("ChunkedContent: {:?}", contents);
            // [Data([48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None), DataEnded]
            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57], None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_chunked() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n1e\nall your base are belong to us\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("all your base are belong to us".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn two_chunks_and_triple_zero_prefixed_end_chunk() {
            let message =
                "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n000\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers_with_multiple_newline_endings() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\n\nContent-Type: text/plain\n\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers_with_multiple_clrf() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\r\nContent-Type: text/plain\r\n\r\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn trailing_headers() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5\nhello\n6\n world\n0\nVary: *\nContent-Type: text/plain\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("hello".as_bytes().to_vec(), None),
                    ChunkedData::Data(" world".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                    ChunkedData::Trailers(vec![
                        ("Vary".into(), Some("*".into())),
                        ("Content-Type".into(), Some("text/plain".into())),
                    ])
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_extensions_noquoting() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5;ilovew3;somuchlove=aretheseparametersfor;another=withvalue\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(
                        "hello".as_bytes().to_vec(),
                        Some(vec![
                            ("ilovew3".into(), None),
                            ("somuchlove".into(), Some("aretheseparametersfor".into())),
                            ("another".into(), Some("withvalue".into())),
                        ])
                    ),
                    ChunkedData::Data(
                        " world".as_bytes().to_vec(),
                        Some(vec![("blahblah".into(), None), ("blah".into(), None)])
                    ),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_semicolon_before_chunk_extensions() {
            let message = "POST /url HTTP/1.1\nHost: localhost\nTransfer-encoding: chunked\n\n2 erfrferferf\naa\n0 rrrr\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::HOST, vec!["localhost".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_extension_after_semicolon() {
            let message = "POST /url HTTP/1.1\nHost: localhost\nTransfer-encoding: chunked\n\n2;\naa\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::HOST, vec!["localhost".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_extensions_quoting() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5;ilovew3=\"I \\\"love\\\"; \\extensions\\\";somuchlove=\"aretheseparametersfor\";blah;foo=bar\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data(
                        "hello".as_bytes().to_vec(),
                        Some(vec![
                            (
                                "ilovew3".into(),
                                Some("\"I \\\"love\\\"; \\extensions\\\"".into())
                            ),
                            (
                                "somuchlove".into(),
                                Some("\"aretheseparametersfor\"".into())
                            ),
                            ("blah".into(), None),
                            ("foo".into(), Some("bar".into())),
                        ])
                    ),
                    ChunkedData::Data(
                        " world".as_bytes().to_vec(),
                        Some(vec![("blahblah".into(), None), ("blah".into(), None)])
                    ),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn unbalanced_chunk_extensions_quoting() {
            let message = "POST /url HTTP/1.1\nTransfer-Encoding: chunked\n\n5;ilovew3=\"abc\";somuchlove=\"def; ghi\nhello\n6;blahblah;blah\n world\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // Requests cannot have invalid `Transfer-Encoding`. It is impossible to determine
        // their body size. Not erroring would make HTTP smuggling attacks possible.
        #[test]
        #[traced_test]
        fn ignoring_pigeons_we_do_not_allow_request_smuggling() {
            let message = "PUT /url HTTP/1.1\nTransfer-Encoding: pigeons\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_and_content_length() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: identity\nContent-Length: 5\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_transfer_encoding_and_content_length_lenient() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: identity\nContent-Length: 1\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_empty_transfer_encoding_and_content_length_lenient() {
            let message = "POST / HTTP/1.1\nHost: foo\nContent-Length: 10\nTransfer-Encoding:\nTransfer-Encoding:\nTransfer-Encoding:\n\n2\nAA\n0\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_before_other_transfer_coding_names() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: chunked, deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_and_duplicate_transfer_encoding() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: chunked\nTransfer-Encoding: deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_before_other_transfer_coding_lenient() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: chunked, deflate\n\nWorld\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_and_duplicate_transfer_encoding_lenient() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: chunked\nTransfer-Encoding: deflate\n\nWorld\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_as_last_transfer_encoding() {
            let message = "POST /url HTTP/1.1\nAccept: */*\nTransfer-Encoding: deflate, chunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                    (
                        SimpleHeader::TRANSFER_ENCODING,
                        vec!["deflate".into(), "chunked".into()],
                    ),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunked_as_last_transfer_encoding_multiple_headers() {
            let message = "POST /url HTTP/1.1\nAccept: */*\nTransfer-Encoding: deflate\nTransfer-Encoding: chunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::POST,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                    (
                        SimpleHeader::TRANSFER_ENCODING,
                        vec!["deflate".into(), "chunked".into()],
                    ),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn post_with_chunkedchunked_as_transfer_encoding() {
            let message = "POST /post_identity_body_world?q=search#hey HTTP/1.1\nAccept: */*\nTransfer-Encoding: chunkedchunked\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn missing_last_chunk() {
            let message = "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\n3\nfoo\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn validate_chunk_parameters() {
            let message =
                "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\n3 \\n  \\r\\n\\nfoo\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_obs_fold_after_chunked_value() {
            let message =
                "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n  abc\n\n5\nWorld\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts_result = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts_result);

            assert!(matches!(request_parts_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_header_not_terminated_by_crlf() {
            let message = "GET /url HTTP/1.1\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n5\\r\\r;ABCD\n34\nE\n0\n\nGET / HTTP/1.1 \nHost: a\nContent-Length: 5\n\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_header_not_terminated_by_crlf_lenient() {
            let message = "GET /url HTTP/1.1\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n6\\r\\r;ABCD\n33\nE\n0\n\nGET / HTTP/1.1 \nHost: a\nContent-Length: 5\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            assert!(matches!(content_result, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn chunk_data_not_terminated_by_crlf() {
            let message = "GET /url HTTP/1.1\nHost: a\nConnection: close \nTransfer-Encoding: chunked \n\n5\nWorld0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                    (SimpleHeader::HOST, vec!["a".into()]),
                    (SimpleHeader::TRANSFER_ENCODING, vec!["chunked".into()]),
                ])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("World".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn space_after_chunk_header() {
            let message =
                "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\na \\r\\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("0123456789".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn space_after_chunk_header_lenient() {
            let message =
                "PUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\na \\r\\n0123456789\n0\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::PUT,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::TRANSFER_ENCODING,
                    vec!["chunked".into()],
                )])),
            ];

            assert_eq!(&request_parts[0..2], expected_parts);

            let mut chunked_body = request_parts.pop().expect("retrieved body");
            assert!(matches!(
                &chunked_body,
                IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SendSafeBody::ChunkedStream(Some(body_iter))) =
                chunked_body
            else {
                panic!("Not a ChunkedStream")
            };

            let content_result: Result<Vec<ChunkedData>, BoxedError> = body_iter.collect();
            let contents = content_result.expect("extracted all chunks");

            assert_eq!(
                contents,
                vec![
                    ChunkedData::Data("0123456789".as_bytes().to_vec(), None),
                    ChunkedData::DataEnded,
                ],
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod sample_requests {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn simple_request() {
            let message = "OPTIONS /url HTTP/1.1\nHeader1: Value1\nHeader2:\t Value2\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::OPTIONS,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::Custom(String::from("Header1")),
                        vec!["Value1".into()],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Header2")),
                        vec!["Value2".into()],
                    ),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn request_with_method_starting_with_h() {
            let message = "HEAD /url HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::HEAD,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn curl_get() {
            let message = "GET /url HTTP/1.1\nUser-Agent: curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1\nHost: 0.0.0.0=5000\nAccept: */*\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::ACCEPT,
                        vec!["*/*".into()],
                    ),
                    (
                        SimpleHeader::HOST,
                        vec!["0.0.0.0=5000".into()],
                    ),
                    (
                        SimpleHeader::USER_AGENT,
                        vec!["curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1".into()],
                    ),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn firefox_get() {
            let message = "GET /favicon.ico HTTP/1.1\nHost: 0.0.0.0=5000\nUser-Agent: Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0\nAccept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\nAccept-Language: en-us,en;q=0.5\nAccept-Encoding: gzip,deflate\nAccept-Charset: ISO-8859-1,utf-8;q=0.7,*;q=0.7\nKeep-Alive: 300\nConnection: keep-alive\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/favicon.ico".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/favicon.ico"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::ACCEPT,
                        vec![
                            "text/html".into(),
                            "application/xhtml+xml".into(),
                            "application/xml;q=0.9".into(),
                            "*/*;q=0.8".into(),
                        ],
                    ),
                    (
                        SimpleHeader::ACCEPT_LANGUAGE,
                        vec!["en-us".into(), "en;q=0.5".into()],
                    ),
                    (
                        SimpleHeader::ACCEPT_ENCODING,
                        vec!["gzip".into(), "deflate".into()],
                    ),
                    (
                        SimpleHeader::ACCEPT_CHARSET,
                        vec!["ISO-8859-1".into(), "utf-8;q=0.7".into(), "*;q=0.7".into()],
                    ),
                    (
                        SimpleHeader::KEEP_ALIVE,
                        vec!["300".into()],
                    ),
                    (
                        SimpleHeader::CONNECTION,
                        vec!["keep-alive".into()],
                    ),
                    (
                        SimpleHeader::HOST,
                        vec!["0.0.0.0=5000".into()],
                    ),
                    (
                        SimpleHeader::USER_AGENT,
                        vec!["Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0".into()],
                    ),

                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn dumbpack() {
            let message = "GET /dumbpack HTTP/1.1\naaaaaaaaaaaaa:++++++++++\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/dumbpack".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::Custom(String::from("aaaaaaaaaaaaa")),
                    vec!["++++++++++".into()],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_headers_and_no_body() {
            let message = "GET /get_no_headers_no_body/world HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/get_no_headers_no_body/world".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new(
                            "/get_no_headers_no_body/world"
                        ))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn one_header_and_no_body() {
            let message = "GET /get_one_header_no_body HTTP/1.1\nAccept: */*\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/get_one_header_no_body".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/get_one_header_no_body"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::ACCEPT,
                    vec!["*/*".into()],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn apache_bench_get() {
            let message = "GET /url HTTP/1.0\nHost: 0.0.0.0:5000\nUser-Agent: ApacheBench/2.3\nAccept: */*\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.0".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (SimpleHeader::HOST, vec!["0.0.0.0:5000".into()]),
                    (SimpleHeader::ACCEPT, vec!["*/*".into()]),
                    (SimpleHeader::USER_AGENT, vec!["ApacheBench/2.3".into()]),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn prefix_newline() {
            let message = "\r\nGET /url HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::SKIP,
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_http_version() {
            let message = "GET /\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn line_folding_in_header_value_with_crlf() {
            let message = "GET /url HTTP/1.1\nLine1:   abc\n\tdef\n ghi\n\t\tjkl\n  mno \n\t \tqrs\nLine2: \t line2\t\nLine3:\n line3\nLine4: \n \nConnection:\n close\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::Custom(String::from("Line1")),
                        vec![
                            "abc".into(),
                            "def".into(),
                            "ghi".into(),
                            "jkl".into(),
                            "mno".into(),
                            "qrs".into(),
                        ],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Line2")),
                        vec!["line2".into()],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Line3")),
                        vec!["line3".into()],
                    ),
                    (SimpleHeader::Custom(String::from("Line4")), vec![]),
                    (SimpleHeader::CONNECTION, vec!["close".into()]),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn line_folding_in_header_value_with_lf() {
            let message = "GET /url HTTP/1.1\nLine1:   abc\\n\\\n\tdef\\n\\\n ghi\\n\\\n\t\tjkl\\n\\\n  mno \\n\\\n\t \tqrs\\n\\\nLine2: \t line2\t\\n\\\nLine3:\\n\\\n line3\\n\\\nLine4: \\n\\\n \\n\\\nConnection:\\n\\\n close\\n\\\n\\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::Custom(String::from("Line1")),
                        vec![
                            "abc\\n\\".into(),
                            "def\\n\\".into(),
                            "ghi\\n\\".into(),
                            "jkl\\n\\".into(),
                            "mno \\n\\".into(),
                            "qrs\\n\\".into(),
                        ],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Line2")),
                        vec!["line2\t\\n\\".into()],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Line3")),
                        vec!["\\n\\".into(), "line3\\n\\".into()],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Line4")),
                        vec!["\\n\\".into(), "\\n\\".into()],
                    ),
                    (
                        SimpleHeader::CONNECTION,
                        vec!["\\n\\".into(), "close\\n\\".into(), "\\n".into()],
                    ),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_lf_after_cr() {
            let message = "GET /url HTTP/1.1\rLine: 1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts);

            assert!(matches!(request_parts, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn no_lf_after_cr_lenient() {
            let message = "GET / HTTP/1.1\rLine: 1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            dbg!(&request_parts);

            assert!(matches!(request_parts, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn request_starting_with_crlf() {
            let message = "\r\nGET /url HTTP/1.1\nHeader1: Value1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::SKIP,
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::Custom(String::from("Header1")),
                    vec!["Value1".into()],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }
    }

    mod extended_characters {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn extended_characters() {
            let message = "GET / HTTP/1.1\nTest: Düsseldorf\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::Custom(String::from("Test")),
                    vec!["Düsseldorf".into()],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }
    }

    mod ascii_255_in_header_value {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn ascii_255_in_header_value() {
            let message =
                b"OPTIONS /url HTTP/1.1\r\nHeader1: Value1\r\nHeader2: \xFFValue2\r\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::OPTIONS,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                    (
                        SimpleHeader::Custom(String::from("Header1")),
                        vec!["Value1".into()],
                    ),
                    (
                        SimpleHeader::Custom(String::from("Header2")),
                        vec!["ÿValue2".into()],
                    ),
                ])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }
    }

    mod x_ssl_nonsense {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn x_ssl_nonsense() {
            let message = "GET /url HTTP/1.1\nX-SSL-Nonsense:   -----BEGIN CERTIFICATE-----\n\tMIIFbTCCBFWgAwIBAgICH4cwDQYJKoZIhvcNAQEFBQAwcDELMAkGA1UEBhMCVUsx\n\tETAPBgNVBAoTCGVTY2llbmNlMRIwEAYDVQQLEwlBdXRob3JpdHkxCzAJBgNVBAMT\n\tAkNBMS0wKwYJKoZIhvcNAQkBFh5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMu\n\tdWswHhcNMDYwNzI3MTQxMzI4WhcNMDcwNzI3MTQxMzI4WjBbMQswCQYDVQQGEwJV\n\tSzERMA8GA1UEChMIZVNjaWVuY2UxEzARBgNVBAsTCk1hbmNoZXN0ZXIxCzAJBgNV\n\tBAcTmrsogriqMWLAk1DMRcwFQYDVQQDEw5taWNoYWVsIHBhcmQYJKoZIhvcNAQEB\n\tBQADggEPADCCAQoCggEBANPEQBgl1IaKdSS1TbhF3hEXSl72G9J+WC/1R64fAcEF\n\tW51rEyFYiIeZGx/BVzwXbeBoNUK41OK65sxGuflMo5gLflbwJtHBRIEKAfVVp3YR\n\tgW7cMA/s/XKgL1GEC7rQw8lIZT8RApukCGqOVHSi/F1SiFlPDxuDfmdiNzL31+sL\n\t0iwHDdNkGjy5pyBSB8Y79dsSJtCW/iaLB0/n8Sj7HgvvZJ7x0fr+RQjYOUUfrePP\n\tu2MSpFyf+9BbC/aXgaZuiCvSR+8Snv3xApQY+fULK/xY8h8Ua51iXoQ5jrgu2SqR\n\twgA7BUi3G8LFzMBl8FRCDYGUDy7M6QaHXx1ZWIPWNKsCAwEAAaOCAiQwggIgMAwG\n\tA1UdEwEB/wQCMAAwEQYJYIZIAYb4QgHTTPAQDAgWgMA4GA1UdDwEB/wQEAwID6DAs\n\tBglghkgBhvhCAQ0EHxYdVUsgZS1TY2llbmNlIFVzZXIgQ2VydGlmaWNhdGUwHQYD\n\tVR0OBBYEFDTt/sf9PeMaZDHkUIldrDYMNTBZMIGaBgNVHSMEgZIwgY+AFAI4qxGj\n\tloCLDdMVKwiljjDastqooXSkcjBwMQswCQYDVQQGEwJVSzERMA8GA1UEChMIZVNj\n\taWVuY2UxEjAQBgNVBAsTCUF1dGhvcml0eTELMAkGA1UEAxMCQ0ExLTArBgkqhkiG\n\t9w0BCQEWHmNhLW9wZXJhdG9yQGdyaWQtc3VwcG9ydC5hYy51a4IBADApBgNVHRIE\n\tIjAggR5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMudWswGQYDVR0gBBIwEDAO\n\tBgwrBgEEAdkvAQEBAQYwPQYJYIZIAYb4QgEEBDAWLmh0dHA6Ly9jYS5ncmlkLXN1\n\tcHBvcnQuYWMudmT4sopwqlBWsvcHViL2NybC9jYWNybC5jcmwwPQYJYIZIAYb4QgEDBDAWLmh0\n\tdHA6Ly9jYS5ncmlkLXN1cHBvcnQuYWMudWsvcHViL2NybC9jYWNybC5jcmwwPwYD\n\tVR0fBDgwNjA0oDKgMIYuaHR0cDovL2NhLmdyaWQt5hYy51ay9wdWIv\n\tY3JsL2NhY3JsLmNybDANBgkqhkiG9w0BAQUFAAOCAQEAS/U4iiooBENGW/Hwmmd3\n\tXCy6Zrt08YjKCzGNjorT98g8uGsqYjSxv/hmi0qlnlHs+k/3Iobc3LjS5AMYr5L8\n\tUO7OSkgFFlLHQyC9JzPfmLCAugvzEbyv4Olnsr8hbxF1MbKZoQxUZtMVu29wjfXk\n\thTeApBv7eaKCWpSp7MCbvgzm74izKhu3vlDk9w6qVrxePfGgpKPqfHiOoGhFnbTK\n\twTC6o2xq5y0qZ03JonF7OJspEd3I5zKY3E+ov7/ZhW6DqT8UFvsAdjvQbXyhV8Eu\n\tYhixw1aKEPzNjNowuIseVogKOLXxWI5vAi5HgXdS0/ES5gDGsABo4fqovUKlgop3\n\tRA==\n\t-----END CERTIFICATE-----\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_reader = http_streams::send::request_reader(reader);

            let mut request_parts = request_reader
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            dbg!(&request_parts);

            let expected_parts: Vec<IncomingRequestParts> = vec![
                IncomingRequestParts::Intro(
                    SimpleMethod::GET,
                    SimpleUrl {
                        url: "/url".into(),
                        url_only: false,
                        matcher: Some(panic_if_failed!(Regex::new("/url"))),
                        params: None,
                        queries: None,
                    },
                    "HTTP/1.1".into(),
                ),
                IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                    SimpleHeader::Custom(String::from("X-SSL-Nonsense")),
                    vec![
                            "-----BEGIN CERTIFICATE-----".into(),
                            "MIIFbTCCBFWgAwIBAgICH4cwDQYJKoZIhvcNAQEFBQAwcDELMAkGA1UEBhMCVUsx".into(),
                            "ETAPBgNVBAoTCGVTY2llbmNlMRIwEAYDVQQLEwlBdXRob3JpdHkxCzAJBgNVBAMT".into(),
                            "AkNBMS0wKwYJKoZIhvcNAQkBFh5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMu".into(),
                            "dWswHhcNMDYwNzI3MTQxMzI4WhcNMDcwNzI3MTQxMzI4WjBbMQswCQYDVQQGEwJV".into(),
                            "SzERMA8GA1UEChMIZVNjaWVuY2UxEzARBgNVBAsTCk1hbmNoZXN0ZXIxCzAJBgNV".into(),
                            "BAcTmrsogriqMWLAk1DMRcwFQYDVQQDEw5taWNoYWVsIHBhcmQYJKoZIhvcNAQEB".into(),
                            "BQADggEPADCCAQoCggEBANPEQBgl1IaKdSS1TbhF3hEXSl72G9J+WC/1R64fAcEF".into(),
                            "W51rEyFYiIeZGx/BVzwXbeBoNUK41OK65sxGuflMo5gLflbwJtHBRIEKAfVVp3YR".into(),
                            "gW7cMA/s/XKgL1GEC7rQw8lIZT8RApukCGqOVHSi/F1SiFlPDxuDfmdiNzL31+sL".into(),
                            "0iwHDdNkGjy5pyBSB8Y79dsSJtCW/iaLB0/n8Sj7HgvvZJ7x0fr+RQjYOUUfrePP".into(),
                            "u2MSpFyf+9BbC/aXgaZuiCvSR+8Snv3xApQY+fULK/xY8h8Ua51iXoQ5jrgu2SqR".into(),
                            "wgA7BUi3G8LFzMBl8FRCDYGUDy7M6QaHXx1ZWIPWNKsCAwEAAaOCAiQwggIgMAwG".into(),
                            "A1UdEwEB/wQCMAAwEQYJYIZIAYb4QgHTTPAQDAgWgMA4GA1UdDwEB/wQEAwID6DAs".into(),
                            "BglghkgBhvhCAQ0EHxYdVUsgZS1TY2llbmNlIFVzZXIgQ2VydGlmaWNhdGUwHQYD".into(),
                            "VR0OBBYEFDTt/sf9PeMaZDHkUIldrDYMNTBZMIGaBgNVHSMEgZIwgY+AFAI4qxGj".into(),
                            "loCLDdMVKwiljjDastqooXSkcjBwMQswCQYDVQQGEwJVSzERMA8GA1UEChMIZVNj".into(),
                            "aWVuY2UxEjAQBgNVBAsTCUF1dGhvcml0eTELMAkGA1UEAxMCQ0ExLTArBgkqhkiG".into(),
                            "9w0BCQEWHmNhLW9wZXJhdG9yQGdyaWQtc3VwcG9ydC5hYy51a4IBADApBgNVHRIE".into(),
                            "IjAggR5jYS1vcGVyYXRvckBncmlkLXN1cHBvcnQuYWMudWswGQYDVR0gBBIwEDAO".into(),
                            "BgwrBgEEAdkvAQEBAQYwPQYJYIZIAYb4QgEEBDAWLmh0dHA6Ly9jYS5ncmlkLXN1".into(),
                            "cHBvcnQuYWMudmT4sopwqlBWsvcHViL2NybC9jYWNybC5jcmwwPQYJYIZIAYb4QgEDBDAWLmh0".into(),
                            "dHA6Ly9jYS5ncmlkLXN1cHBvcnQuYWMudWsvcHViL2NybC9jYWNybC5jcmwwPwYD".into(),
                            "VR0fBDgwNjA0oDKgMIYuaHR0cDovL2NhLmdyaWQt5hYy51ay9wdWIv".into(),
                            "Y3JsL2NhY3JsLmNybDANBgkqhkiG9w0BAQUFAAOCAQEAS/U4iiooBENGW/Hwmmd3".into(),
                            "XCy6Zrt08YjKCzGNjorT98g8uGsqYjSxv/hmi0qlnlHs+k/3Iobc3LjS5AMYr5L8".into(),
                            "UO7OSkgFFlLHQyC9JzPfmLCAugvzEbyv4Olnsr8hbxF1MbKZoQxUZtMVu29wjfXk".into(),
                            "hTeApBv7eaKCWpSp7MCbvgzm74izKhu3vlDk9w6qVrxePfGgpKPqfHiOoGhFnbTK".into(),
                            "wTC6o2xq5y0qZ03JonF7OJspEd3I5zKY3E+ov7/ZhW6DqT8UFvsAdjvQbXyhV8Eu".into(),
                            "Yhixw1aKEPzNjNowuIseVogKOLXxWI5vAi5HgXdS0/ES5gDGsABo4fqovUKlgop3".into(),
                            "RA==".into(),
                            "-----END CERTIFICATE-----".into(),
                        ],
                )])),
                IncomingRequestParts::NoBody,
            ];

            assert_eq!(request_parts, expected_parts);

            req_thread.join().expect("should be closed");
        }
    }

    mod pipelining {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn should_parse_multiple_events() {
            let message = "POST /aaa HTTP/1.1\nContent-Length: 3\n\nAAA\nPUT /bbb HTTP/1.1\nContent-Length: 4\n\nBBBB\nPATCH /ccc HTTP/1.1\nContent-Length: 5\n\nCCCC\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::POST,
                        SimpleUrl {
                            url: "/aaa".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("/aaa"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.1".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                        SimpleHeader::CONTENT_LENGTH,
                        vec!["3".into()],
                    )])),
                    IncomingRequestParts::SizedBody(SendSafeBody::Bytes("AAA".as_bytes().to_vec())),
                ]
            );

            let request_two = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");
            assert_eq!(
                request_two,
                vec![
                    IncomingRequestParts::SKIP,
                    IncomingRequestParts::Intro(
                        SimpleMethod::PUT,
                        SimpleUrl {
                            url: "/bbb".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("/bbb"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.1".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                        SimpleHeader::CONTENT_LENGTH,
                        vec!["4".into()],
                    )])),
                    IncomingRequestParts::SizedBody(SendSafeBody::Bytes(
                        "BBBB".as_bytes().to_vec()
                    )),
                ]
            );

            let request_three = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");
            assert_eq!(
                request_three,
                vec![
                    IncomingRequestParts::SKIP,
                    IncomingRequestParts::Intro(
                        SimpleMethod::PATCH,
                        SimpleUrl {
                            url: "/ccc".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("/ccc"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.1".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([(
                        SimpleHeader::CONTENT_LENGTH,
                        vec!["5".into()],
                    )])),
                    IncomingRequestParts::SizedBody(SendSafeBody::Bytes(
                        "CCCC\n".as_bytes().to_vec()
                    )),
                ]
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod methods {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn report_request() {
            let message = "REPORT /test HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::Custom("REPORT".into()),
                        SimpleUrl {
                            url: "/test".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("/test"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.1".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([])),
                    IncomingRequestParts::NoBody,
                ]
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn connect_request() {
            let message = "CONNECT 0-home0.netscape.com:443 HTTP/1.0\nUser-agent: Mozilla/1.1N\nProxy-authorization: basic aGVsbG86d29ybGQ=\n\nsome data\nand yet even more data\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::CONNECT,
                        SimpleUrl {
                            url: "0-home0.netscape.com:443".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("0-home0.netscape.com:443"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.0".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                        (SimpleHeader::USER_AGENT, vec!["Mozilla/1.1N".into()],),
                        (
                            SimpleHeader::PROXY_AUTHORIZATION,
                            vec!["basic aGVsbG86d29ybGQ=".into()],
                        ),
                    ])),
                    IncomingRequestParts::NoBody,
                ]
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn connect_request_with_caps() {
            let message = "CONNECT HOME0.NETSCAPE.COM:443 HTTP/1.0\nUser-agent: Mozilla/1.1N\nProxy-authorization: basic aGVsbG86d29ybGQ=\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::CONNECT,
                        SimpleUrl {
                            url: "HOME0.NETSCAPE.COM:443".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("HOME0.NETSCAPE.COM:443"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.0".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                        (SimpleHeader::USER_AGENT, vec!["Mozilla/1.1N".into()],),
                        (
                            SimpleHeader::PROXY_AUTHORIZATION,
                            vec!["basic aGVsbG86d29ybGQ=".into()],
                        ),
                    ])),
                    IncomingRequestParts::NoBody,
                ]
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn connect_with_body() {
            let message = "CONNECT foo.bar.com:443 HTTP/1.0\nUser-agent: Mozilla/1.1N\nProxy-authorization: basic aGVsbG86d29ybGQ=\nContent-Length: 10\n\nblarfcicle\"\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::CONNECT,
                        SimpleUrl {
                            url: "foo.bar.com:443".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("foo.bar.com:443"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.0".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                        (SimpleHeader::USER_AGENT, vec!["Mozilla/1.1N".into()],),
                        (SimpleHeader::CONTENT_LENGTH, vec!["10".into()]),
                        (
                            SimpleHeader::PROXY_AUTHORIZATION,
                            vec!["basic aGVsbG86d29ybGQ=".into()],
                        ),
                    ])),
                    IncomingRequestParts::NoBody,
                ]
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn m_search_request() {
            let message = "M-SEARCH * HTTP/1.1\nHOST: 239.255.255.250:1900\nMAN: \"ssdp:discover\"\nST: \"ssdp:all\"\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
                .expect("should generate output");

            assert_eq!(
                request_one,
                vec![
                    IncomingRequestParts::Intro(
                        SimpleMethod::Custom("M-SEARCH".into()),
                        SimpleUrl {
                            url: "*".into(),
                            url_only: false,
                            matcher: Some(panic_if_failed!(Regex::new("\\*"))),
                            params: None,
                            queries: None,
                        },
                        "HTTP/1.1".into(),
                    ),
                    IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, Vec<String>>::from([
                        (SimpleHeader::HOST, vec!["239.255.255.250:1900".into()]),
                        (
                            SimpleHeader::Custom("MAN".into()),
                            vec!["\"ssdp:discover\"".into()]
                        ),
                        (
                            SimpleHeader::Custom("ST".into()),
                            vec!["\"ssdp:all\"".into()]
                        ),
                    ])),
                    IncomingRequestParts::NoBody,
                ]
            );

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn patch_request() {
            let message = "PATCH /file.txt HTTP/1.1\nHost: www.example.com\nContent-Type: application/example\nIf-Match: \"e0023aa4e\"\nContent-Length: 10\n\ncccccccccc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn purge_request() {
            let message = "PURGE /file.txt HTTP/1.1\nHost: www.example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn search_request() {
            let message = "SEARCH / HTTP/1.1\nHost: www.example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn link_request() {
            let message = "LINK /images/my_dog.jpg HTTP/1.1\nHost: example.com\nLink: <http://example.com/profiles/joe>; rel=\"tag\"\nLink: <http://example.com/profiles/sally>; rel=\"tag\"\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn unlink_request() {
            let message = "UNLINK /images/my_dog.jpg HTTP/1.1\nHost: example.com\nLink: <http://example.com/profiles/sally>; rel=\"tag\"\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        // We do not support custom protocols
        #[test]
        #[traced_test]
        fn source_request() {
            let message = "SOURCE /music/sweet/music HTTP/1.1\nHost: example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn source_request_with_ice() {
            let message = "SOURCE /music/sweet/music ICE/1.0\nHost: example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn options_request_with_rtsp() {
            let message = "OPTIONS /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn announce_request_with_rtsp() {
            let message = "ANNOUNCE /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn pri_request_http2() {
            let message = "PRI * HTTP/1.1\n\nSM\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn query_request() {
            let message = "QUERY /contacts HTTP/1.1\nHost: example.org\nContent-Type: example/query\nAccept: text/csv\nContent-Length: 41\n\nselect surname, givenname, email limit 10\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod lenient_http_version_parsing {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn invalid_http_version_lenient() {
            let message = "GET / HTTP/5.6\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod finish {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn safe_to_finish_after_get_request() {
            let message = "GET / HTTP/1.1\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn unsafe_to_finish_after_incomplete_put_request() {
            let message = "PUT / HTTP/1.1\nContent-Length: 100\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn unsafe_to_finish_inside_of_the_header() {
            let message = "PUT / HTTP/1.1\nContent-Leng\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod content_length_header {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        #[test]
        #[traced_test]
        fn content_length_with_zeroes() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 003\n\nabc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn content_length_with_follow_up_headers() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 003\nOhai: world\n\nabc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn error_on_content_length_overflow() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1000000000000000000000\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn error_on_duplicate_content_length() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nContent-Length: 2\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn error_on_simultaneous_content_length_and_transfer_encoding_identity() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nTransfer-Encoding: identity\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_whitespace_token_with_content_length_header_field() {
            let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length : 4\nUpgrade: ws\n\nabcdefgh\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_whitespace_token_with_content_length_header_field_lenient() {
            let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length : 4\nUpgrade: ws\n\nabcdefgh\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn error_on_simultaneous_content_length_and_transfer_encoding_identity_lenient() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nTransfer-Encoding: identity\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn funky_content_length_with_body() {
            let message =
                "GET /get_funky_content_length_body_hello HTTP/1.0\nconTENT-Length: 5\n\nHELLO\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_surrounding() {
            let message = "POST / HTTP/1.1\nContent-Length:  42 \n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_2() {
            let message = "POST / HTTP/1.1\nContent-Length: 4 2\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_3() {
            let message = "POST / HTTP/1.1\nContent-Length: 13 37\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn empty_content_length() {
            let message = "POST / HTTP/1.1\nContent-Length:\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Ok(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn content_length_with_cr_instead_of_dash() {
            let message = "PUT /url HTTP/1.1\nContent\rLength: 003\n\nabc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn content_length_reset_when_no_body_is_received() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 123\n\nPOST /url HTTP/1.1\nContent-Length: 456\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn missing_crlf_crlf_before_body() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 3\n\rabc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn missing_crlf_crlf_before_body_lenient() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 3\n\rabc\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Result: {:?}", request_one);
            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }
    }

    mod connection_header {
        use tracing_test::traced_test;

        use foundation_core::wire::simple_http::ChunkStateError;

        use super::*;

        mod keep_alive {
            use foundation_core::panic_if_failed;

            use super::*;

            #[test]
            #[traced_test]
            fn setting_flag() {
                let message = "PUT /url HTTP/1.1\nConnection: keep-alive\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn restarting_when_keep_alive_is_explicitly() {
                let message = "PUT /url HTTP/1.1\nConnection: keep-alive\n\nPUT /url HTTP/1.1\nConnection: keep-alive\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                let request_two = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_two);
                assert!(matches!(request_two, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn no_restart_when_keep_alive_is_off_1_0() {
                let message = "PUT /url HTTP/1.0\n\nPUT /url HTTP/1.1\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn resetting_flags_when_keep_alive_is_off_1_0_lenient() {
                let message = "PUT /url HTTP/1.0\nContent-Length: 0\n\nPUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_implicit_keep_alive() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\n\nq=42\n\nGET / HTTP/1.1\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn not_treating_cr_as_dash() {
                let message = "PUT /url HTTP/1.1\nConnection: keep\ralive\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn error_with_cr_in_header_name() {
                let message = "PUT /url HTTP/1.1\nConne\rction: keep\ralive\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Err(_)));

                req_thread.join().expect("should be closed");
            }
        }

        mod close {
            use foundation_core::panic_if_failed;

            use super::*;

            #[test]
            #[traced_test]
            fn setting_flag_on_close() {
                let message = "PUT /url HTTP/1.1\nConnection: close\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_explicit_close() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\nConnection: close\n\nq=42\n\nGET / HTTP/1.1\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_explicit_close_lenient() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\nConnection: close\n\nq=42\n\nGET / HTTP/1.1\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }
        }

        mod parsing_multiple_tokens {
            use foundation_core::panic_if_failed;

            use super::*;

            #[test]
            #[traced_test]
            fn sample() {
                let message =
                    "PUT /url HTTP/1.1\nConnection: close, token, upgrade, token, keep-alive\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding() {
                let message = "GET /demo HTTP/1.1\nHost: example.com\nConnection: Something,\n Upgrade, ,Keep-Alive\nSec-WebSocket-Key2: 12998 5 Y3 1  .P00\nSec-WebSocket-Protocol: sample\nUpgrade: WebSocket\nSec-WebSocket-Key1: 4 @1  46546xW%0l 1 5\nOrigin: http://example.com\n\nHot diggity dogg\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding_and_lws() {
                let message = "GET /demo HTTP/1.1\nConnection: keep-alive, upgrade\nUpgrade: WebSocket\n\nHot diggity dogg\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding_lws_and_crlf() {
                let message = "GET /demo HTTP/1.1\nConnection: keep-alive, \r\n upgrade\nUpgrade: WebSocket\n\nHot diggity dogg\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn invalid_whitespace_token_with_connection_header_field() {
                let message = "PUT /url HTTP/1.1\nConnection : upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Err(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn invalid_whitespace_token_with_connection_header_field_lenient() {
                let message = "PUT /url HTTP/1.1\nConnection : upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Err(_)));

                req_thread.join().expect("should be closed");
            }
        }

        mod upgrade {
            use foundation_core::panic_if_failed;

            use super::*;

            #[test]
            #[traced_test]
            fn setting_a_flag_and_pausing() {
                let message = "PUT /url HTTP/1.1\nConnection: upgrade\nUpgrade: ws\n\n\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn emitting_part_of_body_and_pausing() {
                let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn upgrade_get_request() {
                let message = "GET /demo HTTP/1.1\nHost: example.com\nConnection: Upgrade\nSec-WebSocket-Key2: 12998 5 Y3 1  .P00\nSec-WebSocket-Protocol: sample\nUpgrade: WebSocket\nSec-WebSocket-Key1: 4 @1  46546xW%0l 1 5\nOrigin: http://example.com\n\nHot diggity dogg\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }

            #[test]
            #[traced_test]
            fn upgrade_post_request() {
                let message = "POST /demo HTTP/1.1\nHost: example.com\nConnection: Upgrade\nUpgrade: HTTP/2.0\nContent-Length: 15\n\nsweet post body\nHot diggity dogg\n";

                // Test implementation would go here
                let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
                let addr = listener.local_addr().expect("should return address");

                let req_thread = thread::spawn(move || {
                    let mut client = panic_if_failed!(TcpStream::connect(addr));
                    panic_if_failed!(client.write(message.as_bytes()))
                });

                let (client_stream, _) = panic_if_failed!(listener.accept());
                let reader = RawStream::from_tcp(client_stream).expect("should create stream");
                let request_stream = http_streams::send::http_streams(reader);

                let request_one = request_stream
                    .next_request()
                    .into_iter()
                    .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

                tracing::debug!("Result: {:?}", request_one);
                assert!(matches!(request_one, Ok(_)));

                req_thread.join().expect("should be closed");
            }
        }
    }

    mod invalid_requests {
        use tracing_test::traced_test;

        use foundation_core::{panic_if_failed, wire::simple_http::ChunkStateError};

        use super::*;

        // Custom protocols that i do not plan to implement logic for.
        //
        //
        // #[test]
        // #[traced_test]
        // fn ice_protocol_and_get_method() {
        //     let message = "GET /music/sweet/music ICE/1.0\nHost: example.com\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }
        //
        // #[test]
        // #[traced_test]
        // fn ice_protocol_but_not_really() {
        //     let message = "GET /music/sweet/music IHTTP/1.0\nHost: example.com\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }
        //
        // #[test]
        // #[traced_test]
        // fn rtsp_protocol_and_put_method() {
        //     let message = "PUT /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        // #[test]
        // #[traced_test]
        // fn http_protocol_and_announce_method() {
        //     let message = "ANNOUNCE /music/sweet/music HTTP/1.0\nHost: example.com\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        // Current parser does not too much care about this but parses it regardless.
        //
        // #[test]
        // #[traced_test]
        // fn headers_separated_by_cr() {
        //     let message = "GET / HTTP/1.1\nFoo: 1\rBar: 2\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }
        //
        // #[test]
        // #[traced_test]
        // fn headers_separated_by_lf() {
        //     let message = "POST / HTTP/1.1\nHost: localhost:5000\nx:x\nTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        #[test]
        #[traced_test]
        fn headers_separated_by_dummy_characters() {
            let message =
                "GET / HTTP/1.1\nConnection: close\nHost: a\n\rZGET /evil: HTTP/1.1\nHost: a\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_dummy_characters_lenient() {
            let message =
                "GET / HTTP/1.1\nConnection: close\nHost: a\n\rZGET /evil: HTTP/1.1\nHost: a\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn empty_headers_separated_by_cr() {
            let message = "POST / HTTP/1.1\nConnection: Close\nHost: localhost:5000\nx:\rTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // #[test]
        // #[traced_test]
        // fn empty_headers_separated_by_lf() {
        //     let message = "POST / HTTP/1.1\nHost: localhost:5000\nx:\nTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        #[test]
        #[traced_test]
        fn invalid_header_token_1() {
            let message = "GET / HTTP/1.1\nFo@: Failure\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_header_token_2() {
            let message = r#"GET / HTTP/1.1\nFoo\01\test: Bar\n\n\n"#;
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn invalid_header_token_3() {
            let message = "GET / HTTP/1.1\n: Bar\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // We support custom methods so this does not apply
        //
        // #[test]
        // #[traced_test]
        // fn invalid_method() {
        //     let message = "MKCOLA / HTTP/1.1\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }
        //
        #[test]
        #[traced_test]
        fn illegal_header_field_name_line_folding() {
            let message = "GET / HTTP/1.1\nname\n : value\n\n\n";
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn corrupted_connection_header() {
            let message = r#"GET / HTTP/1.1\nHost: www.example.com\nConnection\r\033\065\325eep-Alive\nAccept-Encoding: gzip\n\n\n"#;
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn corrupted_header_name() {
            let message = r#"GET / HTTP/1.1\nHost: www.example.com\nX-Some-Header\r\033\065\325eep-Alive\nAccept-Encoding: gzip\n\n\n"#;
            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // #[test]
        // #[traced_test]
        // fn missing_cr_between_headers() {
        //     let message = "GET / HTTP/1.1\nHost: localhost\nDummy: x\nContent-Length: 23\n\nGET / HTTP/1.1\nDummy: GET /admin HTTP/1.1\nHost: localhost\n\n\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        // We support custom protocols so this does not apply
        //
        // #[test]
        // #[traced_test]
        // fn invalid_http_version() {
        //     let message = "GET / HTTP/5.6\n";
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        #[test]
        #[traced_test]
        fn invalid_space_after_start_line() {
            let message = "GET / HTTP/1.1\n Host: foo\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        // We are lenient by default with LF and CRLFs
        //
        // #[test]
        // #[traced_test]
        // fn only_lfs_present() {
        //     let message = "POST / HTTP/1.1\nTransfer-Encoding: chunked\nTrailer: Baz\nFoo: abc\nBar: def\n\n1\nA\n1;abc\nB\n1;def=ghi\nC\n1;jkl=\"mno\"\nD\n0\n\nBaz: ghi\n\n\n";
        //
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }
        //
        // #[test]
        // #[traced_test]
        // fn only_lfs_present_lenient() {
        //     let message = "POST / HTTP/1.1\nTransfer-Encoding: chunked\nTrailer: Baz\nFoo: abc\nBar: def\n\n1\nA\n1;abc\nB\n1;def=ghi\nC\n1;jkl=\"mno\"\nD\n0\n\nBaz: ghi\n\n\n";
        //
        //     // Test implementation would go here
        //     let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
        //     let addr = listener.local_addr().expect("should return address");
        //
        //     let req_thread = thread::spawn(move || {
        //         let mut client = panic_if_failed!(TcpStream::connect(addr));
        //         panic_if_failed!(client.write(message.as_bytes()))
        //     });
        //
        //     let (client_stream, _) = panic_if_failed!(listener.accept());
        //     let reader = RawStream::from_tcp(client_stream).expect("should create stream");
        //     let request_stream = http_streams::send::http_streams(reader);
        //
        //     let request_one = request_stream
        //         .next_request()
        //         .into_iter()
        //         .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();
        //
        //     tracing::debug!("Finished with {:?}", request_one);
        //
        //     assert!(matches!(request_one, Err(_)));
        //
        //     req_thread.join().expect("should be closed");
        // }

        #[test]
        #[traced_test]
        fn spaces_before_headers() {
            let message = "POST /hello HTTP/1.1\nHost: localhost\nFoo: bar\n Content-Length: 38\n\nGET /bye HTTP/1.1\nHost: localhost\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }

        #[test]
        #[traced_test]
        fn spaces_before_headers_lenient() {
            let message = "POST /hello HTTP/1.1\nHost: localhost\nFoo: bar\n Content-Length: 38\n\nGET /bye HTTP/1.1\nHost: localhost\n\n\n";

            // Test implementation would go here
            let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:0"));
            let addr = listener.local_addr().expect("should return address");

            let req_thread = thread::spawn(move || {
                let mut client = panic_if_failed!(TcpStream::connect(addr));
                panic_if_failed!(client.write(message.as_bytes()))
            });

            let (client_stream, _) = panic_if_failed!(listener.accept());
            let reader = RawStream::from_tcp(client_stream).expect("should create stream");
            let request_stream = http_streams::send::http_streams(reader);

            let request_one = request_stream
                .next_request()
                .into_iter()
                .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>();

            tracing::debug!("Finished with {:?}", request_one);

            assert!(matches!(request_one, Err(_)));

            req_thread.join().expect("should be closed");
        }
    }
}
