//! # `foundation_wasm_ui`
//!
//! WHY: `foundation_wasm` is the pure WASM↔JS ABI — memory, binary messaging,
//! function invocation — with no DOM concepts. UI applications still need DOM/window
//! bindings and protocol implementations; those live here so the ABI layer stays
//! generic and reusable (decision 015, feature 00).
//!
//! WHAT: Owns all DOM/window/animation bindings and the WASM protocol
//! implementations that compose `foundation_ui_traits` encoders (Layer 1) with the
//! `foundation_wasm` transport (Layer 2).
//!
//! HOW: Depends on `foundation_wasm` (ABI + transport) and `foundation_ui_traits`
//! (encoders). Adds `wasm::dom` (DOM references), and — in later feature work —
//! protocol impls (`ArrowV1`/`BatchInstructionsV1`/`JsonV1`) and the `InstructionReceiver`.

#![no_std]

extern crate alloc;

pub mod html_macro;
pub mod instruction;
pub mod protocol;
pub mod runtime;
pub mod wasm;

#[cfg(feature = "embedded-js")]
pub mod embedded;

pub use html_macro::MaybeCallback;
pub use instruction::InstructionReceiver;
// The html! proc macro itself — re-exported so users write
// `use foundation_wasm_ui::html;`.
pub use foundation_macros::html;
pub use runtime::{DomSignalBinding, Runtime, RuntimeBuilder, SharedInstructionReceiver};
pub use protocol::{
    ArrowV1, BatchInstructionsV1, DomOpsBatch, HandleResult, JsonV1, MockProtocol,
    ProtocolMethods, SendResult, BATCH_OP_APPLY_DOM,
};
