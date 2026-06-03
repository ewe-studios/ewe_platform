/// Message types for the IPC bus.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use super::version::Version;
use super::label::Label;
use super::util::EndpointID;
use super::selector::Selector;

/// Trait for types that can be sent over the IPC bus.
///
/// Requires `Serialize + Encode + Decode<()> + Send + Sync + 'static`.
/// The `Decode<()>` bound is bincode 2's default context.
pub trait MessageBox: Serialize + bincode::Encode + bincode::Decode<()> + Send + Sync + 'static {
    fn type_uuid() -> u128;
}

/// A built-in raw bytes message type.
#[derive(Debug, Clone, Encode, Decode)]
pub struct BytesMessage {
    pub format: u16,
    pub data: Vec<u8>,
}

// Manual Serialize/Deserialize for MessageBox compatibility
impl serde::Serialize for BytesMessage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("BytesMessage", 2)?;
        state.serialize_field("format", &self.format)?;
        state.serialize_field("data", &self.data)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for BytesMessage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Inner {
            format: u16,
            data: Vec<u8>,
        }
        let inner = Inner::deserialize(deserializer)?;
        Ok(BytesMessage { format: inner.format, data: inner.data })
    }
}

impl BytesMessage {
    pub fn new(data: Vec<u8>) -> Self {
        Self { format: 0, data }
    }

    pub fn with_format(format: u16, data: Vec<u8>) -> Self {
        Self { format, data }
    }
}

impl MessageBox for BytesMessage {
    fn type_uuid() -> u128 {
        0xdd95ba8e_1279_47cf_925e_83e614e79588
    }
}

/// Connect message sent by endpoint during handshake.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct ConnectMessage {
    pub version: Version,
    pub token: String,
    pub label: Label,
}

/// Acknowledgement sent by controller during handshake.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum ConnectMessageAck {
    Ok(EndpointID),
    ErrVersion(Version),
    ErrToken,
}

/// A generic IPC message with typed payload.
#[derive(Debug)]
pub struct Message<T> {
    pub selector: Selector,
    pub payload: T,
    pub objects: Vec<crate::ipc::platform::Object>,
    pub memory_regions: Vec<crate::ipc::platform::MemoryRegion>,
}

impl<T> Message<T> {
    pub fn broadcast(payload: T) -> Self {
        Self {
            selector: Selector::broadcast(),
            payload,
            objects: Vec::new(),
            memory_regions: Vec::new(),
        }
    }

    pub fn unicast(label: impl Into<String>, payload: T) -> Self {
        Self {
            selector: Selector::unicast(label),
            payload,
            objects: Vec::new(),
            memory_regions: Vec::new(),
        }
    }

    pub fn with_object(mut self, obj: crate::ipc::platform::Object) -> Self {
        self.objects.push(obj);
        self
    }

    pub fn with_memory_region(mut self, region: crate::ipc::platform::MemoryRegion) -> Self {
        self.memory_regions.push(region);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_message_roundtrip() {
        let msg = BytesMessage::new(b"hello world".to_vec());
        let encoded = bincode::encode_to_vec(&msg, bincode::config::standard()).unwrap();
        let (decoded, _): (BytesMessage, _) =
            bincode::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
        assert_eq!(decoded.data, b"hello world");
    }
}
