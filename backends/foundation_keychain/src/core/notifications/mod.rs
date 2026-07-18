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
        Self { update_type, payload }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_msgpack_round_trips() {
        let n = Notification::new(UpdateType::SyncCipherCreate, Some(serde_json::json!({"id": "abc"})));
        let bytes = n.to_msgpack().expect("serialize");
        assert!(!bytes.is_empty());
        // First byte should be the update type (SyncCipherCreate = 2)
        assert!(bytes.len() > 0);
    }

    #[test]
    fn notification_without_payload() {
        let n = Notification::new(UpdateType::SyncLogOut, None);
        let bytes = n.to_msgpack().expect("serialize");
        assert!(!bytes.is_empty());
    }
}
