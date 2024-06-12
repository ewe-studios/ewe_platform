#![cfg_attr(feature = "nightly", feature(test))]

extern crate lazy_regex;
extern crate lazy_static;

#[cfg_attr(any(tracing), macro_use)]
extern crate tracing;

pub mod markup;
pub mod parsers;

#[cfg(test)]
#[cfg(feature = "nightly")]
mod bench;
