//! Native (unix/windows) backend using crossterm.
//!
//! Provides raw-mode keyboard input and terminal drawing. Enabled by the
//! `native` feature flag.

#[cfg(feature = "native")]
pub mod input;
#[cfg(feature = "native")]
pub mod display;
