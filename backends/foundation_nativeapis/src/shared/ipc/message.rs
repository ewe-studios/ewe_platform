use foundation_core::type_uuid::{Bytes, TypeUuid};
use foundation_macros::TypeUuid;
/// Shareable IPC message types and the MessageBox trait.
///
/// `MessageBox` is a blanket impl for any `T: TypeUuid + Serialize + Deserialize + Send + 'static`.
/// The `#[derive(TypeUuid)]` with `#[uuid = "..."]` attribute provides type identification.
use serde::{Deserialize, Serialize};

use super::{EndpointID, Error, Label, Version};

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

/// Windows-only: request for a remote process handle via the reply pipe protocol.
#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "fbf88372-d2cd-425a-a183-133f8f119df2"]
pub struct FetchProcessHandleMessage {
    pub pid: u32,
    pub reply_pipe: String,
}
