//! ErrorWriter tests (spec-41 F21 / Decision 03): protocol-correct error
//! rendering for Connect unary/streaming + gRPC-Web (binary + text), 304, and a
//! typed-detail base64 (RawStdEncoding) round-trip.

use bytes::Bytes;
use foundation_netio::simple_http::shared::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use foundation_connectrpc::envelope::Envelope;
use foundation_connectrpc::error::ErrorDetail;
use foundation_connectrpc::protocol::grpc_web::{parse_status_trailers, parse_trailer_frame_body};
use foundation_connectrpc::{Code, ConnectError, ErrorWriter};

fn request(content_type: &str, method: SimpleMethod) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("http://t/svc.Svc/M")
        .with_method(method)
        .add_header(SimpleHeader::CONTENT_TYPE, content_type)
        .build()
        .expect("request")
}

fn blank_response() -> SimpleOutgoingResponse {
    SimpleOutgoingResponse {
        proto: Proto::HTTP11,
        status: Status::OK,
        headers: SimpleHeaders::new(),
        body: None,
        trailers: SimpleHeaders::new(),
    }
}

fn body_bytes(resp: &SimpleOutgoingResponse) -> Vec<u8> {
    match &resp.body {
        Some(SendSafeBody::Bytes(b)) => b.clone(),
        other => panic!("expected byte body, got {other:?}"),
    }
}

fn content_type(resp: &SimpleOutgoingResponse) -> String {
    resp.headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default()
}

#[test]
fn connect_unary_error_status_and_json_body() {
    let writer = ErrorWriter::new();
    let req = request("application/proto", SimpleMethod::POST);
    assert!(writer.is_supported(&req));
    let mut resp = blank_response();
    let err = ConnectError::permission_denied("no access").into();
    writer.write(&mut resp, &req, &err).unwrap();

    assert_eq!(resp.status, Status::Forbidden); // 403
    assert_eq!(content_type(&resp), "application/json"); // errors are always JSON
    assert_eq!(
        body_bytes(&resp),
        br#"{"code":"permission_denied","message":"no access"}"#
    );
}

#[test]
fn connect_streaming_error_is_endstream_envelope() {
    let writer = ErrorWriter::new();
    let req = request("application/connect+proto", SimpleMethod::POST);
    let mut resp = blank_response();
    let err = ConnectError::unavailable("later").into();
    writer.write(&mut resp, &req, &err).unwrap();

    assert_eq!(resp.status, Status::OK); // streaming errors are in-band
    assert_eq!(content_type(&resp), "application/connect+proto");
    let bytes = Bytes::from(body_bytes(&resp));
    let (env, _) = Envelope::decode(&bytes).unwrap();
    assert!(env.is_end_stream(), "0x02 end-stream frame");
    let json: serde_json::Value = serde_json::from_slice(&env.data).unwrap();
    assert_eq!(json["error"]["code"], "unavailable");
}

#[test]
fn grpc_web_error_is_trailer_frame() {
    let writer = ErrorWriter::new();
    let req = request("application/grpc-web+proto", SimpleMethod::POST);
    let mut resp = blank_response();
    let err = ConnectError::unauthenticated("who?").into();
    writer.write(&mut resp, &req, &err).unwrap();

    assert_eq!(resp.status, Status::OK);
    assert_eq!(content_type(&resp), "application/grpc-web+proto");
    let bytes = Bytes::from(body_bytes(&resp));
    let (env, _) = Envelope::decode(&bytes).unwrap();
    assert!(env.is_trailer(), "0x80 trailer frame");
    let trailers = parse_trailer_frame_body(&env.data);
    let parsed = parse_status_trailers(&trailers).expect("status");
    assert_eq!(parsed.current_context().code(), Code::Unauthenticated); // grpc-status 16
}

#[test]
fn grpc_web_text_error_body_is_base64() {
    use base64::Engine as _;
    let writer = ErrorWriter::new();
    let req = request("application/grpc-web-text+proto", SimpleMethod::POST);
    let mut resp = blank_response();
    let err = ConnectError::internal("boom").into();
    writer.write(&mut resp, &req, &err).unwrap();

    assert_eq!(content_type(&resp), "application/grpc-web-text+proto");
    let body = body_bytes(&resp);
    // Body is base64; decoding yields a 0x80 trailer frame.
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&body)
        .expect("valid base64");
    let (env, _) = Envelope::decode(&Bytes::from(raw)).unwrap();
    assert!(env.is_trailer());
}

#[test]
fn not_modified_is_304_no_body_on_get() {
    let writer = ErrorWriter::new();
    let req = request("application/proto", SimpleMethod::GET);
    let mut resp = blank_response();
    let nm = ConnectError::not_modified().into();
    writer.write(&mut resp, &req, &nm).unwrap();

    assert_eq!(resp.status, Status::NotModified);
    assert!(resp.body.is_none());
}

#[test]
fn typed_detail_roundtrips_base64_rawstd() {
    // A detail's value is base64 RawStdEncoding (unpadded) in the Connect JSON.
    let writer = ErrorWriter::new();
    let req = request("application/proto", SimpleMethod::POST);
    let mut resp = blank_response();
    let mut err_ctx = ConnectError::invalid_argument("bad");
    err_ctx.add_detail(ErrorDetail::from_raw(
        "type.googleapis.com/google.rpc.RetryInfo",
        vec![10, 2, 8, 60],
    ));
    let err: foundation_errstacks::ErrorTrace<ConnectError> = err_ctx.into();
    writer.write(&mut resp, &req, &err).unwrap();

    let json: serde_json::Value = serde_json::from_slice(&body_bytes(&resp)).unwrap();
    let detail = &json["details"][0];
    assert_eq!(detail["type"], "google.rpc.RetryInfo");
    assert_eq!(detail["value"], "CgIIPA"); // base64 RawStd of [10,2,8,60]
}
