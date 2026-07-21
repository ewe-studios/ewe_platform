//! Shared cross-platform components of the REPL.
//!
//! These modules work on all targets and contain the core types, traits,
//! and REPL logic that both native and wasm backends implement.

pub mod config;
pub mod commands;
pub mod repl;
pub mod traits;
pub mod history;
