//! The one valtron pool of the test binary.
//!
//! Every test module shares one pool. `initialize_pool` blocks until the
//! previous pool guard drops, and the test binary keeps its guard for the life
//! of the process, so a second call from another module never returns.

use std::future::Future;
use std::sync::OnceLock;

/// Starts the shared pool one time.
pub fn pool() {
    static POOL: OnceLock<()> = OnceLock::new();
    POOL.get_or_init(|| {
        std::mem::forget(foundation_core::valtron::initialize_pool(7, Some(8)));
    });
}

/// Drives a future to its result on the test thread.
pub fn block_on<F: Future>(future: F) -> F::Output {
    pool();
    futures::executor::block_on(future)
}
