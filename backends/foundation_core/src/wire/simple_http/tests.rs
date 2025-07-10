#[cfg(test)]
mod test_http_reader_requests_compliance {
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
    fn request_finish_should_safe_to_finish_with_incomplete_put_request() {
        let message = "\
GET / HTTP/1.1
Content-Length: 100

";

        // TODO
    }

    #[test]
    fn request_finish_should_unsafe_to_finish_with_incomplete_put_request() {
        let message = "\
PUT / HTTP/1.1
Content-Length: 100

";

        // TODO
    }

    #[test]
    fn request_connection_upgrade_post_request() {
        let message = "\
POST /demo HTTP/1.1
Host: example.com
Connection: Upgrade
Upgrade: HTTP/2.0
Content-Length: 15

sweet post body\
Hot diggity dogg
";

        // TODO
    }

    #[test]
    fn request_connection_get_request() {
        let message = "\
GET /demo HTTP/1.1
Host: example.com
Connection: Upgrade
Sec-WebSocket-Key2: 12998 5 Y3 1  .P00
Sec-WebSocket-Protocol: sample
Upgrade: WebSocket
Sec-WebSocket-Key1: 4 @1  46546xW%0l 1 5
Origin: http://example.com

Hot diggity dogg
";

        // TODO
    }

    #[test]
    fn request_connection_with_part_of_body_and_pausing() {
        let message = "\
PUT /url HTTP/1.1
Connection: upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
";

        // TODO
    }

    #[test]
    fn request_connection_with_setting_flag_and_pausing() {
        let message = "\
PUT /url HTTP/1.1
Connection: upgrade
Upgrade: ws


";

        // TODO
    }

    #[test]
    fn request_connection_with_invalid_whitespace_token_with_connection_header_field_lenient() {
        let message = "\
PUT /url HTTP/1.1
Connection : upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
";

        // TODO
    }

    #[test]
    fn request_connection_with_invalid_whitespace_token_with_connection_header_field() {
        let message = "\
PUT /url HTTP/1.1
Connection : upgrade
Content-Length: 4
Upgrade: ws

abcdefgh
";

        // TODO
    }

    #[test]
    fn request_connection_with_with_folding_and_lws_and_crlf() {
        let message = "\
GET /demo HTTP/1.1
Connection: keep-alive, \r\n upgrade
Upgrade: WebSocket

Hot diggity dogg
";

        // TODO
    }

    #[test]
    fn request_connection_with_with_folding_and_lws() {
        let message = "\
GET /demo HTTP/1.1
Connection: keep-alive, upgrade
Upgrade: WebSocket

Hot diggity dogg
";

        // TODO
    }

    #[test]
    fn request_connection_with_multiple_tokens_with_folding() {
        let message = "\
GET /demo HTTP/1.1
Host: example.com
Connection: Something,
 Upgrade, ,Keep-Alive
Sec-WebSocket-Key2: 12998 5 Y3 1  .P00
Sec-WebSocket-Protocol: sample
Upgrade: WebSocket
Sec-WebSocket-Key1: 4 @1  46546xW%0l 1 5
Origin: http://example.com

Hot diggity dogg
";

        // TODO
    }

    #[test]
    fn request_connection_with_multiple_tokens() {
        let message = "\
PUT /url HTTP/1.1
Connection: close, token, upgrade, token, keep-alive


";

        // TODO
    }

    #[test]
    fn connection_clrf_between_request_is_explicit_close() {
        let message = "\
POST / HTTP/1.1
Host: www.example.com
Content-Type: application/x-www-form-urlencoded
Content-Length: 4
Connection: close

q=42

GET / HTTP/1.1        ";

        // TODO
    }

    #[test]
    fn connection_no_carriage_return_in_request() {
        let message = "\
PUT /url HTTP/1.1
Connection: keep\ralive


        ";

        // TODO
    }

    #[test]
    fn connectioon_close_received() {
        let message = "\
PUT /url HTTP/1.1
Connection: close


        ";

        // TODO
    }

    #[test]
    fn implicit_keep_alive_for_crlf_between_requests() {
        let message = "\
POST / HTTP/1.1
Host: www.example.com
Content-Type: application/x-www-form-urlencoded
Content-Length: 4

q=42

GET / HTTP/1.1
        ";

        // TODO
    }

    #[test]
    fn connection_reset_without_keep_alive() {
        let message = "\
PUT /url HTTP/1.0
Content-Length: 0

PUT /url HTTP/1.1
Transfer-Encoding: chunked


        ";

        // TODO
    }

    #[test]
    fn connection_header_protocol_without_headers() {
        let message = "\
PUT /url HTTP/1.0

PUT /url HTTP/1.1


        ";

        // TODO
    }

    #[test]
    fn connection_header_keep_alive() {
        let message = "\
PUT /url HTTP/1.1\r
Connection: keep-alive\r
\r
PUT /url HTTP/1.1\r
Connection: keep-alive\r
        ";

        // TODO
    }

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

#[cfg(test)]
mod test_http_reader_response_compliance {
    // TODO
}
