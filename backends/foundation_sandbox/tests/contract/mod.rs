//! Contract tests. Every test lives here and in the submodules.

mod common;

mod container;
#[cfg(feature = "docker")]
mod docker;
mod error;
mod host;
mod ids;
mod message;
mod sequence;
mod spec;
