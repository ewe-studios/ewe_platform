//! The clients of agent-rpc, one for each platform.
//!
//! WHY: a native client and a browser client share the generated service
//! stubs and the same wire protocol. Only the transport and the credential
//! differ.
//!
//! WHAT: the native client at its stable path, and the browser client
//! ([`web`]) behind the `web-client` feature.
//!
//! HOW: a module is built only on its target and with its feature.

#[cfg(feature = "multi")]
pub use crate::host::native::client::*;

#[cfg(all(feature = "web-client", target_family = "wasm"))]
pub mod web;
