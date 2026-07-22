//! Shareable capability traits.
//!
//! WHY: traits that any crate may want to implement — and that are not tied to
//! one subsystem — need a neutral home. Defining them inside the crate that
//! happened to need them first (the HTTP layer, say) forces unrelated code to
//! depend on that crate just to implement a general capability.
//!
//! WHAT: general-purpose traits with no domain of their own.
//!
//! Distinct from [`crate::extensions`], which holds `*_ext` traits that add
//! methods to specific existing types.

pub mod try_clone;

pub use try_clone::{TryClone, TryCloneError};
