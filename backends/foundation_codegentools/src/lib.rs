//! Code generation tooling for the ewe platform.

/// The declaration a crate makes about what it generates — [`generate`], with
/// `check()` and `write()`. Callable from a `build.rs` (see the module docs for
/// which verb belongs there).
pub mod codegen;
#[cfg(feature = "cli")]
pub mod cli;
pub mod schema_gen;

pub use codegen::{generate, Codegen, CodegenError, CodegenReport};
