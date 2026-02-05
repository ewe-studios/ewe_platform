#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64"),
    not(feature = "multi")
))]
mod nowasm_single;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64"),
    not(feature = "multi")
))]
pub use nowasm_single::{RunOnceWrapper, RunOnceWrapperBuilder};

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64"),
    feature = "multi"
))]
mod nowasm_multi;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64"),
    feature = "multi"
))]
pub use nowasm_multi::{RunOnceWrapper, RunOnceWrapperBuilder};

#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    not(feature = "multi")
))]
pub mod wasm;

#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    not(feature = "multi")
))]
pub use wasm::{RunOnceWrapper, RunOnceWrapperBuilder};
