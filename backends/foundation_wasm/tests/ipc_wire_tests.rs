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

#[test]
fn content_type_request_round_trip_all_variants() {
    // Json request
    let req = IpcRequest {
        ipc: "test".into(), action: "do".into(),
        payload: b"{}".to_vec(), content_type: IpcContentType::Json, target: None,
    };
    let enc = encode_request(&req);
    let dec = decode_request(&enc).unwrap();
    assert_eq!(dec.content_type, IpcContentType::Json);

    // Arrow request
    let req = IpcRequest {
        ipc: "analytics".into(), action: "query".into(),
        payload: vec![0x00, 0x01, 0x02, 0x03],
        content_type: IpcContentType::Arrow, target: Some("db".into()),
    };
    let enc = encode_request(&req);
    let dec = decode_request(&enc).unwrap();
    assert_eq!(dec.content_type, IpcContentType::Arrow);
    assert_eq!(dec.payload, vec![0, 1, 2, 3]);

    // Binary request
    let req = IpcRequest {
        ipc: "file".into(), action: "write".into(),
        payload: vec![0xFF, 0xFE, 0xFD],
        content_type: IpcContentType::Binary, target: None,
    };
    let enc = encode_request(&req);
    let dec = decode_request(&enc).unwrap();
    assert_eq!(dec.content_type, IpcContentType::Binary);
    assert_eq!(dec.payload, vec![0xFF, 0xFE, 0xFD]);
}

#[test]
fn content_type_byte_values_match_ipc_spec() {
    // The wire format contract: 0=Json, 1=Arrow, 2=Binary
    // encode_request writes these bytes; decode_request reads them
    for (ct, expected_byte) in [
        (IpcContentType::Json, 0u8),
        (IpcContentType::Arrow, 1u8),
        (IpcContentType::Binary, 2u8),
    ] {
        let req = IpcRequest {
            ipc: "t".into(), action: "a".into(),
            payload: vec![0xAB], content_type: ct, target: None,
        };
        let enc = encode_request(&req);
        // The content_type byte is at a fixed position: after ipc+action before target
        // Skip: 4(ipc_len) + 1(ipc="t") + 4(action_len) + 1(action="a") = 10
        let ct_byte_offset = 4 + 1 + 4 + 1;
        assert_eq!(enc[ct_byte_offset], expected_byte,
            "content_type {:?} should encode as byte {}", ct, expected_byte);
    }
}
