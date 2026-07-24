//! Typed WASM wrappers — compile only on wasm32.
//!
//! Each module exposes a struct with methods that call through the IPC FFI
//! bridge (`ipc_dispatch`). WASM apps import these directly — no native
//! dep needed, just the IPC bridge.

#[cfg(feature = "modal")]
pub mod modal;
#[cfg(feature = "dialog")]
pub mod dialog;
