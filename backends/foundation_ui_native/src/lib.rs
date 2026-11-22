#![doc = include_str!("../README.md")]
uniffi::setup_scaffolding!();

pub mod command;
mod core;
mod host;
pub mod rpc;
pub use core::{CoreConfig, NativeCore, NativeCoreBuilder, NativePlatform};
pub use host::{
    action_sender, CoreError, EventInput, FrameListener, HostTransport, HostTransportProvider,
    NativeSender, ServiceError,
};

#[cfg(feature = "app")]
mod sink;
#[cfg(feature = "app")]
pub use sink::CoreSink;

#[cfg(feature = "tasks")]
pub mod tasks;
