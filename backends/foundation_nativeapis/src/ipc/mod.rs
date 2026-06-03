/// Interprocess message bus (IPC) — adapted from ipmb.
///
/// Provides a bus-based IPC where endpoints join a named bus, send typed messages,
/// and receive messages that match their label selectors.
///
/// # Quick Start
///
/// ```ignore
/// use foundation_nativeapis::ipc::{join, Options, Label, Selector};
///
/// let options = Options::new("com.ewe.test", Label::new("my-endpoint"));
/// let (sender, receiver) = join::<BytesMessage, BytesMessage>(options, None)?;
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

pub use errors::{Error, IpcError, JoinError, SendError, RecvError, Result};
pub use label::{Label, LabelOp};
pub use options::Options;
pub use selector::{Selector, SelectorMode};
pub use version::Version;
pub use util::{EndpointID, Align4};
pub use message::{Message, MessageBox, BytesMessage, ConnectMessage, ConnectMessageAck};
pub use memory_registry::MemoryRegistry;
