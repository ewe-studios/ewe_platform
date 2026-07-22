//! Shared cross-platform components of the REPL.
//!
//! These modules work on all targets and contain the core types, traits,
//! and REPL logic that both native and wasm backends implement.

pub mod activity;
pub mod commands;
pub mod config;
pub mod history;
// Both need ratatui, which only a rendering backend pulls in. A build with
// just the wasm logging stub has no renderer to configure.
#[cfg(any(feature = "native", feature = "ratzilla"))]
pub mod host;
pub mod layout;
#[cfg(any(feature = "native", feature = "ratzilla"))]
pub mod render;
// The blocking message loop needs an input backend to block on. A build with
// only a rendering backend (a browser, say) gets the renderer without it.
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod repl;
pub mod theme;
pub mod traits;
