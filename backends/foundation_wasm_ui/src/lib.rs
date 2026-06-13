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
//! protocol impls (`ColumnarV1`/`BatchInstructionsV1`/`JsonV1`) and the `InstructionReceiver`.

#![no_std]

extern crate alloc;

// Build tooling is a STD, native-only concern (feature 20) — target-gated so
// every native build carries the CLI while wasm builds stay no_std-clean.
#[cfg(not(target_arch = "wasm32"))]
extern crate std;

#[cfg(not(target_arch = "wasm32"))]
pub mod build_tools;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
// Server-driven UI fan-out (broadcaster + App sink). Native (std), like the CLI.
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
#[cfg(not(target_arch = "wasm32"))]
pub use server::{Broadcaster, BroadcastSink, BroadcastTx, FrameTransport};

pub mod app;
pub mod events;
pub mod reactive;
pub mod html_macro;
pub mod slots;
pub mod instruction;
pub mod protocol;
pub mod runtime;
pub mod theme;
pub mod wasm;

#[cfg(feature = "embedded-js")]
pub mod embedded;

pub use events::{install_event_bridge, invoke_signal_callback, uninstall_event_bridge};
pub use html_macro::MaybeCallback;
pub use app::{App, SentBatches};
pub use reactive::{mount_for, mount_show};
pub use slots::{mount_before, mount_fragment, mount_into, Render, Slot};
pub use instruction::{ColumnarReceiver, InstructionReceiver};
// The html! proc macro itself — re-exported so users write
// `use foundation_wasm_ui::html;`.
pub use foundation_macros::{html, theme, ThemeTokens};
// Theme token model + runtime builder, re-exported so `theme!{}` output and
// the no-macro `Theme` builder are reachable from one import path.
pub use foundation_theme::{GeneratedTheme, Theme, ThemeToken};
pub use theme::{inject_theme_css, BODY_NODE_ID, HEAD_NODE_ID, THEME_STYLE_NODE_ID};
pub use runtime::{DomSignalBinding, Runtime, RuntimeBuilder, SharedInstructionReceiver};
pub use protocol::{
    ColumnarV1, BatchInstructionsV1, CollectedFrames, DomOpsBatch, FrameSinkV1, HandleResult,
    JsonV1, MockProtocol, ProtocolMethods, SendResult, BATCH_OP_APPLY_DOM,
};
#[cfg(feature = "arrow")]
pub use protocol::ArrowIpcV2;
// The Arrow IPC Layer-1 encoder (wire v2), re-exported so a server sink can pick
// it without a direct foundation_arrow dependency.
#[cfg(feature = "arrow")]
pub use foundation_arrow::ArrowIpcEncoder;
