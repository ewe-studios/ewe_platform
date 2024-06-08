#![cfg_attr(feature = "nightly", feature(test))]

pub mod markup;
pub mod parsers;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
