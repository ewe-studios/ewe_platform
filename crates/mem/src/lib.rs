#![cfg_attr(feature = "nightly", feature(test))]

extern crate lazy_static;
extern crate lazycell;

pub mod encoding;
pub mod memory;
pub mod primitives;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
