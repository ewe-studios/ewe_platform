//! Error-model tests (spec-41 F12 / Decision 03): Code↔HTTP mappings, Connect
//! JSON error serialization byte-for-byte, detail round-trips, and domain-error
//! flow via `From`/`change_context`.

use foundation_connectrpc::error::{
    code_of, wrap_if_context, wrap_if_h2c, wrap_if_rst, wrap_if_uncoded, EndStreamResponse,
    ERRSTACKS_TYPE_URL,
};
use foundation_connectrpc::{Code, ConnectError, ConnectResult, ErrorDetail, WireError, WireErrorDetail};
use foundation_errstacks::ErrorTrace;

// ── Code ↔ HTTP mappings (Decision 03 tables, exact) ──────────────────────────

#[test]
fn code_to_http_status_matches_table() {
    let cases = [
        (Code::Canceled, 499u16),
        (Code::Unknown, 500),
        (Code::InvalidArgument, 400),
        (Code::DeadlineExceeded, 504),
        (Code::NotFound, 404),
        (Code::AlreadyExists, 409),
        (Code::PermissionDenied, 403),
        (Code::ResourceExhausted, 429),
        (Code::FailedPrecondition, 400),
        (Code::Aborted, 409),
        (Code::OutOfRange, 400),
        (Code::Unimplemented, 501),
        (Code::Internal, 500),
        (Code::Unavailable, 503),
        (Code::DataLoss, 500),
        (Code::Unauthenticated, 401),
    ];
    for (code, status) in cases {
        assert_eq!(code.http_status(), status, "http_status for {code}");
    }
}

#[test]
fn http_status_to_code_reverse_table() {
    assert_eq!(Code::from_http_status(400), Code::Internal);
    assert_eq!(Code::from_http_status(401), Code::Unauthenticated);
    assert_eq!(Code::from_http_status(403), Code::PermissionDenied);
    assert_eq!(Code::from_http_status(404), Code::Unimplemented);
    assert_eq!(Code::from_http_status(429), Code::Unavailable);
    assert_eq!(Code::from_http_status(502), Code::Unavailable);
    assert_eq!(Code::from_http_status(503), Code::Unavailable);
    assert_eq!(Code::from_http_status(504), Code::Unavailable);
    // Anything else → Unknown.
    assert_eq!(Code::from_http_status(200), Code::Unknown);
    assert_eq!(Code::from_http_status(418), Code::Unknown);
    assert_eq!(Code::from_http_status(500), Code::Unknown);
}

#[test]
fn code_name_and_number_roundtrip() {
    for n in 1..=16u32 {
        let code = Code::from_u32(n).expect("valid code");
        assert_eq!(code.grpc_code(), n, "grpc_code identity");
        assert_eq!(Code::from_str(code.as_str()), Some(code), "name roundtrip");
    }
    assert_eq!(Code::from_u32(0), None); // gRPC OK is not an error code
    assert_eq!(Code::from_u32(17), None);
    assert_eq!(Code::from_str("nope"), None);
    assert_eq!(Code::InvalidArgument.as_str(), "invalid_argument");
}

// ── Connect JSON error serialization (byte-for-byte) ──────────────────────────

#[test]
fn wire_json_simple_error_is_exact() {
    let err = ConnectError::unavailable("overloaded: back off and retry");
    let json = err.to_json().expect("serialize");
    assert_eq!(
        json,
        r#"{"code":"unavailable","message":"overloaded: back off and retry"}"#
    );
}

#[test]
fn wire_json_empty_message_is_omitted() {
    let err = ConnectError::new(Code::NotFound, "");
    let json = err.to_json().expect("serialize");
    assert_eq!(json, r#"{"code":"not_found"}"#);
}

#[test]
fn wire_json_with_detail_is_exact() {
    // value bytes [10, 2, 8, 60] base64 (unpadded, standard) = "CgIIPA".
    let mut err = ConnectError::unavailable("overloaded: back off and retry");
    err.add_detail(ErrorDetail::from_raw(
        "type.googleapis.com/google.rpc.RetryInfo",
        vec![10, 2, 8, 60],
    ));
    let json = err.to_json().expect("serialize");
    assert_eq!(
        json,
        r#"{"code":"unavailable","message":"overloaded: back off and retry","details":[{"type":"google.rpc.RetryInfo","value":"CgIIPA"}]}"#
    );
}

#[test]
fn wire_error_detail_base64_and_type_roundtrip() {
    let detail = ErrorDetail::from_raw("type.googleapis.com/google.rpc.RetryInfo", vec![10, 2, 8, 60]);
    assert_eq!(detail.type_name(), "google.rpc.RetryInfo");

    let wire = WireErrorDetail::from_detail(&detail);
    assert_eq!(wire.type_url, "google.rpc.RetryInfo"); // JSON `type` = name, no prefix
    assert_eq!(wire.value, "CgIIPA");

    let back = wire.to_detail();
    assert_eq!(back.value(), &[10, 2, 8, 60]);
    assert_eq!(back.type_url(), "type.googleapis.com/google.rpc.RetryInfo");
}

#[test]
fn wire_error_deserializes_back() {
    let json = r#"{"code":"unavailable","message":"m","details":[{"type":"google.rpc.RetryInfo","value":"CgIIPA"}]}"#;
    let we: WireError = serde_json::from_str(json).expect("deserialize");
    assert_eq!(we.code, "unavailable");
    assert_eq!(we.message.as_deref(), Some("m"));
    assert_eq!(we.details.as_ref().unwrap()[0].type_url, "google.rpc.RetryInfo");
}

#[test]
fn end_stream_response_shapes() {
    // Empty end-stream → "{}" (both fields skipped).
    let empty = EndStreamResponse::default();
    assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");

    // With an error.
    let with_err = EndStreamResponse {
        error: Some(ConnectError::internal("boom").to_wire()),
        metadata: None,
    };
    assert_eq!(
        serde_json::to_string(&with_err).unwrap(),
        r#"{"error":{"code":"internal","message":"boom"}}"#
    );
}

// ── errstacks well-known detail ───────────────────────────────────────────────

#[test]
fn errstacks_detail_carries_trace_json() {
    let trace = ErrorTrace::new(ConnectError::internal("db down"));
    let structured = trace.to_structured();
    let detail = ErrorDetail::from_errstacks(&structured);
    assert_eq!(detail.type_url(), ERRSTACKS_TYPE_URL);
    assert_eq!(detail.type_name(), "foundation.errstacks.StructuredErrorTrace");
    // The value is the trace JSON and is retained as wire_json for round-trips.
    assert!(detail.wire_json().is_some());
    let parsed: serde_json::Value = serde_json::from_slice(detail.value()).expect("valid json");
    assert!(parsed.get("current_context").is_some());
}

// ── not_modified sentinel (Decision 03 P15) ───────────────────────────────────

#[test]
fn not_modified_sentinel() {
    let nm = ConnectError::not_modified();
    assert!(nm.is_not_modified());
    assert_eq!(nm.code(), Code::Unknown);

    let regular = ConnectError::unknown("x");
    assert!(!regular.is_not_modified());
}

// ── code_of + wrap_if_* classifiers ───────────────────────────────────────────

#[test]
fn code_of_downcasts_and_walks_chain() {
    let err = ConnectError::permission_denied("nope");
    assert_eq!(code_of(&err), Code::PermissionDenied);

    // A foreign error with no code → Unknown.
    let io = std::io::Error::new(std::io::ErrorKind::Other, "plain");
    assert_eq!(code_of(&io), Code::Unknown);
}

#[test]
fn wrap_if_classifiers() {
    let deadline = std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out");
    assert_eq!(wrap_if_context(&deadline), Some(Code::DeadlineExceeded));

    let canceled = std::io::Error::new(std::io::ErrorKind::Other, "request was canceled");
    assert_eq!(wrap_if_context(&canceled), Some(Code::Canceled));

    let unrelated = std::io::Error::new(std::io::ErrorKind::Other, "disk full");
    assert_eq!(wrap_if_context(&unrelated), None);

    let rst = std::io::Error::new(std::io::ErrorKind::Other, "http2: RST_STREAM received");
    assert_eq!(wrap_if_rst(&rst), Some(Code::Canceled));

    let h2c = std::io::Error::new(std::io::ErrorKind::Other, "http2: unexpected server response");
    assert_eq!(wrap_if_h2c(&h2c), Some(Code::Unavailable));

    // wrap_if_uncoded wraps a foreign error as Unknown, keeping it as source.
    let wrapped = wrap_if_uncoded(std::io::Error::new(std::io::ErrorKind::Other, "x"));
    assert_eq!(wrapped.code(), Code::Unknown);
    assert!(std::error::Error::source(&wrapped).is_some());
}

// ── Domain errors flow via From / change_context ──────────────────────────────

#[derive(Debug)]
enum DomainError {
    UserNotFound(u64),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::UserNotFound(id) => write!(f, "user {id} not found"),
        }
    }
}

impl std::error::Error for DomainError {}

/// A domain layer returns its own typed error trace...
fn load_user(id: u64) -> Result<(), ErrorTrace<DomainError>> {
    Err(ErrorTrace::new(DomainError::UserNotFound(id)))
}

/// ...which maps to a ConnectError at the RPC boundary via `change_context`.
fn rpc_handler() -> ConnectResult<()> {
    load_user(42).map_err(|trace| {
        trace.change_context(ConnectError::not_found("no such user"))
    })?;
    Ok(())
}

#[test]
fn domain_error_changes_context_to_connect_error() {
    let result = rpc_handler();
    let trace = result.expect_err("should fail");
    assert_eq!(trace.current_context().code(), Code::NotFound);
    // The original domain frame is preserved in the trace.
    let structured = trace.to_structured();
    assert!(
        structured.frames.iter().any(|f| f.message.contains("user 42 not found")),
        "domain frame should be preserved, frames: {:?}",
        structured.frames
    );
}

#[test]
fn connect_error_into_trace_composes_with_question_mark() {
    fn inner() -> ConnectResult<()> {
        Err(ConnectError::aborted("conflict"))?
    }
    assert_eq!(inner().unwrap_err().current_context().code(), Code::Aborted);
}
