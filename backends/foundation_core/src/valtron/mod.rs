mod branches;
#[cfg(feature = "std")]
mod ctx;
mod doc;
mod drain;
mod executors;
mod extensions;
mod funcs;
mod iterators;
mod notifiers;
mod pipe;
mod pipe_mapped;
mod stream_future;
mod streams;
mod task;
mod types;
mod wake;

// modules exported as is
pub mod delayed_iterators;
pub mod multi_iterator;

// The engine entry-point macros (#[valtron] / #[valtron_test]) — defined in
// foundation_macros, re-exported here so users reach everything valtron from one
// path: `use foundation_core::valtron::{valtron_test, initialize_pool, spawn};`.
// Their expansions call `foundation_core::valtron::initialize_pool` and hold the
// returned PoolGuard until the wrapped fn exits (tokio-main style).
pub use foundation_macros::{task, timeout, valtron, valtron_test};

/// Default `shutdown_grace` for `#[valtron_test(timeout = ..)]`: after a test
/// times out, how long its pool gets for the workers to stop before the stuck
/// ones are detached (`PoolGuard::shutdown_within`) and the next test can take
/// the pool. 5 s: a healthy pool tears down in milliseconds, so this only ever
/// runs out for workers that are truly stuck.
/// Override per test with `#[valtron_test(timeout = .., shutdown_grace = ms)]`.
pub const DEFAULT_TEST_SHUTDOWN_GRACE: core::time::Duration = core::time::Duration::from_secs(5);

pub use branches::*;
#[cfg(feature = "std")]
pub use ctx::*;
pub use drain::*;
pub use executors::*;
pub use extensions::*;
pub use funcs::*;
pub use iterators::*;
pub use notifiers::*;
pub use pipe::*;
pub use pipe_mapped::*;
pub use stream_future::*;
pub use streams::*;
pub use task::*;
pub use types::*;
pub use wake::*;

/// `true` where valtron can block the calling thread (`.block()`,
/// `.blocking()`, [`block_on_future`], [`run_stream_iter`],
/// [`sync_collect_one`]): native, and wasm hosts that are not JS. `false` on
/// JS-hosted wasm (`js-wasmbindgen` / `js-foundation-wasm`), where blocking
/// would freeze the isolate's only thread.
///
/// Evaluated in this crate, so it reflects foundation_core's own features
/// after Cargo's feature unification — which a dependent crate cannot see
/// with its own `cfg`. Use [`blocking_boundary!`](crate::blocking_boundary)
/// to compile items only where blocking exists.
pub const BLOCKING_BOUNDARY: bool = !cfg!(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
));

/// Compile items only where valtron's blocking sync boundary exists
/// ([`BLOCKING_BOUNDARY`] is `true`).
///
/// WHY: `.block()` / `.blocking()` are absent on JS-hosted wasm, and the
/// choice is made by foundation_core's features, not the dependent crate's.
/// A sync entry point that waits on async code (a `main`, an OS thread)
/// imports its blocking helpers inside this macro so they exist exactly
/// where valtron can block.
///
/// ```ignore
/// foundation_core::blocking_boundary! {
///     use foundation_core::valtron::block_on_future;
/// }
/// // Or with an `else` arm for the JS-hosted wasm build:
/// foundation_core::blocking_boundary! {
///     if { pub const CAN_BLOCK: bool = true; }
///     else { pub const CAN_BLOCK: bool = false; }
/// }
/// ```
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
#[macro_export]
macro_rules! blocking_boundary {
    (if { $($yes:tt)* } else { $($no:tt)* }) => { $($yes)* };
    ($($yes:tt)*) => { $($yes)* };
}

/// Compile items only where valtron's blocking sync boundary exists — this is
/// the JS-hosted wasm build, where it does not: the items are dropped (or the
/// `else` arm is used). See the native definition for details.
#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
#[macro_export]
macro_rules! blocking_boundary {
    (if { $($yes:tt)* } else { $($no:tt)* }) => { $($no)* };
    ($($yes:tt)*) => {};
}

// Compile-time check that `blocking_boundary!` and the blocking helpers use
// the same gate: wherever the macro keeps its items, `.block()`,
// `.blocking()` and `sync_collect_one` must exist. CI builds this natively and
// for every wasm32 feature set of foundation_core in ci/packages.toml (plain
// `std`, `js-foundation-wasm`, `js-wasmbindgen`), so a drift between the two
// gates fails the build.
#[cfg(feature = "std")]
mod blocking_boundary_probe {
    use super::BLOCKING_BOUNDARY;

    crate::blocking_boundary! {
        if {
            use core::future::Future;

            use super::{from_future, sync_collect_one, BlockingFutureExt, BlockingStreamExt};

            #[allow(dead_code)]
            fn probe<F, S>(future: F, stream: S)
            where
                F: Future + Send + 'static,
                F::Output: Send + 'static,
                S: futures_core::Stream + Send + 'static,
                S::Item: Send + 'static,
            {
                let _ = sync_collect_one(from_future(async {}));
                let _ = future.block();
                let _ = stream.blocking();
            }

            const _: () = assert!(BLOCKING_BOUNDARY);
        } else {
            const _: () = assert!(!BLOCKING_BOUNDARY);
        }
    }
}

/// Support for valtron's macros. Not public API.
#[doc(hidden)]
pub mod __private {
    pub use super::executors::block_on_future_unchecked as block_on_future;
}

// `futures_lite::StreamExt` — `.next().await`, `.collect::<Vec<_>>().await`,
// `.for_each(..).await`, … on the `futures_core::Stream` bridges returned by
// `into_next_stream()`/`into_future_stream()`. Re-exported so a consumer needs no
// separate `futures`/`futures-lite` dependency to drain a valtron stream.
#[cfg(feature = "std")]
pub use futures_lite::StreamExt;

// Re-export foundation_compact::trace so #[valtron_test] can reference
// foundation_core::valtron::trace::try_init_tracing_with() without every
// crate adding foundation_compact as a direct dependency.
pub use foundation_compact::trace;
