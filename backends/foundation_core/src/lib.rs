#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]
//! Foundation types and the valtron task engine.
//!
//! # Feature sets
//!
//! The default features enable `standard`, `std` and `alloc`. The valtron
//! executors pass [`valtron::Ctx`](valtron::Ctx), and `Ctx` needs `std`
//! (`HashMap`, `Arc`, `TypeId`, `dyn Any` and a monotonic clock). `alloc`-only
//! support is deferred, so `std` is the minimum feature set for the crate. A
//! bare `--no-default-features` build is refused with an error that names this
//! minimum.

// valtron's `multi` executor is a native thread pool: it does not compile for
// wasm32 (no threads, no idle-wake path). Cargo merges features across the whole
// graph, so one dependency that asks for `foundation_core/multi` (directly or
// through `foundation_netio/multi`, `foundation_db/multi`, ...) turns it on for
// every wasm build of the graph. Fail with the cause instead of deep inside the
// executor. Put the dependency that asks for `multi` in a
// `[target.'cfg(not(target_family = "wasm"))'.dependencies]` table.
#[cfg(all(feature = "multi", target_family = "wasm"))]
compile_error!(
    "foundation_core: the `multi` feature is native-only and cannot be enabled for wasm32. \
     Find who enables it with \
     `cargo tree -e features -i foundation_core --target wasm32-unknown-unknown`, \
     and move that dependency to a `cfg(not(target_family = \"wasm\"))` target table."
);

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
pub mod extensions;
pub mod io;
pub mod macros;
pub mod retries;
pub mod synca;
pub mod trace;
pub mod traits;
pub mod type_uuid;
pub mod url;
// The executors and `valtron::Ctx` need `std`, so this module compiles only
// when the feature is on. The `compile_error!` above already refuses the bare
// build; without this gate its executors would add unresolved-import noise for
// `Ctx` on top of that one actionable message.
#[cfg(feature = "std")]
pub mod valtron;

/// `#[wasm_logs(stdout | console)]` — install a tracing subscriber (console.log
/// on wasm, stdout on native) at the top of a function body. See
/// `foundation_macros::wasm_logs`.
pub use foundation_macros::wasm_logs;
