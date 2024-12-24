#[cfg(test)]
mod test_http_reader_compliance {
    use regex::Regex;

    use crate::io::ioutils;
    use crate::panic_if_failed;
    use crate::wire::simple_http::{
        HttpReader, HttpReaderError, IncomingRequestParts, SimpleBody, SimpleHeader, SimpleMethod,
        SimpleUrl, WrappedTcpStream,
    };

    use std::collections::BTreeMap;
    use std::io::Write;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    #[test]
    fn plain_text_content_with_headers() {
        let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:7888"));

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

        let req_thread = thread::spawn(move || {
            let mut client = panic_if_failed!(TcpStream::connect("localhost:7888"));
            panic_if_failed!(client.write(message.as_bytes()))
        });

        let (client_stream, _) = panic_if_failed!(listener.accept());
        let reader = ioutils::BufferedReader::new(WrappedTcpStream::new(client_stream));
        let simple_tcp_stream = HttpReader::simple_tcp_stream(reader);
        let request_reader = simple_tcp_stream;

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
            IncomingRequestParts::Headers(BTreeMap::<SimpleHeader, String>::from([
                (SimpleHeader::ACCEPT_RANGES, "bytes".into()),
                (SimpleHeader::CONNECTION, "close".into()),
                (SimpleHeader::CONTENT_LENGTH, "12".into()),
                (SimpleHeader::CONTENT_TYPE, "text/html".into()),
                (SimpleHeader::DATE, "Sun, 10 Oct 2010 23:26:07 GMT".into()),
                (SimpleHeader::ETAG, "\"45b6-834-49130cc1182c0\"".into()),
                (
                    SimpleHeader::LAST_MODIFIED,
                    "Sun, 26 Sep 2010 22:04:35 GMT".into(),
                ),
                (
                    SimpleHeader::SERVER,
                    "Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g".into(),
                ),
            ])),
            IncomingRequestParts::Body(Some(SimpleBody::Bytes(vec![
                72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33,
            ]))),
        ];

        assert_eq!(request_parts, expected_parts);
        req_thread.join().expect("should be closed");
    }
}
