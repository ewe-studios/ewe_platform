/// Message types for the IPC bus.
///
/// `MessageBox` is a blanket impl for any `T: TypeUuid + Serialize + Deserialize + Send + 'static`.
/// The `#[derive(TypeUuid)]` with `#[uuid = "..."]` attribute provides type identification.

use serde::{Deserialize, Serialize};
use type_uuid::{Bytes, TypeUuid};

use super::{EndpointID, Error, Label, MemoryRegion, Object, Selector, Version};

/// A message with typed payload, kernel objects, and shared memory regions.
pub struct Message<T> {
    pub(crate) selector: Selector,
    pub payload: T,
    pub objects: Vec<Object>,
    pub memory_regions: Vec<MemoryRegion>,
}

impl<T: MessageBox> Message<T> {
    pub fn new(mut selector: Selector, payload: T) -> Self {
        selector.uuid = payload.uuid();

        Self {
            selector,
            payload,
            objects: vec![],
            memory_regions: vec![],
        }
    }
}

/// Trait for types that can be sent over the IPC bus.
///
/// Implemented as a blanket impl for any `T: TypeUuid + Serialize + Deserialize + Send + 'static`.
pub trait MessageBox: Send + 'static {
    fn decode(uuid: Bytes, data: &[u8]) -> Result<Self, Error>
    where
        Self: Sized;

    fn encode(&self) -> Result<Vec<u8>, Error>;

    fn uuid(&self) -> Bytes;
}

/// Blanket impl: any TypeUuid + Serialize + Deserialize type is a MessageBox.
impl<T: TypeUuid + Serialize + for<'de> Deserialize<'de> + Send + 'static> MessageBox for T {
    fn decode(uuid: Bytes, data: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        if uuid == T::UUID {
            let (decoded, _): (T, _) =
                bincode::serde::borrow_decode_from_slice(data, bincode::config::standard())
                    .map_err(Error::Decode)?;
            Ok(decoded)
        } else {
            Err(Error::TypeUuidNotFound)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, Error> {
        bincode::serde::encode_to_vec(self, bincode::config::standard()).map_err(Error::Encode)
    }

    fn uuid(&self) -> Bytes {
        T::UUID
    }
}

/// A predefined raw bytes message type.
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "dd95ba8e-1279-47cf-925e-83e614e79588"]
pub struct BytesMessage {
    pub format: u16,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Connect message sent by endpoint during handshake.
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "b2c1deb3-3091-4a74-a99c-c8e8d710d4b2"]
pub struct ConnectMessage {
    pub version: Version,
    pub token: String,
    pub label: Label,
}

/// Acknowledgement sent by controller during handshake.
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "c3de9eb4-c310-4c14-9747-093d62c09998"]
pub enum ConnectMessageAck {
    Ok(EndpointID),
    ErrVersion(Version),
    ErrToken,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
