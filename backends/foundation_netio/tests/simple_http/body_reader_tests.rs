//! Body reading utility tests (moved from `src/shared/client/body_reader.rs`).

use foundation_core::extensions::result_ext::{BoxedError, SendableBoxedError};
use foundation_core::io::readers::Data;
use foundation_core::valtron::Stream;
use foundation_netio::event_source::{Event, ParseResult};
use foundation_netio::shared::client::{
    collect_bytes_from_send_safe, collect_bytes_into, collect_bytes_strict,
    collect_from_chunked_stream, collect_from_linefeed_stream, collect_from_stream,
    collect_string_strict, collect_strings_from_send_safe, parse_json_str, parse_with_fallback,
    try_collect_bytes, try_collect_string, ProcessStreamResult, ResponseParser,
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator, write_from_chunked_stream,
    write_from_linefeed_stream, write_from_stream,
};
use foundation_netio::shared::http::{
    ChunkedData, ContentLengthEnforcingIterator, HttpReaderError, IncomingResponseParts, LineFeed,
    SendSafeBody,
};
use serde::Deserialize;

/// Helper to create a BoxedError for testing
fn make_error(msg: &str) -> BoxedError {
    msg.to_string().into()
}

/// Helper to create a SendableBoxedError for testing
fn make_sendable_error(msg: &str) -> SendableBoxedError {
    msg.to_string().into()
}

fn make_sse_stream(
    data: Vec<Result<ParseResult, SendableBoxedError>>,
) -> SendSafeBody {
    let send_iter: Box<
        dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync,
    > = Box::new(data.into_iter());
    SendSafeBody::SseStream(Some(send_iter))
}

#[test]
fn test_process_stream_result_debug() {
    // Basic smoke test for the Result type
    let result = ProcessStreamResult::Completed;
    assert_eq!(result, ProcessStreamResult::Completed);
}

#[test]
fn test_parse_json_str() {
    let json = r#"{"name": "test", "value": 42}"#;

    #[derive(Deserialize, Debug, PartialEq)]
    struct TestObj {
        name: String,
        value: i32,
    }

    let result: Option<TestObj> = parse_json_str(json, "test-source");
    assert!(result.is_some());
    let obj = result.unwrap();
    assert_eq!(obj.name, "test");
    assert_eq!(obj.value, 42);
}

#[test]
fn test_parse_json_str_invalid() {
    let invalid_json = r#"{"name": "test", invalid}"#;

    #[derive(Deserialize, Debug, PartialEq)]
    struct TestObj {
        name: String,
        value: i32,
    }

    let result: Option<TestObj> = parse_json_str(invalid_json, "test-source");
    assert!(result.is_none());
}

#[test]
fn test_parse_with_fallback_success() {
    let json = r#"{"items": [1, 2, 3]}"#;

    #[derive(Deserialize, Debug, PartialEq)]
    struct TestResponse {
        items: Vec<i32>,
    }

    let fallback_called = std::cell::RefCell::new(false);
    let result = parse_with_fallback(json, "test-source", || {
        *fallback_called.borrow_mut() = true;
        TestResponse { items: vec![] }
    });

    assert!(!*fallback_called.borrow());
    assert_eq!(result.items, vec![1, 2, 3]);
}

#[test]
fn test_parse_with_fallback_error() {
    let invalid_json = r#"{"items": invalid}"#;

    #[derive(Deserialize, Debug, PartialEq)]
    struct TestResponse {
        items: Vec<i32>,
    }

    let fallback_called = std::cell::RefCell::new(false);
    let result = parse_with_fallback(invalid_json, "test-source", || {
        *fallback_called.borrow_mut() = true;
        TestResponse { items: vec![0] }
    });

    assert!(*fallback_called.borrow());
    assert_eq!(result.items, vec![0]);
}

#[test]
fn test_response_parser_trait() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct ApiResponse {
        data: Vec<String>,
    }

    impl ResponseParser for ApiResponse {
        type Output = Vec<String>;

        fn parse(body: &str, _source: &str) -> Self::Output {
            match serde_json::from_str::<Self>(body) {
                Ok(response) => response.data,
                Err(_) => Vec::new(),
            }
        }
    }

    let valid_json = r#"{"data": ["a", "b", "c"]}"#;
    let result = ApiResponse::parse(valid_json, "test");
    assert_eq!(result, vec!["a", "b", "c"]);

    let invalid_json = r#"{"data": invalid}"#;
    let result = ApiResponse::parse(invalid_json, "test");
    assert_eq!(result, Vec::<String>::new());
}

#[test]
fn test_collect_from_stream_success() {
    let data = vec![
        Ok(Data::Bytes(b"hello".to_vec())),
        Ok(Data::Bytes(b" ".to_vec())),
        Ok(Data::Bytes(b"world".to_vec())),
    ];
    let result = collect_from_stream(data.into_iter());
    assert_eq!(result, b"hello world".to_vec());
}

#[test]
fn test_collect_from_stream_error() {
    let data: Vec<Result<Data, BoxedError>> = vec![
        Ok(Data::Bytes(b"hello".to_vec())),
        Err(make_error("stream error")),
    ];
    let result = collect_from_stream(data.into_iter());
    // Should collect what it got before error
    assert_eq!(result, b"hello".to_vec());
}

#[test]
fn test_collect_from_stream_empty() {
    let data: Vec<Result<Data, BoxedError>> = vec![];
    let result = collect_from_stream(data.into_iter());
    assert!(result.is_empty());
}

#[test]
fn test_collect_from_chunked_stream_success() {
    let data = vec![
        Ok(ChunkedData::Data(b"chunk1".to_vec(), None)),
        Ok(ChunkedData::Data(b"chunk2".to_vec(), None)),
        Ok(ChunkedData::DataEnded),
    ];
    let result = collect_from_chunked_stream(data.into_iter());
    assert_eq!(result, b"chunk1chunk2".to_vec());
}

#[test]
fn test_collect_from_chunked_stream_with_trailers() {
    let data = vec![
        Ok(ChunkedData::Data(b"data".to_vec(), None)),
        Ok(ChunkedData::Trailers(vec![])),
        Ok(ChunkedData::DataEnded),
    ];
    let result = collect_from_chunked_stream(data.into_iter());
    assert_eq!(result, b"data".to_vec());
}

#[test]
fn test_collect_from_chunked_stream_error() {
    let data: Vec<Result<ChunkedData, BoxedError>> = vec![
        Ok(ChunkedData::Data(b"hello".to_vec(), None)),
        Err(make_error("chunked error")),
    ];
    let result = collect_from_chunked_stream(data.into_iter());
    assert_eq!(result, b"hello".to_vec());
}

#[test]
fn test_collect_from_linefeed_stream_success() {
    let data = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Ok(LineFeed::Line("line2".to_string())),
        Ok(LineFeed::END),
    ];
    let result = collect_from_linefeed_stream(data.into_iter());
    // Each line gets \n appended
    assert_eq!(result, b"line1\nline2\n".to_vec());
}

#[test]
fn test_collect_from_linefeed_stream_with_skip() {
    let data = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Ok(LineFeed::SKIP),
        Ok(LineFeed::Line("line2".to_string())),
        Ok(LineFeed::END),
    ];
    let result = collect_from_linefeed_stream(data.into_iter());
    assert_eq!(result, b"line1\nline2\n".to_vec());
}

#[test]
fn test_collect_from_linefeed_stream_error() {
    let data: Vec<Result<LineFeed, BoxedError>> = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Err(make_error("line error")),
    ];
    let result = collect_from_linefeed_stream(data.into_iter());
    assert_eq!(result, b"line1\n".to_vec());
}

#[test]
fn test_write_from_stream_success() {
    let data = vec![
        Ok(Data::Bytes(b"hello".to_vec())),
        Ok(Data::Bytes(b" world".to_vec())),
    ];
    let mut output = Vec::new();
    let result = write_from_stream(data.into_iter(), &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 11);
    assert_eq!(output, b"hello world".to_vec());
}

#[test]
fn test_write_from_stream_error() {
    let data: Vec<Result<Data, BoxedError>> = vec![
        Ok(Data::Bytes(b"hello".to_vec())),
        Err(make_error("write error")),
    ];
    let mut output = Vec::new();
    let result = write_from_stream(data.into_iter(), &mut output);
    assert!(result.is_err());
    assert_eq!(output, b"hello".to_vec());
}

#[test]
fn test_write_from_chunked_stream_success() {
    let data = vec![
        Ok(ChunkedData::Data(b"chunk1".to_vec(), None)),
        Ok(ChunkedData::Data(b"chunk2".to_vec(), None)),
        Ok(ChunkedData::DataEnded),
    ];
    let mut output = Vec::new();
    let result = write_from_chunked_stream(data.into_iter(), &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 12);
    assert_eq!(output, b"chunk1chunk2".to_vec());
}

#[test]
fn test_write_from_linefeed_stream_success() {
    let data = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Ok(LineFeed::Line("line2".to_string())),
        Ok(LineFeed::END),
    ];
    let mut output = Vec::new();
    let result = write_from_linefeed_stream(data.into_iter(), &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 12);
    assert_eq!(output, b"line1\nline2\n".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_text() {
    let body = SendSafeBody::Text("hello world".to_string());
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"hello world".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_bytes() {
    let body = SendSafeBody::Bytes(b"binary data".to_vec());
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"binary data".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_none() {
    let body = SendSafeBody::None;
    let result = collect_bytes_from_send_safe(body);
    assert!(result.is_empty());
}

#[test]
fn test_collect_bytes_from_send_safe_stream() {
    // Use SendableBoxedError which has Send + Sync for the iterator
    let stream_data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"chunk1".to_vec())),
        Ok(Data::Bytes(b"chunk2".to_vec())),
    ];
    // Cast to BoxedError iterator via Box<dyn Iterator + Send>
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::Stream(Some(send_iter));
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"chunk1chunk2".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_stream_none() {
    let body = SendSafeBody::Stream(None);
    let result = collect_bytes_from_send_safe(body);
    assert!(result.is_empty());
}

#[test]
fn test_collect_bytes_from_send_safe_chunked_stream() {
    let stream_data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
        Ok(ChunkedData::Data(b"data1".to_vec(), None)),
        Ok(ChunkedData::Data(b"data2".to_vec(), None)),
        Ok(ChunkedData::DataEnded),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::ChunkedStream(Some(send_iter));
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"data1data2".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_chunked_stream_none() {
    let body = SendSafeBody::ChunkedStream(None);
    let result = collect_bytes_from_send_safe(body);
    assert!(result.is_empty());
}

#[test]
fn test_collect_bytes_from_send_safe_linefeed_stream() {
    let stream_data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Ok(LineFeed::Line("line2".to_string())),
        Ok(LineFeed::END),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::LineFeedStream(Some(send_iter));
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"line1\nline2\n".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_linefeed_stream_none() {
    let body = SendSafeBody::LineFeedStream(None);
    let result = collect_bytes_from_send_safe(body);
    assert!(result.is_empty());
}

#[test]
fn test_collect_bytes_into_text() {
    let body = SendSafeBody::Text("hello".to_string());
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);
    assert_eq!(output, b"hello".to_vec());
}

#[test]
fn test_collect_bytes_into_bytes() {
    let body = SendSafeBody::Bytes(b"binary".to_vec());
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 6);
    assert_eq!(output, b"binary".to_vec());
}

#[test]
fn test_collect_bytes_into_none() {
    let body = SendSafeBody::None;
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    assert!(output.is_empty());
}

#[test]
fn test_collect_bytes_into_stream() {
    let stream_data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"chunk1".to_vec())),
        Ok(Data::Bytes(b"chunk2".to_vec())),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::Stream(Some(send_iter));
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 12);
    assert_eq!(output, b"chunk1chunk2".to_vec());
}

#[test]
fn test_collect_bytes_into_chunked_stream() {
    let stream_data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
        Ok(ChunkedData::Data(b"data1".to_vec(), None)),
        Ok(ChunkedData::Data(b"data2".to_vec(), None)),
        Ok(ChunkedData::DataEnded),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::ChunkedStream(Some(send_iter));
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 10);
    assert_eq!(output, b"data1data2".to_vec());
}

#[test]
fn test_collect_bytes_into_linefeed_stream() {
    let stream_data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
        Ok(LineFeed::Line("line1".to_string())),
        Ok(LineFeed::Line("line2".to_string())),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send + Sync> = Box::new(
        stream_data
            .into_iter()
            .map(|r| r.map_err(|e| e as BoxedError)),
    );
    let body = SendSafeBody::LineFeedStream(Some(send_iter));
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 12);
    assert_eq!(output, b"line1\nline2\n".to_vec());
}

#[test]
fn test_collect_bytes_into_file() {
    // Test writing to an actual file
    let body = SendSafeBody::Bytes(b"file content".to_vec());
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("test_collect_bytes_into.txt");

    {
        let mut file = std::fs::File::create(&test_file).unwrap();
        let result = collect_bytes_into(body, &mut file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);
    }

    // Verify file contents
    let contents = std::fs::read(&test_file).unwrap();
    assert_eq!(contents, b"file content".to_vec());

    // Cleanup
    let _ = std::fs::remove_file(&test_file);
}

#[test]
fn test_content_length_enforcing_exact_match() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"hello".to_vec())),
        Ok(Data::Bytes(b"world".to_vec())),
    ];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
    // First chunk: "hello" (5 bytes)
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"hello"),
        _ => panic!("expected Bytes"),
    }
    // Second chunk: "world" (5 bytes)
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"world"),
        _ => panic!("expected Bytes"),
    }
    // Inner exhausted, bytes_read == expected → None
    assert!(enforcing.next().is_none());
}

#[test]
fn test_content_length_enforcing_truncated() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![Ok(Data::Bytes(b"short".to_vec()))];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"short"),
        _ => panic!("expected Bytes"),
    }
    // Inner exhausted, bytes_read (5) != expected (10) → error
    let result = enforcing.next();
    assert!(result.is_some());
    let err = result.unwrap().unwrap_err();
    assert_eq!(
        err.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn test_content_length_enforcing_retry_passthrough() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"a".to_vec())),
        Ok(Data::Retry),
        Ok(Data::Bytes(b"b".to_vec())),
    ];
    // "a" (1 byte) + "b" (1 byte) = 2 bytes total; Retry doesn't count
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 2);
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"a"),
        _ => panic!("expected Bytes"),
    }
    assert!(matches!(enforcing.next().unwrap().unwrap(), Data::Retry));
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"b"),
        _ => panic!("expected Bytes"),
    }
    // Inner exhausted, bytes_read == expected (2) → None
    assert!(enforcing.next().is_none());
}

#[test]
fn test_content_length_enforcing_error_passthrough() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"a".to_vec())),
        Err(make_sendable_error("boom")),
    ];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"a"),
        _ => panic!("expected Bytes"),
    }
    let result = enforcing.next();
    assert!(result.is_some());
    assert_eq!(result.unwrap().unwrap_err().to_string(), "boom");
}

#[test]
fn test_content_length_enforcing_zero_expected_empty_body() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 0);
    assert!(enforcing.next().is_none());
}

#[test]
fn test_content_length_enforcing_stops_after_exhaustion() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![Ok(Data::Bytes(b"a".to_vec()))];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut enforcing = ContentLengthEnforcingIterator::new(inner, 5);
    match enforcing.next().unwrap().unwrap() {
        Data::Bytes(b) => assert_eq!(&b, b"a"),
        _ => panic!("expected Bytes"),
    }
    // Should return error on inner None (bytes_read != expected)
    assert!(enforcing.next().is_some_and(|r| r.is_err()));
    // Should return None after error was consumed (inner is now None)
    assert!(enforcing.next().is_none());
}

#[test]
fn test_try_collect_bytes_text() {
    let body = SendSafeBody::Text("hello".to_string());
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"hello".to_vec());
}

#[test]
fn test_try_collect_bytes_bytes() {
    let body = SendSafeBody::Bytes(b"\x00\x01\x02".to_vec());
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"\x00\x01\x02".to_vec());
}

#[test]
fn test_try_collect_bytes_none() {
    let body = SendSafeBody::None;
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_try_collect_bytes_stream_success() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"abc".to_vec())),
        Ok(Data::Bytes(b"def".to_vec())),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let body = SendSafeBody::Stream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"abcdef".to_vec());
}

#[test]
fn test_try_collect_bytes_stream_error_propagates() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"ab".to_vec())),
        Err(make_sendable_error("stream failed")),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let body = SendSafeBody::Stream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "stream failed");
}

#[test]
fn test_try_collect_bytes_stream_none() {
    let body = SendSafeBody::Stream(None);
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_try_collect_bytes_chunked_stream_success() {
    let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
        Ok(ChunkedData::Data(b"foo".to_vec(), None)),
        Ok(ChunkedData::Data(b"bar".to_vec(), None)),
        Ok(ChunkedData::DataEnded),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let body = SendSafeBody::ChunkedStream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"foobar".to_vec());
}

#[test]
fn test_try_collect_bytes_chunked_stream_none() {
    let body = SendSafeBody::ChunkedStream(None);
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_try_collect_bytes_linefeed_stream_success() {
    let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
        Ok(LineFeed::Line("alpha".to_string())),
        Ok(LineFeed::Line("beta".to_string())),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let body = SendSafeBody::LineFeedStream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"alpha\nbeta\n".to_vec());
}

#[test]
fn test_try_collect_bytes_linefeed_stream_none() {
    let body = SendSafeBody::LineFeedStream(None);
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_try_collect_bytes_sse_stream_success() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
        Ok(ParseResult::new(
            Event::Message {
                id: Some("1".to_string()),
                event_type: None,
                data: "event one".to_string(),
                retry: None,
            },
            Some("1".to_string()),
        )),
        Ok(ParseResult::new(
            Event::Message {
                id: None,
                event_type: Some("custom".to_string()),
                data: "event two".to_string(),
                retry: None,
            },
            Some("1".to_string()),
        )),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let body = SendSafeBody::SseStream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    // Each message data followed by \n
    assert_eq!(result.unwrap(), b"event one\nevent two\n".to_vec());
}

#[test]
fn test_try_collect_bytes_sse_stream_skips_comments() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
        Ok(ParseResult::new(
            Event::Message {
                id: None,
                event_type: None,
                data: "real data".to_string(),
                retry: None,
            },
            None,
        )),
        Ok(ParseResult::new(
            Event::Comment("keepalive".to_string()),
            None,
        )),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let body = SendSafeBody::SseStream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    // Only the message event data should appear
    assert_eq!(result.unwrap(), b"real data\n".to_vec());
}

#[test]
fn test_try_collect_bytes_sse_stream_empty() {
    let body = SendSafeBody::SseStream(None);
    let result = try_collect_bytes(body);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_try_collect_string_valid_utf8() {
    let body = SendSafeBody::Text("hello world".to_string());
    let result = try_collect_string(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "hello world");
}

#[test]
fn test_try_collect_string_invalid_utf8() {
    // Invalid UTF-8: 0xFF is never valid in UTF-8
    let body = SendSafeBody::Bytes(vec![0xFF, 0xFE]);
    let result = try_collect_string(body);
    assert!(result.is_err());
}

#[test]
fn test_try_collect_string_sse_stream() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
        Event::Message {
            id: None,
            event_type: None,
            data: "message".to_string(),
            retry: None,
        },
        None,
    ))];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let body = SendSafeBody::SseStream(Some(send_iter));
    let result = try_collect_string(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "message\n");
}

#[test]
fn test_collect_strings_from_send_safe_text() {
    let body = SendSafeBody::Text("unicode: \u{2764}".to_string());
    let result = collect_strings_from_send_safe(body);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "unicode: \u{2764}");
}

#[test]
fn test_collect_strings_from_send_safe_invalid_utf8() {
    let body = SendSafeBody::Bytes(vec![0xC0, 0x80]);
    let result = collect_strings_from_send_safe(body);
    assert!(result.is_err());
}

#[test]
fn test_try_collect_bytes_propagates_content_length_enforcement() {
    // Simulate a LimitedBatchStreamReader that returns fewer bytes than expected
    let data: Vec<Result<Data, SendableBoxedError>> =
        vec![Ok(Data::Bytes(b"partial".to_vec()))];
    let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let enforcing = ContentLengthEnforcingIterator::new(inner, 100);
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(enforcing);
    let body = SendSafeBody::Stream(Some(send_iter));
    let result = try_collect_bytes(body);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    assert!(err.to_string().contains("expected 100 bytes"));
    assert!(err.to_string().contains("got 7"));
}

#[test]
fn test_collect_bytes_from_send_safe_sse_stream() {
    let body = make_sse_stream(vec![
        Ok(ParseResult::new(
            Event::Message {
                id: Some("1".to_string()),
                event_type: None,
                data: "event one".to_string(),
                retry: None,
            },
            Some("1".to_string()),
        )),
        Ok(ParseResult::new(
            Event::Message {
                id: Some("2".to_string()),
                event_type: None,
                data: "event two".to_string(),
                retry: None,
            },
            Some("2".to_string()),
        )),
    ]);
    let result = collect_bytes_from_send_safe(body);
    // Each event data followed by \n
    assert_eq!(result, b"event one\nevent two\n".to_vec());
}

#[test]
fn test_collect_bytes_from_send_safe_sse_skips_comments() {
    let body = make_sse_stream(vec![
        Ok(ParseResult::new(
            Event::Message {
                id: None,
                event_type: None,
                data: "real".to_string(),
                retry: None,
            },
            None,
        )),
        Ok(ParseResult::new(
            Event::Comment("keepalive".to_string()),
            None,
        )),
    ]);
    let result = collect_bytes_from_send_safe(body);
    assert_eq!(result, b"real\n".to_vec());
}

#[test]
fn test_collect_bytes_into_sse_stream() {
    let body = make_sse_stream(vec![Ok(ParseResult::new(
        Event::Message {
            id: None,
            event_type: None,
            data: "sse data".to_string(),
            retry: None,
        },
        None,
    ))]);
    let mut output = Vec::new();
    let result = collect_bytes_into(body, &mut output);
    assert!(result.is_ok());
    assert_eq!(output, b"sse data\n".to_vec());
}

#[test]
fn test_collect_string_strict_sse_stream() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
        Event::Message {
            id: None,
            event_type: None,
            data: "sse message".to_string(),
            retry: None,
        },
        None,
    ))];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let stream: Box<
        dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send,
    > = Box::new(std::iter::once(Ok(IncomingResponseParts::StreamedBody(
        SendSafeBody::SseStream(Some(send_iter)),
    ))));
    let result = collect_string_strict(stream);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sse message\n");
}

#[test]
fn test_collect_bytes_strict_sse_stream() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
        Event::Message {
            id: None,
            event_type: None,
            data: "bytes".to_string(),
            retry: None,
        },
        None,
    ))];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let stream: Box<
        dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send,
    > = Box::new(std::iter::once(Ok(IncomingResponseParts::StreamedBody(
        SendSafeBody::SseStream(Some(send_iter)),
    ))));
    let result = collect_bytes_strict(stream);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"bytes\n".to_vec());
}

#[test]
fn test_send_safe_body_bytes_iterator_bytes() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(b"hello".to_vec()));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"hello"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_bytes_empty() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(vec![]));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_text() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Text("world".into()));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"world"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_text_empty() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Text(String::new()));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_none() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::None);
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_stream_data_chunks() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"a".to_vec())),
        Ok(Data::Retry),
        Ok(Data::Bytes(b"b".to_vec())),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(Some(send_iter)));

    // Chunk "a"
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"a"
    ));
    // Retry → Wait (nothing yet, not bookkeeping).
    assert!(matches!(iter.next(), Some(Stream::Wait)));
    // Chunk "b"
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"b"
    ));
    // Exhausted
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_stream_error() {
    let data: Vec<Result<Data, SendableBoxedError>> = vec![
        Ok(Data::Bytes(b"ok".to_vec())),
        Err(make_sendable_error("boom")),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"ok"
    ));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "boom"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_stream_none() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(None));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_chunked_stream() {
    let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
        Ok(ChunkedData::Data(b"x".to_vec(), None)),
        Ok(ChunkedData::Trailers(vec![])),
        Ok(ChunkedData::DataEnded),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"x"
    ));
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // Trailers
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // DataEnded
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_chunked_stream_error() {
    let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
        Ok(ChunkedData::Data(b"ok".to_vec(), None)),
        Err(make_sendable_error("fail")),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"ok"
    ));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_linefeed_stream() {
    let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
        Ok(LineFeed::Line("hello".into())),
        Ok(LineFeed::SKIP),
        Ok(LineFeed::END),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter =
        SendSafeBodyBytesIterator::new(SendSafeBody::LineFeedStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"hello\n"
    ));
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // SKIP
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // END
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_linefeed_stream_error() {
    let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
        Ok(LineFeed::Line("a".into())),
        Err(make_sendable_error("fail")),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send + Sync> =
        Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
    let mut iter =
        SendSafeBodyBytesIterator::new(SendSafeBody::LineFeedStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"a\n"
    ));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_sse_stream() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
        Ok(ParseResult::new(
            Event::Message {
                id: None,
                event_type: None,
                data: "payload".to_string(),
                retry: None,
            },
            None,
        )),
        Ok(ParseResult::new(Event::Comment("ignore".to_string()), None)),
        Ok(ParseResult::new(Event::Reconnect, None)),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::SseStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"payload"
    ));
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // Comment
    assert!(matches!(iter.next(), Some(Stream::Ignore))); // Reconnect
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_sse_stream_error() {
    let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
        Ok(ParseResult::new(
            Event::Message {
                id: None,
                event_type: None,
                data: "msg".to_string(),
                retry: None,
            },
            None,
        )),
        Err(make_sendable_error("fail")),
    ];
    let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send + Sync> =
        Box::new(data.into_iter());
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::SseStream(Some(send_iter)));

    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"msg"
    ));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_send_safe_body_bytes_iterator_exhaust_is_idempotent() {
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(b"once".to_vec()));
    assert!(iter.next().is_some()); // chunk
    assert!(iter.next().is_none()); // exhausted
    assert!(iter.next().is_none()); // idempotent
    assert!(iter.next().is_none()); // still idempotent
}
