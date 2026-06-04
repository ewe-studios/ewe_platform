/// Interprocess message bus (IPC) — adapted from ipmb.
///
/// Provides a bus-based IPC where endpoints join a named bus, send typed messages,
/// and receive messages that match their label selectors.
///
/// # Architecture
///
/// ```text
/// Process A                          Process B (controller)
///     │                                  │
///     │  look_up("com.ewe.watchers")     │
///     │  ──► connect to abstract socket  │
///     │  ──► send ConnectMessage         │
///     │      with socketpair write fd     │
///     │                                  │
///     │  ◄── ConnectMessageAck(Ok(id))   │
///     │      on socketpair read fd        │
///     │                                  │
///     │  send(Message) ────────────────► │
///     │                                  │  route by label
///     │                                  │ ──────────────► Process C
/// ```
///
/// # Quick Start
///
/// ```ignore
/// use foundation_nativeapis::ipc::{join, Options, Label, BytesMessage};
///
/// let options = Options::new("com.ewe.test", Label::new("my-endpoint"))
///     .controller_affinity(true);
/// let (sender, mut receiver) = join::<BytesMessage, BytesMessage>(options, None)?;
///
/// sender.send(Message::broadcast(BytesMessage::new(b"hello".to_vec())))?;
/// let msg = receiver.recv(Some(Duration::from_secs(5)))?;
/// ```

pub mod errors;
pub mod label;
pub mod options;
pub mod selector;
pub mod version;
pub mod util;
pub mod platform;
pub mod message;
pub mod bus_controller;
pub mod memory_registry;
pub mod ffi;

pub use errors::{Error, JoinError, RecvError, SendError};
pub use label::{Label, LabelOp};
pub use options::Options;
pub use selector::{Selector, SelectorMode};
pub use version::{Version, version_pre};
pub use util::{EndpointID, Align4};
pub use message::{BytesMessage, Message, MessageBox, ConnectMessage, ConnectMessageAck};
#[cfg(target_os = "windows")]
pub use message::FetchProcessHandleMessage;
pub use memory_registry::MemoryRegistry;

// Re-export platform types
pub use platform::{MemoryRegion, Object};

// Public API
pub use bus_controller::{join, EndpointSender, EndpointReceiver};

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
