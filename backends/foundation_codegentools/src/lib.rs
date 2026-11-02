//! Code generation tooling for the ewe platform.

#[cfg(feature = "cli")]
pub mod cli;
/// The declaration a crate makes about what it generates — [`generate`], with
/// `check()` and `write()`. Callable from a `build.rs` (see the module docs for
/// which verb belongs there).
pub mod codegen;
pub mod schema_gen;

pub use codegen::{
    cloudflare_wire_types, generate, selection_from_file, Codegen, CodegenError, CodegenReport,
};
