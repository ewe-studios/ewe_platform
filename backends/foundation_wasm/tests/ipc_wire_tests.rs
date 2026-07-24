//! F41 — IPC binary wire format round-trip tests.
//!
//! Validates that `encode_request` / `encode_response` / `decode_request` /
//! `decode_response` are symmetric and lossless.

use foundation_wasm::ipc::{
    IpcContentType, IpcError, IpcRequest, IpcResponse,
    encode_request, encode_response, decode_request, decode_response,
    is_valid_ipc_name,
};

#[test]
fn encode_decode_request_round_trip() {
    let req = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: b"{\"facing\":\"back\"}".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);
    let decoded = decode_request(&encoded).unwrap();
    assert_eq!(decoded.ipc, "camera");
    assert_eq!(decoded.action, "capture");
    assert_eq!(decoded.content_type, IpcContentType::Json);
    assert_eq!(decoded.target, None);
    assert_eq!(decoded.payload, b"{\"facing\":\"back\"}");
}

#[test]
fn encode_decode_response_round_trip() {
    let resp = IpcResponse {
        payload: b"{\"handle\":7}".to_vec(),
        content_type: IpcContentType::Json,
    };
    let encoded = encode_response(&resp);
    let decoded = decode_response(&encoded).unwrap();
    assert_eq!(decoded.content_type, IpcContentType::Json);
    assert_eq!(decoded.payload, b"{\"handle\":7}");
}

#[test]
fn request_with_target_preserves_target() {
    let req = IpcRequest {
        ipc: "emit".into(),
        action: "broadcast".into(),
        payload: b"data".to_vec(),
        content_type: IpcContentType::Binary,
        target: Some("wv_3".into()),
    };
    let encoded = encode_request(&req);
    let decoded = decode_request(&encoded).unwrap();
    assert_eq!(decoded.target.as_deref(), Some("wv_3"));
    assert_eq!(decoded.content_type, IpcContentType::Binary);
}

#[test]
fn empty_payload_round_trips() {
    let req = IpcRequest {
        ipc: "ping".into(),
        action: "check".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);
    let decoded = decode_request(&encoded).unwrap();
    assert!(decoded.payload.is_empty());
}

#[test]
fn large_payload_round_trips() {
    let payload = vec![0xABu8; 10_000];
    let req = IpcRequest {
        ipc: "filesystem".into(),
        action: "write".into(),
        payload: payload.clone(),
        content_type: IpcContentType::Binary,
        target: None,
    };
    let encoded = encode_request(&req);
    let decoded = decode_request(&encoded).unwrap();
    assert_eq!(decoded.payload, payload);
}

#[test]
fn long_ipc_names_round_trip() {
    let name = "a".repeat(64);
    let req = IpcRequest {
        ipc: name.clone(),
        action: "do".into(),
        payload: vec![1],
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);
    let decoded = decode_request(&encoded).unwrap();
    assert_eq!(decoded.ipc, name);
}

#[test]
fn decode_garbage_returns_error() {
    assert!(decode_response(&[0]).is_err());
    assert!(decode_response(&[]).is_err());
    assert!(decode_request(&[0, 0, 0, 0, 255]).is_err());
}

#[test]
fn content_type_values_preserved() {
    for ct in [IpcContentType::Json, IpcContentType::Arrow, IpcContentType::Binary] {
        let resp = IpcResponse { payload: vec![1, 2, 3], content_type: ct };
        let encoded = encode_response(&resp);
        let decoded = decode_response(&encoded).unwrap();
        assert_eq!(decoded.content_type, ct);
    }
}

#[test]
fn name_validation() {
    assert!(is_valid_ipc_name("camera"));
    assert!(is_valid_ipc_name("file-system"));
    assert!(is_valid_ipc_name("biometric_auth"));

    assert!(!is_valid_ipc_name(""));
    assert!(!is_valid_ipc_name(&"x".repeat(65)));
    assert!(!is_valid_ipc_name("has spaces"));
    assert!(!is_valid_ipc_name("has/slash"));
}
