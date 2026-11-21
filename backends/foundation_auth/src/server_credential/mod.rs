//! Server credential primitives.
//!
//! This module holds the small auth types that every server needs and no
//! server should define twice: one redacted [`Secret`], one bearer parser, one
//! constant-time comparison, the [`AuthInfo`] result, the [`ClientAuth`] trait
//! and a [`StaticToken`] provider.
//!
//! The sandbox service (spec 77), the agent RPC server (spec 83), the signed
//! secret provider (spec 81) and the mutual-TLS (mTLS) provider (spec 87) use
//! these types. They do not define a second secret type, a second bearer
//! parser or a second comparison.

mod secret;

pub use secret::{ConfidentialText, Secret};

pub use crate::shared::constant_time_eq;
