//! F43 — IPC FFI callback round-trip tests.
//!
//! Uses `ReplyEncoder` to build protocol-format response data — the same
//! Rust encoder that mirrors JS `ReplyEncoder.encode()`.

use std::sync::mpsc;

use foundation_wasm::internal_api;
use foundation_wasm::ipc::{encode_response, IpcContentType, IpcError, IpcResponse};
use foundation_wasm::ipc_ffi::ipc_resolve;
use foundation_wasm::reply_encoder::encode_reply_into_slot;
use foundation_wasm::{
    ReturnTypeHints, ReturnTypeId, ReturnValues, Returns, ThreeState,
};

fn register_cb(
    tx: mpsc::Sender<Result<IpcResponse<Vec<u8>>, IpcError>>,
) -> u64 {
    let token = internal_api::register_callback(
        ReturnTypeHints::One(ThreeState::Two(
            ReturnTypeId::Uint8ArrayBuffer,
            ReturnTypeId::ErrorCode,
        )),
        move |r| {
            let decoded = match r {
                Ok(Returns::One(ReturnValues::Uint8Array(buf))) => {
                    foundation_wasm::ipc::decode_response(buf.as_slice())
                }
                _ => Err(IpcError::InvalidPayload),
            };
            let _ = tx.send(decoded);
        },
    );
    token.into()
}

fn marshal_response(bytes: &[u8]) -> u64 {
    encode_reply_into_slot(&[ReturnValues::Uint8Array(bytes.to_vec())])
}

#[test]
fn round_trip_json_response() {
    let (tx, rx) = mpsc::channel();
    let cb = register_cb(tx);
    let resp = IpcResponse { payload: b"hello".to_vec(), content_type: IpcContentType::Json };
    ipc_resolve(cb, marshal_response(&encode_response(&resp)));
    let r = rx.recv().unwrap().unwrap();
    assert_eq!(r.payload, b"hello");
    assert_eq!(r.content_type, IpcContentType::Json);
}

#[test]
fn empty_payload() {
    let (tx, rx) = mpsc::channel();
    let cb = register_cb(tx);
    let resp = IpcResponse { payload: vec![], content_type: IpcContentType::Json };
    ipc_resolve(cb, marshal_response(&encode_response(&resp)));
    assert!(rx.recv().unwrap().unwrap().payload.is_empty());
}

#[test]
fn binary_content_type() {
    let (tx, rx) = mpsc::channel();
    let cb = register_cb(tx);
    let resp = IpcResponse { payload: vec![0, 1, 2, 0xFF], content_type: IpcContentType::Binary };
    ipc_resolve(cb, marshal_response(&encode_response(&resp)));
    let r = rx.recv().unwrap().unwrap();
    assert_eq!(r.payload, vec![0, 1, 2, 0xFF]);
    assert_eq!(r.content_type, IpcContentType::Binary);
}

#[test]
fn large_payload() {
    let (tx, rx) = mpsc::channel();
    let cb = register_cb(tx);
    let data = vec![0xABu8; 50_000];
    let resp = IpcResponse { payload: data.clone(), content_type: IpcContentType::Json };
    ipc_resolve(cb, marshal_response(&encode_response(&resp)));
    assert_eq!(rx.recv().unwrap().unwrap().payload, data);
}

#[test]
fn multiple_callbacks_independent() {
    let (t1, r1) = mpsc::channel();
    let (t2, r2) = mpsc::channel();
    let c1 = register_cb(t1);
    let c2 = register_cb(t2);
    ipc_resolve(c2, marshal_response(&encode_response(&IpcResponse { payload: b"#2".to_vec(), content_type: IpcContentType::Json })));
    ipc_resolve(c1, marshal_response(&encode_response(&IpcResponse { payload: b"#1".to_vec(), content_type: IpcContentType::Json })));
    assert_eq!(r2.recv().unwrap().unwrap().payload, b"#2");
    assert_eq!(r1.recv().unwrap().unwrap().payload, b"#1");
}
