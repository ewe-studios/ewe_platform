//! The shared library code: value types and interfaces for every target.
//!
//! Nothing in this module starts a sandbox, reads a clock or touches a file.
//! The control plane (feature 02), the slave (feature 01) and the drivers
//! (features 05, 07, 09 and 10) add the behaviour.

pub mod command;
pub mod container;
pub mod control;
pub mod driver;
pub mod error;
pub mod handles;
pub mod host;
pub mod ids;
pub mod images;
pub mod krun_config;
pub mod limits;
pub mod listen;
pub mod platform;
pub mod rpc;
pub mod sequence;
pub mod serde_helpers;
pub mod spec;
pub mod unpack;
pub mod user;

/// The Cloudflare part of the platform. Feature 07 fills this module.
#[cfg(feature = "cloudflare")]
pub mod cloudflare;

use std::pin::Pin;

use futures::Stream;

use crate::shared::error::SandboxResult;
use crate::shared::rpc::messages::{ExecEventMsg, FileChunk};

/// A boxed future that is `Send`.
pub type BoxFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// A stream of byte chunks.
pub type BoxByteStream = Pin<Box<dyn Stream<Item = SandboxResult<Vec<u8>>> + Send>>;

/// A stream of execution events from the slave.
pub type ExecEventStream = Pin<Box<dyn Stream<Item = SandboxResult<ExecEventMsg>> + Send>>;

/// A stream of file chunks from the slave.
pub type FileChunkStream = Pin<Box<dyn Stream<Item = SandboxResult<FileChunk>> + Send>>;

/// Any byte stream that a caller gives to a write call.
///
/// A signature that says `impl ByteStream` accepts any
/// `futures::Stream<Item = SandboxResult<Vec<u8>>> + Send + 'static`.
pub trait ByteStream: Stream<Item = SandboxResult<Vec<u8>>> + Send + 'static {}

impl<T> ByteStream for T where T: Stream<Item = SandboxResult<Vec<u8>>> + Send + 'static {}

/// `Secret` and `extract_bearer_token` from spec 86.
///
/// This crate never defines a second secret type. Later features read the
/// re-export here.
pub use foundation_auth::server_credential::{extract_bearer_token, Secret};
