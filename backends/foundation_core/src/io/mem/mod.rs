#![cfg_attr(feature = "nightly", feature(test))]

pub mod encoding;
pub mod memory;
pub mod primitives;
pub mod stringpointer;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
