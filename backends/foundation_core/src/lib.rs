//! Foundation types and the valtron task engine.
//!
//! # Feature sets
//!
//! The default features enable `standard`, `std`, `alloc` and `valtron`. The
//! `valtron` feature turns on the task engine: the executors,
//! [`valtron::Ctx`](valtron::Ctx) and the `#[valtron]` macros. `Ctx` needs
//! `std` (`HashMap`, `Arc`, `TypeId`, `dyn Any` and a monotonic clock), so
//! `valtron` implies `std`. `alloc`-only support is deferred, so `std` is the
//! minimum feature set for the crate. A bare `--no-default-features` build is
//! refused with an error that names this minimum.
//!
//! Without `valtron`, the crate keeps the runtime-neutral modules, including
//! [`durable`]. Build them with
//! `default-features = false, features = ["std", "alloc"]`.

// The `multi` feature is additive on every target. Its executor is a native
// thread pool, so the native-only parts (the idle-wake handle) compile only for
// a non-wasm target. Cargo merges features across the whole graph, so a wasm
// build whose dependencies leave `multi` on must still compile. The default
// netio wasm check in `ci/packages.toml` guards that (issue #1030).

// The feature set is the contract, not the target. `valtron::Ctx` and the
// executors that pass it need `std`; `alloc`-only support is deferred (see
// `valtron::ctx`). A bare `--no-default-features` build has neither and would
// fail with unresolved-import errors deep in the executors. Name the minimum
// here instead.
#[cfg(not(feature = "std"))]
compile_error!(
    "foundation_core needs the `std` feature: `valtron::Ctx` and its executors require `std`, \
     and `alloc`-only support is deferred. Enable the default features or the `std` feature."
);

pub mod compati;
/// The Durable Streams protocol core. It has no valtron, HTTP or socket dependency.
pub mod durable;
/// The host builders for the effect environment (spec 84).
///
/// The timer is valtron's sleep, so this module needs the `valtron` feature.
#[cfg(feature = "valtron")]
pub mod env;
pub mod extensions;
pub mod io;
pub mod macros;
pub mod retries;
pub mod synca;
pub mod trace;
pub mod traits;
pub mod type_uuid;
pub mod url;
// The executors and `valtron::Ctx` are the `valtron` feature. The
// `compile_error!` above already refuses a bare build; without this gate its
// executors would add unresolved-import noise for `Ctx` on top of that one
// actionable message.
#[cfg(feature = "valtron")]
pub mod valtron;

/// `#[wasm_logs(stdout | console)]` — install a tracing subscriber (console.log
/// on wasm, stdout on native) at the top of a function body. See
/// `foundation_macros::wasm_logs`.
pub use foundation_macros::wasm_logs;
