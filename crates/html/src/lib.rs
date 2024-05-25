#![cfg_attr(feature = "nightly", feature(test))]

extern crate lazycell;

pub mod core;
pub mod encoding;
pub mod memory;
pub mod primitives;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
