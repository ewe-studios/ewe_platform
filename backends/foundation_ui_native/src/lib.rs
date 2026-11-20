#![doc = include_str!("../README.md")]
uniffi::setup_scaffolding!();

pub mod command;
mod core;
mod host;
pub use core::{CoreConfig, NativeCore, NativePlatform};
pub use host::{CoreError, EventInput, FrameListener, HostTransport};

#[cfg(feature = "app")]
mod sink;
#[cfg(feature = "app")]
pub use sink::CoreSink;
