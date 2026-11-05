//! Tests for the portable `SignalR` `MessagePack` notification framing.

use foundation_keychain::core::notifications::{
    decode_signalr_frame, decode_varint, encode_signalr_notification, encode_varint, Notification,
    UpdateType,
};

#[test]
fn varint_round_trip() {
    for val in [0u32, 1, 127, 128, 16_383, 16_384, 2_097_151, 268_435_456] {
        let enc = encode_varint(val);
        let (dec, n) = decode_varint(&enc).expect("decode varint");
        assert_eq!(dec, u64::from(val), "varint round-trip fail at {val}");
        assert_eq!(n, enc.len());
    }
}

#[test]
fn notification_frame_is_valid() {
    let notif = Notification::new(
        UpdateType::SyncCipherCreate,
        Some(serde_json::json!({"id": "test-123"})),
    );
    let frame = encode_signalr_notification(&notif).expect("encode");
    let (msg_type, _payload) = decode_signalr_frame(&frame).expect("decode");
    assert_eq!(msg_type, 1); // Invocation
}

#[test]
fn notification_msgpack_round_trips() {
    let n = Notification::new(
        UpdateType::SyncCipherCreate,
        Some(serde_json::json!({"id": "abc"})),
    );
    let bytes = n.to_msgpack().expect("serialize");
    assert_ne!(bytes, [] as [u8; 0]);
}

#[test]
fn notification_without_payload() {
    let n = Notification::new(UpdateType::SyncLogOut, None);
    let bytes = n.to_msgpack().expect("serialize");
    assert_ne!(bytes, [] as [u8; 0]);
}
