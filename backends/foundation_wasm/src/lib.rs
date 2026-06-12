#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![no_std]

// Build tooling is a STD, native-only concern (feature 20) — target-gated so
// every native build carries the CLI while wasm builds stay no_std-clean.
#[cfg(not(target_arch = "wasm32"))]
extern crate std;

#[cfg(not(target_arch = "wasm32"))]
pub mod build_tools;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

extern crate alloc;

mod base;
mod error;
mod frames;
mod intervals;
mod host_runtime;
mod mem;
mod ops;
mod protocol;
mod registry;
mod schedule;
mod wrapped;

#[cfg(feature = "web")]
pub mod testing;

#[cfg(feature = "embedded-js")]
pub mod embedded;

pub use base::*;
pub use error::*;
pub use frames::*;
pub use intervals::*;
pub use host_runtime::*;
pub use mem::*;
pub use ops::*;
pub use protocol::*;
pub use registry::*;
pub use schedule::*;
pub use wrapped::*;

// Re-export the raw-parts helper so dependent crates (e.g. foundation_wasm_ui)
// can hand param buffers to their own host FFI the same way this crate does.
pub use foundation_nostd::raw_parts;
