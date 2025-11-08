#![allow(unused)]

#[cfg(test)]
mod http_requests_compliance {
    use super::*;
    use crate::extensions::result_ext::BoxedError;
    use crate::io::ioutils;
    use crate::netcap::RawStream;
    use crate::panic_if_failed;
    use crate::wire::simple_http::{
        ChunkedData, HTTPStreams, HttpReader, HttpReaderError, IncomingRequestParts, SimpleBody,
        SimpleHeader, SimpleMethod, SimpleUrl,
    };
    use regex::Regex;

    use std::collections::BTreeMap;
    use std::io::Write;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    mod hello_request {

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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::SizedBody(SimpleBody::Bytes(vec![
                    72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33,
                ])),
            ];

            assert_eq!(request_parts, expected_parts);
            req_thread.join().expect("should be closed");
        }
    }

    mod uri {
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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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

    mod transfer_encoding {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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
            let request_reader = super::HttpReader::from_reader(reader);

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
                IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(_)))
            ));

            let IncomingRequestParts::StreamedBody(SimpleBody::ChunkedStream(Some(body_iter))) =
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

        use crate::wire::simple_http::ChunkStateError;

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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
            let request_reader = super::HttpReader::from_reader(reader);

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

        use crate::wire::simple_http::ChunkStateError;

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
            let request_reader = super::HttpReader::from_reader(reader);

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

        use crate::wire::simple_http::ChunkStateError;

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
            let request_reader = super::HttpReader::from_reader(reader);

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

        use crate::wire::simple_http::ChunkStateError;

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
            let request_reader = super::HttpReader::from_reader(reader);

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

        use crate::wire::simple_http::ChunkStateError;

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
            let request_stream = super::HTTPStreams::from_reader(reader);

            let request_one = request_stream
                .next_reader()
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
                    IncomingRequestParts::SizedBody(SimpleBody::Bytes("AAA".as_bytes().to_vec())),
                ]
            );

            let request_two = request_stream
                .next_reader()
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
                    IncomingRequestParts::SizedBody(SimpleBody::Bytes("BBBB".as_bytes().to_vec())),
                ]
            );

            let request_three = request_stream
                .next_reader()
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
                    IncomingRequestParts::SizedBody(SimpleBody::Bytes(
                        "CCCC\n".as_bytes().to_vec()
                    )),
                ]
            );

            req_thread.join().expect("should be closed");
        }
    }

    mod methods {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

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
            let request_stream = super::HTTPStreams::from_reader(reader);

            let request_one = request_stream
                .next_reader()
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
            let request_stream = super::HTTPStreams::from_reader(reader);

            let request_one = request_stream
                .next_reader()
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
            let request_stream = super::HTTPStreams::from_reader(reader);

            let request_one = request_stream
                .next_reader()
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
        }

        #[test]
        #[traced_test]
        fn m_search_request() {
            let message = "M-SEARCH * HTTP/1.1\nHOST: 239.255.255.250:1900\nMAN: \"ssdp:discover\"\nST: \"ssdp:all\"\n\n\n";
        }

        #[test]
        #[traced_test]
        fn patch_request() {
            let message = "PATCH /file.txt HTTP/1.1\nHost: www.example.com\nContent-Type: application/example\nIf-Match: \"e0023aa4e\"\nContent-Length: 10\n\ncccccccccc\n";
        }

        #[test]
        #[traced_test]
        fn purge_request() {
            let message = "PURGE /file.txt HTTP/1.1\nHost: www.example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn search_request() {
            let message = "SEARCH / HTTP/1.1\nHost: www.example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn link_request() {
            let message = "LINK /images/my_dog.jpg HTTP/1.1\nHost: example.com\nLink: <http://example.com/profiles/joe>; rel=\"tag\"\nLink: <http://example.com/profiles/sally>; rel=\"tag\"\n\n\n";
        }

        #[test]
        #[traced_test]
        fn unlink_request() {
            let message = "UNLINK /images/my_dog.jpg HTTP/1.1\nHost: example.com\nLink: <http://example.com/profiles/sally>; rel=\"tag\"\n\n\n";
        }

        #[test]
        #[traced_test]
        fn source_request() {
            let message = "SOURCE /music/sweet/music HTTP/1.1\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn source_request_with_ice() {
            let message = "SOURCE /music/sweet/music ICE/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn options_request_with_rtsp() {
            let message = "OPTIONS /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn announce_request_with_rtsp() {
            let message = "ANNOUNCE /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn pri_request_http2() {
            let message = "PRI * HTTP/1.1\n\nSM\n\n\n";
        }

        #[test]
        #[traced_test]
        fn query_request() {
            let message = "QUERY /contacts HTTP/1.1\nHost: example.org\nContent-Type: example/query\nAccept: text/csv\nContent-Length: 41\n\nselect surname, givenname, email limit 10\n";
        }
    }

    mod lenient_http_version_parsing {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

        use super::*;

        #[test]
        #[traced_test]
        fn invalid_http_version_lenient() {
            let message = "GET / HTTP/5.6\n\n\n";
        }
    }

    mod finish {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

        use super::*;

        #[test]
        #[traced_test]
        fn safe_to_finish_after_get_request() {
            let message = "GET / HTTP/1.1\n\n\n";
        }

        #[test]
        #[traced_test]
        fn unsafe_to_finish_after_incomplete_put_request() {
            let message = "PUT / HTTP/1.1\nContent-Length: 100\n\n";
        }

        #[test]
        #[traced_test]
        fn unsafe_to_finish_inside_of_the_header() {
            let message = "PUT / HTTP/1.1\nContent-Leng\n";
        }
    }

    mod content_length_header {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

        use super::*;

        #[test]
        #[traced_test]
        fn content_length_with_zeroes() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 003\n\nabc\n";
        }

        #[test]
        #[traced_test]
        fn content_length_with_follow_up_headers() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 003\nOhai: world\n\nabc\n";
        }

        #[test]
        #[traced_test]
        fn error_on_content_length_overflow() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1000000000000000000000\n\n";
        }

        #[test]
        #[traced_test]
        fn error_on_duplicate_content_length() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nContent-Length: 2\n\n";
        }

        #[test]
        #[traced_test]
        fn error_on_simultaneous_content_length_and_transfer_encoding_identity() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nTransfer-Encoding: identity\n\n";
        }

        #[test]
        #[traced_test]
        fn invalid_whitespace_token_with_content_length_header_field() {
            let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length : 4\nUpgrade: ws\n\nabcdefgh\n";
        }

        #[test]
        #[traced_test]
        fn invalid_whitespace_token_with_content_length_header_field_lenient() {
            let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length : 4\nUpgrade: ws\n\nabcdefgh\n";
        }

        #[test]
        #[traced_test]
        fn no_error_on_simultaneous_content_length_and_transfer_encoding_identity_lenient() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 1\nTransfer-Encoding: identity\n\n";
        }

        #[test]
        #[traced_test]
        fn funky_content_length_with_body() {
            let message =
                "GET /get_funky_content_length_body_hello HTTP/1.0\nconTENT-Length: 5\n\nHELLO\n";
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_surrounding() {
            let message = "POST / HTTP/1.1\nContent-Length:  42 \n\n";
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_2() {
            let message = "POST / HTTP/1.1\nContent-Length: 4 2\n\n";
        }

        #[test]
        #[traced_test]
        fn spaces_in_content_length_3() {
            let message = "POST / HTTP/1.1\nContent-Length: 13 37\n\n";
        }

        #[test]
        #[traced_test]
        fn empty_content_length() {
            let message = "POST / HTTP/1.1\nContent-Length:\n\n";
        }

        #[test]
        #[traced_test]
        fn content_length_with_cr_instead_of_dash() {
            let message = "PUT /url HTTP/1.1\nContent\rLength: 003\n\nabc\n";
        }

        #[test]
        #[traced_test]
        fn content_length_reset_when_no_body_is_received() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 123\n\nPOST /url HTTP/1.1\nContent-Length: 456\n\n";
        }

        #[test]
        #[traced_test]
        fn missing_crlf_crlf_before_body() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 3\n\rabc\n";
        }

        #[test]
        #[traced_test]
        fn missing_crlf_crlf_before_body_lenient() {
            let message = "PUT /url HTTP/1.1\nContent-Length: 3\n\rabc\n";
        }
    }

    mod connection_header {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

        use super::*;

        mod keep_alive {
            use super::*;

            #[test]
            #[traced_test]
            fn setting_flag() {
                let message = "PUT /url HTTP/1.1\nConnection: keep-alive\n\n\n";
            }

            #[test]
            #[traced_test]
            fn restarting_when_keep_alive_is_explicitly() {
                let message = "PUT /url HTTP/1.1\nConnection: keep-alive\n\nPUT /url HTTP/1.1\nConnection: keep-alive\n\n\n";
            }

            #[test]
            #[traced_test]
            fn no_restart_when_keep_alive_is_off_1_0() {
                let message = "PUT /url HTTP/1.0\n\nPUT /url HTTP/1.1\n\n\n";
            }

            #[test]
            #[traced_test]
            fn resetting_flags_when_keep_alive_is_off_1_0_lenient() {
                let message = "PUT /url HTTP/1.0\nContent-Length: 0\n\nPUT /url HTTP/1.1\nTransfer-Encoding: chunked\n\n";
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_implicit_keep_alive() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\n\nq=42\n\nGET / HTTP/1.1\n";
            }

            #[test]
            #[traced_test]
            fn not_treating_cr_as_dash() {
                let message = "PUT /url HTTP/1.1\nConnection: keep\ralive\n\n\n";
            }
        }

        mod close {
            use super::*;

            #[test]
            #[traced_test]
            fn setting_flag_on_close() {
                let message = "PUT /url HTTP/1.1\nConnection: close\n\n\n";
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_explicit_close() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\nConnection: close\n\nq=42\n\nGET / HTTP/1.1\n";
            }

            #[test]
            #[traced_test]
            fn crlf_between_requests_explicit_close_lenient() {
                let message = "POST / HTTP/1.1\nHost: www.example.com\nContent-Type: application/x-www-form-urlencoded\nContent-Length: 4\nConnection: close\n\nq=42\n\nGET / HTTP/1.1\n";
            }
        }

        mod parsing_multiple_tokens {
            use super::*;

            #[test]
            #[traced_test]
            fn sample() {
                let message =
                    "PUT /url HTTP/1.1\nConnection: close, token, upgrade, token, keep-alive\n\n\n";
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding() {
                let message = "GET /demo HTTP/1.1\nHost: example.com\nConnection: Something,\n Upgrade, ,Keep-Alive\nSec-WebSocket-Key2: 12998 5 Y3 1  .P00\nSec-WebSocket-Protocol: sample\nUpgrade: WebSocket\nSec-WebSocket-Key1: 4 @1  46546xW%0l 1 5\nOrigin: http://example.com\n\nHot diggity dogg\n";
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding_and_lws() {
                let message = "GET /demo HTTP/1.1\nConnection: keep-alive, upgrade\nUpgrade: WebSocket\n\nHot diggity dogg\n";
            }

            #[test]
            #[traced_test]
            fn multiple_tokens_with_folding_lws_and_crlf() {
                let message = "GET /demo HTTP/1.1\nConnection: keep-alive, \r\n upgrade\nUpgrade: WebSocket\n\nHot diggity dogg\n";
            }

            #[test]
            #[traced_test]
            fn invalid_whitespace_token_with_connection_header_field() {
                let message = "PUT /url HTTP/1.1\nConnection : upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";
            }

            #[test]
            #[traced_test]
            fn invalid_whitespace_token_with_connection_header_field_lenient() {
                let message = "PUT /url HTTP/1.1\nConnection : upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";
            }
        }

        mod upgrade {
            use super::*;

            #[test]
            #[traced_test]
            fn setting_a_flag_and_pausing() {
                let message = "PUT /url HTTP/1.1\nConnection: upgrade\nUpgrade: ws\n\n\n";
            }

            #[test]
            #[traced_test]
            fn emitting_part_of_body_and_pausing() {
                let message = "PUT /url HTTP/1.1\nConnection: upgrade\nContent-Length: 4\nUpgrade: ws\n\nabcdefgh\n";
            }

            #[test]
            #[traced_test]
            fn upgrade_get_request() {
                let message = "GET /demo HTTP/1.1\nHost: example.com\nConnection: Upgrade\nSec-WebSocket-Key2: 12998 5 Y3 1  .P00\nSec-WebSocket-Protocol: sample\nUpgrade: WebSocket\nSec-WebSocket-Key1: 4 @1  46546xW%0l 1 5\nOrigin: http://example.com\n\nHot diggity dogg\n";
            }

            #[test]
            #[traced_test]
            fn upgrade_post_request() {
                let message = "POST /demo HTTP/1.1\nHost: example.com\nConnection: Upgrade\nUpgrade: HTTP/2.0\nContent-Length: 15\n\nsweet post body\nHot diggity dogg\n";
            }
        }
    }

    mod invalid_requests {
        use tracing_test::traced_test;

        use crate::wire::simple_http::ChunkStateError;

        use super::*;

        #[test]
        #[traced_test]
        fn ice_protocol_and_get_method() {
            let message = "GET /music/sweet/music ICE/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn ice_protocol_but_not_really() {
            let message = "GET /music/sweet/music IHTTP/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn rtsp_protocol_and_put_method() {
            let message = "PUT /music/sweet/music RTSP/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn http_protocol_and_announce_method() {
            let message = "ANNOUNCE /music/sweet/music HTTP/1.0\nHost: example.com\n\n\n";
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_cr() {
            let message = "GET / HTTP/1.1\nFoo: 1\rBar: 2\n\n\n";
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_lf() {
            let message = "POST / HTTP/1.1\nHost: localhost:5000\nx:x\nTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_dummy_characters() {
            let message =
                "GET / HTTP/1.1\nConnection: close\nHost: a\n\rZGET /evil: HTTP/1.1\nHost: a\n\n\n";
        }

        #[test]
        #[traced_test]
        fn headers_separated_by_dummy_characters_lenient() {
            let message =
                "GET / HTTP/1.1\nConnection: close\nHost: a\n\rZGET /evil: HTTP/1.1\nHost: a\n\n\n";
        }

        #[test]
        #[traced_test]
        fn empty_headers_separated_by_cr() {
            let message = "POST / HTTP/1.1\nConnection: Close\nHost: localhost:5000\nx:\rTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
        }

        #[test]
        #[traced_test]
        fn empty_headers_separated_by_lf() {
            let message = "POST / HTTP/1.1\nHost: localhost:5000\nx:\nTransfer-Encoding: chunked\n\n1\nA\n0\n\n\n";
        }

        #[test]
        #[traced_test]
        fn invalid_header_token_1() {
            let message = "GET / HTTP/1.1\nFo@: Failure\n\n\n";
        }

        #[test]
        #[traced_test]
        fn invalid_header_token_2() {
            let message = r#"GET / HTTP/1.1\nFoo\01\test: Bar\n\n\n"#;
        }

        #[test]
        #[traced_test]
        fn invalid_header_token_3() {
            let message = "GET / HTTP/1.1\n: Bar\n\n\n";
        }

        #[test]
        #[traced_test]
        fn invalid_method() {
            let message = "MKCOLA / HTTP/1.1\n\n\n";
        }

        #[test]
        #[traced_test]
        fn illegal_header_field_name_line_folding() {
            let message = "GET / HTTP/1.1\nname\n : value\n\n\n";
        }

        #[test]
        #[traced_test]
        fn corrupted_connection_header() {
            let message = r#"GET / HTTP/1.1\nHost: www.example.com\nConnection\r\033\065\325eep-Alive\nAccept-Encoding: gzip\n\n\n"#;
        }

        #[test]
        #[traced_test]
        fn corrupted_header_name() {
            let message = r#"GET / HTTP/1.1\nHost: www.example.com\nX-Some-Header\r\033\065\325eep-Alive\nAccept-Encoding: gzip\n\n\n"#;
        }

        #[test]
        #[traced_test]
        fn missing_cr_between_headers() {
            let message = "GET / HTTP/1.1\nHost: localhost\nDummy: x\nContent-Length: 23\n\nGET / HTTP/1.1\nDummy: GET /admin HTTP/1.1\nHost: localhost\n\n\n";
        }

        #[test]
        #[traced_test]
        fn invalid_http_version() {
            let message = "GET / HTTP/5.6\n";
        }

        #[test]
        #[traced_test]
        fn invalid_space_after_start_line() {
            let message = "GET / HTTP/1.1\n Host: foo\n";
        }

        #[test]
        #[traced_test]
        fn only_lfs_present() {
            let message = "POST / HTTP/1.1\nTransfer-Encoding: chunked\nTrailer: Baz\nFoo: abc\nBar: def\n\n1\nA\n1;abc\nB\n1;def=ghi\nC\n1;jkl=\"mno\"\nD\n0\n\nBaz: ghi\n\n\n";
        }

        #[test]
        #[traced_test]
        fn only_lfs_present_lenient() {
            let message = "POST / HTTP/1.1\nTransfer-Encoding: chunked\nTrailer: Baz\nFoo: abc\nBar: def\n\n1\nA\n1;abc\nB\n1;def=ghi\nC\n1;jkl=\"mno\"\nD\n0\n\nBaz: ghi\n\n\n";
        }

        #[test]
        #[traced_test]
        fn spaces_before_headers() {
            let message = "POST /hello HTTP/1.1\nHost: localhost\nFoo: bar\n Content-Length: 38\n\nGET /bye HTTP/1.1\nHost: localhost\n\n\n";
        }

        #[test]
        #[traced_test]
        fn spaces_before_headers_lenient() {
            let message = "POST /hello HTTP/1.1\nHost: localhost\nFoo: bar\n Content-Length: 38\n\nGET /bye HTTP/1.1\nHost: localhost\n\n\n";
        }
    }
}
