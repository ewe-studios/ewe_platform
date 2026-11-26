//! The HTTP adapter helpers for Durable Streams.
//!
//! The adapter owns the wire rules. The core stays transport-neutral.

pub mod status;

pub use status::{header, status_for};
