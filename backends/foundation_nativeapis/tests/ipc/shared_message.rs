//! `shared::ipc::message` tests (moved from `src/shared/ipc/message.rs`).

use foundation_core::type_uuid::TypeUuid;
use foundation_nativeapis::ipc::errors::Error;
use foundation_nativeapis::ipc::message::{BytesMessage, MessageBox};

#[test]
fn bytes_message_uuid() {
    let msg = BytesMessage {
        format: 0,
        data: b"hello".to_vec(),
    };
    assert_eq!(msg.uuid(), BytesMessage::UUID);
}

#[test]
fn bytes_message_roundtrip() {
    let original = BytesMessage {
        format: 1,
        data: b"test data".to_vec(),
    };
    let encoded = original.encode().unwrap();
    let decoded = BytesMessage::decode(BytesMessage::UUID, &encoded).unwrap();
    assert_eq!(decoded.data, original.data);
    assert_eq!(decoded.format, original.format);
}

#[test]
fn decode_wrong_uuid() {
    let msg = BytesMessage {
        format: 0,
        data: vec![],
    };
    let encoded = msg.encode().unwrap();
    let wrong_uuid = [0u8; 16];
    let result = BytesMessage::decode(wrong_uuid, &encoded);
    assert!(result.is_err());
    assert!(matches!(result, Err(Error::TypeUuidNotFound)));
}
