/// Interprocess message bus (IPC) — native transport layer.
///
/// This module provides the platform-specific transport (sockets, pipes, shared memory)
/// and the bus controller. It re-exports all shareable types from `shared::ipc`.
///
/// # Architecture
///
/// ```text
/// shared::ipc          — MessageBox, errors, labels, selectors, version (any platform)
/// ipc (this module)    — Message<T>, join(), platform transport (linux/macos/windows only)
/// ```

// Re-export all shareable types so users import everything from `ipc::`.
pub use crate::shared::ipc::errors::{self, Error, JoinError, RecvError, SendError};
pub use crate::shared::ipc::label::{self, Label, LabelOp};
pub use crate::shared::ipc::message::{self, BytesMessage, ConnectMessage, ConnectMessageAck, MessageBox};
#[cfg(target_os = "windows")]
pub use crate::shared::ipc::message::FetchProcessHandleMessage;
pub use crate::shared::ipc::options::{self, Options};
pub use crate::shared::ipc::selector::{self, Selector, SelectorMode};
pub use crate::shared::ipc::util::{self, Align4, EndpointID};
pub use crate::shared::ipc::version::{self, Version, version_pre};
pub use crate::shared::ipc::{decode, encode};

// Native-only modules
pub mod platform;
pub mod bus_controller;
pub mod memory_registry;
pub mod ffi;

pub use memory_registry::MemoryRegistry;
pub use platform::{MemoryRegion, Object};
pub use bus_controller::{join, EndpointSender, EndpointReceiver};

/// The `Message<T>` struct — native-only because it carries platform `Object`s and `MemoryRegion`s.
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
