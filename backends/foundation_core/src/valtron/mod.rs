mod branches;
mod drain;
mod executors;
mod extensions;
mod funcs;
mod iterators;
mod notifiers;
mod pipe;
mod stream_future;
mod streams;
mod task;
mod types;

// modules exported as is
pub mod delayed_iterators;
pub mod multi_iterator;

// The engine entry-point macros (#[valtron] / #[valtron_test]) — defined in
// foundation_macros, re-exported here so users reach everything valtron from one
// path: `use foundation_core::valtron::{valtron_test, initialize_pool, spawn};`.
// Their expansions call `foundation_core::valtron::initialize_pool` and hold the
// returned PoolGuard until the wrapped fn exits (tokio-main style).
pub use foundation_macros::{timeout, valtron, valtron_test};

pub use branches::*;
pub use drain::*;
pub use executors::*;
pub use extensions::*;
pub use funcs::*;
pub use iterators::*;
pub use notifiers::*;
pub use pipe::*;
pub use stream_future::*;
pub use streams::*;
pub use task::*;
pub use types::*;
