//! Backward-compatibility shim: the `netcap` module path is preserved for
//! downstream crates that `use foundation_netio::netcap::*`.
//!
//! All implementation lives under `crate::shared` (platform-agnostic) and
//! `crate::native` (non-wasm). This file re-exports both so existing
//! `use foundation_netio::netcap::RawStream` still works.
//!
//! Each module is re-exported twice: as a name (so `netcap::context::ConnectionContext`
//! resolves) and as a glob (so `netcap::ConnectionContext` resolves).

// ── Shared modules (always compiled) ──────────────────────────────────
pub use crate::shared::context;
pub use crate::shared::core;
pub use crate::shared::errors;

pub use crate::shared::context::*;
pub use crate::shared::core::*;
pub use crate::shared::errors::*;

// ── Native-only modules ───────────────────────────────────────────────
#[cfg(not(target_family = "wasm"))]
pub use crate::native::connection;
#[cfg(not(target_family = "wasm"))]
pub use crate::native::raw_stream;
#[cfg(not(target_family = "wasm"))]
pub use crate::native::ssl;

#[cfg(not(target_family = "wasm"))]
pub use crate::native::connection::*;
#[cfg(not(target_family = "wasm"))]
pub use crate::native::raw_stream::*;

// ── Wasm-only stub ────────────────────────────────────────────────────
#[cfg(target_family = "wasm")]
pub use crate::wasm::netcap;
#[cfg(target_family = "wasm")]
pub use crate::wasm::netcap::*;
