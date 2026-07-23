// wasm32 memory ops inherently require narrowing casts — these are correct
// and unavoidable for the wasm ABI.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![no_std]

// std is available on native, emscripten (partial libc), and WASI (full).
// Only wasm32-unknown-unknown is truly no_std (no libc, no OS).
#[cfg(any(
    not(target_family = "wasm"),
    target_os = "emscripten",
    target_os = "wasi"
))]
extern crate std;

// Build tooling is a native-only concern — the CLI scanner + codegen.
#[cfg(not(target_family = "wasm"))]
pub mod build_tools;
#[cfg(not(target_family = "wasm"))]
pub mod cli;

extern crate alloc;

mod base;
mod error;
mod frames;
mod host_runtime;
mod intervals;
mod mem;
mod ops;
mod protocol;
mod registry;
mod schedule;
mod trigger;
mod wrapped;

pub mod ipc;
pub mod stream;

// F41 backward compat: re-exports old capability names from ipc.
mod capability;

#[cfg(feature = "web")]
pub mod testing;

#[cfg(feature = "wasi")]
pub mod wasi_host;

#[cfg(feature = "embedded-js")]
pub mod embedded;

pub use base::*;
pub use capability::*;
pub use error::*;
pub use frames::*;
pub use host_runtime::*;
pub use intervals::*;
pub use mem::*;
pub use ops::*;
pub use protocol::*;
pub use registry::*;
pub use schedule::*;
pub use stream::*;
pub use trigger::*;
pub use wrapped::*;

// Re-export the raw-parts helper so dependent crates (e.g. foundation_wasm_ui)
// can hand param buffers to their own host FFI the same way this crate does.
pub use foundation_nostd::raw_parts;
