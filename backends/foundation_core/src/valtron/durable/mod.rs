//! Compatibility re-export of the valtron wrappers.
//!
//! The wrappers moved to `foundation_core::durable::valtron`. This module keeps
//! the old `foundation_core::valtron::durable` path working.

pub use crate::durable::shared::valtron::*;
