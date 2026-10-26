//! Async VFS regression tests: the overlay, the IPC client and daemon, and
//! `exec_async`, called through the async traits.

#![cfg(feature = "vfs")]

mod overlay;

#[cfg(all(feature = "vfs-ipc", feature = "vfs-native"))]
mod ipc;

mod exec;
