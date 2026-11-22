//! A web host for typed user interface frames, signals, and templates.

#![no_std]

extern crate alloc;

// Build tooling is a STD, native-only concern (feature 20) — target-gated so
// every native build carries the CLI while wasm builds stay no_std-clean.
#[cfg(not(target_family = "wasm"))]
extern crate std;

#[cfg(not(target_family = "wasm"))]
pub mod build_tools;
#[cfg(not(target_family = "wasm"))]
pub mod cli;
#[cfg(all(feature = "tasks", not(target_family = "wasm")))]
mod runtime_build;
#[cfg(all(feature = "tasks", not(target_family = "wasm")))]
pub mod tasks;

// Server-driven UI fan-out (broadcaster + App sink). Native (std), like the CLI.
#[cfg(not(target_family = "wasm"))]
pub mod server;
#[cfg(not(target_family = "wasm"))]
pub use server::{BroadcastTx, Broadcaster, FrameBroadcastSink, FrameTransport, LiveChannel};

pub mod actions;
pub mod app;
pub mod events;
pub mod html_macro;
pub mod instruction;
mod mounted_scopes;
pub mod protocol;
pub mod reactive;
pub mod runtime;
pub mod slots;
pub mod theme;
pub mod wasm;

#[cfg(feature = "embedded-js")]
pub mod embedded;

pub use app::{App, SentBatches};
pub use events::{
    install_app_bridge, install_event_bridge, invoke_signal_callback, uninstall_event_bridge,
};
/// Shared template types used by the exported `html!` macro.
pub use foundation_ui_traits as template_types;
pub use foundation_ui_wire::{ops, OperationT};
pub use html_macro::MaybeCallback;
pub use instruction::InstructionReceiver;
pub use reactive::{mount_for, mount_show};
pub use slots::{mount_before, mount_fragment, mount_into, Render, Slot};
// The html! proc macro itself — re-exported so users write
// `use foundation_wasm_ui::html;`.
pub use foundation_macros::{html, theme, ThemeTokens};
// Theme token model + runtime builder, re-exported so `theme!{}` output and
// the no-macro `Theme` builder are reachable from one import path.
pub use foundation_theme::{GeneratedTheme, Theme, ThemeToken};
pub use protocol::{
    BatchInstructionsV1, CollectedFrames, FrameV1, HandleResult, MockProtocol, ProtocolError,
    ProtocolMethods, SendResult,
};
pub use runtime::{DomSignalBinding, Runtime, RuntimeBuilder, SharedInstructionReceiver};
pub use theme::{inject_theme_css, BODY_NODE_ID, HEAD_NODE_ID, THEME_STYLE_NODE_ID};
