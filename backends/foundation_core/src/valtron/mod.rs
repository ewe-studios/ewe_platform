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
