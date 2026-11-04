//! SignalR MessagePack notification framing (spec-57, F008).
//!
//! Bitwarden uses SignalR with MessagePack encoding for real-time sync.
//! This module handles the framing: update types, serialization, and
//! notification creation. Transport (WebSocket/Durable Object) is a
//! separate platform concern (see `server/`).

use serde::Serialize;

/// Notification update types sent to connected clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum UpdateType {
    SyncCipherUpdate = 1,
    SyncCipherCreate = 2,
    SyncLoginDelete = 3,
    SyncFolderDelete = 4,
    SyncCiphers = 5,
    SyncVault = 6,
    SyncOrgKeys = 7,
    SyncFolderCreate = 8,
    SyncFolderUpdate = 9,
    SyncCipherDelete = 10,
    SyncSettings = 11,
    SyncLogOut = 12,
    SyncSendCreate = 13,
    SyncSendUpdate = 14,
    SyncSendDelete = 15,
    AuthRequest = 16,
    AuthRequestResponse = 17,
}

/// A notification envelope sent to clients.
#[derive(Debug, Serialize)]
pub struct Notification {
    #[serde(rename = "Type")]
    pub update_type: UpdateType,
    #[serde(rename = "Payload", skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl Notification {
    /// Create a new notification.
    #[must_use]
    pub fn new(update_type: UpdateType, payload: Option<serde_json::Value>) -> Self {
        Self {
            update_type,
            payload,
        }
    }

    /// Serialize to MessagePack bytes (Bitwarden SignalR envelope).
    ///
    /// Uses `[messageType, payloadJsonString]` framing.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        let payload_str = self
            .payload
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
            .unwrap_or_default();
        rmp_serde::encode::to_vec_named(&(self.update_type as u8, payload_str))
    }
}

/// Serialize a value as MessagePack bytes.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn serialize_msgpack<T: Serialize>(value: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    rmp_serde::encode::to_vec_named(value)
}

/// Decode a `VarInt` (up to 5 bytes) from the start of a byte slice.
#[must_use]
pub fn decode_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate().take(5) {
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
        if shift >= 35 {
            return None;
        }
    }
    None
}

/// Encode a u32 as a `VarInt` byte vector.
#[must_use]
pub fn encode_varint(mut value: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

/// Decode a `SignalR` `MessagePack` frame into `(message_type, payload_bytes)`.
#[must_use]
pub fn decode_signalr_frame(data: &[u8]) -> Option<(u8, &[u8])> {
    let (len, consumed) = decode_varint(data)?;
    let len = usize::try_from(len).ok()?;
    let payload = data.get(consumed..consumed + len)?;
    // The payload is a MessagePack array `[msg_type, ..]`. Its first element
    // is the hub invocation message type.
    let value: serde_json::Value = rmp_serde::from_slice(payload).ok()?;
    let msg_type = u8::try_from(value.as_array()?.first()?.as_u64()?).ok()?;
    Some((msg_type, payload))
}

/// Encode a [`Notification`] as a complete `SignalR` WebSocket binary frame:
/// a `VarInt` length prefix and `[1, {Target: "...", Arguments: [payload_json]}]`.
///
/// # Errors
///
/// Returns a [`String`] error when the `MessagePack` serialization fails.
pub fn encode_signalr_notification(notif: &Notification) -> Result<Vec<u8>, String> {
    let payload_json = notif
        .payload
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok())
        .unwrap_or_default();

    let target = match notif.update_type {
        UpdateType::SyncCipherCreate => "SyncCipherCreate",
        UpdateType::SyncCipherUpdate => "SyncCipherUpdate",
        UpdateType::SyncLoginDelete => "SyncLoginDelete",
        UpdateType::SyncFolderDelete => "SyncFolderDelete",
        UpdateType::SyncCiphers => "SyncCiphers",
        UpdateType::SyncVault => "SyncVault",
        UpdateType::SyncOrgKeys => "SyncOrgKeys",
        UpdateType::SyncFolderCreate => "SyncFolderCreate",
        UpdateType::SyncFolderUpdate => "SyncFolderUpdate",
        UpdateType::SyncCipherDelete => "SyncCipherDelete",
        UpdateType::SyncSettings => "SyncSettings",
        UpdateType::SyncLogOut => "SyncLogOut",
        UpdateType::SyncSendCreate => "SyncSendCreate",
        UpdateType::SyncSendUpdate => "SyncSendUpdate",
        UpdateType::SyncSendDelete => "SyncSendDelete",
        UpdateType::AuthRequest => "AuthRequest",
        UpdateType::AuthRequestResponse => "AuthRequestResponse",
    };

    // A SignalR hub invocation is `[1, {Target: "...", Arguments: [...]}]`.
    let invocation = rmp_serde::encode::to_vec_named(&(
        1u8,
        serde_json::json!({
            "Target": target,
            "Arguments": [payload_json],
        }),
    ))
    .map_err(|e| e.to_string())?;

    let frame_len = u32::try_from(invocation.len()).map_err(|e| e.to_string())?;
    let mut frame = encode_varint(frame_len);
    frame.extend_from_slice(&invocation);
    Ok(frame)
}
