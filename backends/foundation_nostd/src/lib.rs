#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

/// A perfect hash map type that needs no `std`.
///
/// `foundation_macros::include_json!` and `include_yaml!` emit a literal of
/// this type. A crate that uses one of those macros depends on this crate and
/// needs no `phf` dependency of its own.
pub use phf;

pub mod comp;
pub mod embeddable;
pub mod macros;
pub mod mobile;
pub mod primitives;
pub mod raw_parts;
pub mod spin_mutex;
#[cfg(feature = "trace")]
pub mod trace;
