/// Shareable IPC types — platform-agnostic, always compiled when `ipc` feature is enabled.
///
/// Contains message traits, error types, labels, selectors, versioning, and utilities.
/// These types have no platform-specific dependencies and can be used on any target.
///
/// The native transport layer (platform sockets, shared memory, FFI) lives in
/// the top-level `ipc` module which is gated to linux/macos/windows.

pub mod errors;
pub mod label;
pub mod message;
pub mod options;
pub mod selector;
pub mod util;
pub mod version;

pub use errors::{Error, JoinError, RecvError, SendError};
pub use label::{Label, LabelOp};
pub use message::{BytesMessage, ConnectMessage, ConnectMessageAck, MessageBox};
#[cfg(target_os = "windows")]
pub use message::FetchProcessHandleMessage;
pub use options::Options;
pub use selector::{Selector, SelectorMode};
pub use util::{Align4, EndpointID};
pub use version::{Version, version_pre};

/// Decode a value from bincode bytes.
pub fn decode<'de, T: serde::Deserialize<'de>>(data: &'de [u8]) -> Result<T, Error> {
    let (d, _): (T, _) =
        bincode::serde::borrow_decode_from_slice(data, bincode::config::standard())
            .map_err(Error::Decode)?;
    Ok(d)
}

/// Encode a value to bincode bytes.
pub fn encode<T: serde::Serialize>(t: &T) -> Result<Vec<u8>, Error> {
    bincode::serde::encode_to_vec(t, bincode::config::standard()).map_err(Error::Encode)
}
