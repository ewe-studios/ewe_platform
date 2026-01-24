#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod embeddable;
pub mod primitives;
pub mod raw_parts;
