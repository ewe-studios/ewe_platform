//! Build-time helpers for cross-platform compilation.
//!
//! This crate is consumed as a `[build-dependency]` — it runs in `build.rs`
//! scripts, never linked into the runtime binary. It extracts the
//! target-detection, EMSDK wiring, and codegen-backend logic that was
//! previously duplicated across `infrastructure/llama-bindings/build.rs`,
//! `foundation_core/build.rs`, and `foundation_testbed`.

mod cranelift;
mod emsdk;
mod target;
mod wasm;

pub use cranelift::{is_cranelift_active, set_cranelift_cfg};
pub use emsdk::Emsdk;
pub use target::{AppleVariant, TargetInfo, TargetOs, WindowsVariant};
pub use wasm::WasmFlavor;
