//! The drivers that live in the portable part of the control plane.
//!
//! A driver here starts no local process. It reaches another control plane
//! through the remote procedure call (RPC) client of feature 06.

#[cfg(feature = "control")]
pub mod nested;
