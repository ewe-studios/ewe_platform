#![cfg_attr(feature = "nightly", feature(test))]

extern crate strum_macros;

pub mod markup;
pub mod parsers;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
