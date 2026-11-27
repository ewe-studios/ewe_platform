//! One valtron pool for each test thread, one test at a time.
//!
//! A test that waits on a simulated clock parks a pool worker until that clock
//! advances. If every test shared one pool, the parked workers of parallel
//! tests would starve the others and the test binary would hang (issue #933).
//! Each test thread therefore takes the one pool for itself: it starts the
//! pool on its first use, holds it until the test thread ends, and blocks the
//! next test until then. A serial run reuses one pool for all tests on the main
//! thread, which is also correct.

use std::cell::RefCell;
use std::future::Future;
use std::sync::{Mutex, MutexGuard, PoisonError};

use foundation_core::valtron::{initialize_pool, PoolGuard};

/// The gate that lets one test thread hold a pool at a time.
static POOL_GATE: Mutex<()> = Mutex::new(());

/// The pool of one test thread.
struct Lease {
    /// Dropped first, so the pool is gone before the next test starts one.
    _guard: PoolGuard,
    _gate: MutexGuard<'static, ()>,
}

thread_local! {
    static LEASE: RefCell<Option<Lease>> = const { RefCell::new(None) };
}

/// Starts this test thread's pool, or keeps the one it already has.
///
/// The call blocks while another test thread holds a pool.
pub fn pool() {
    LEASE.with(|lease| {
        if lease.borrow().is_none() {
            let gate = POOL_GATE.lock().unwrap_or_else(PoisonError::into_inner);
            let guard = initialize_pool(7, Some(8));
            *lease.borrow_mut() = Some(Lease {
                _guard: guard,
                _gate: gate,
            });
        }
    });
}

/// Releases this thread's pool, so a helper thread can take one.
///
/// A test that starts threads which drive their own futures calls this before
/// it joins them. Otherwise the helper threads would wait for a pool that the
/// test thread still holds.
pub fn release() {
    LEASE.with(|lease| {
        drop(lease.borrow_mut().take());
    });
}

/// Drives a future to its result on the test thread.
pub fn block_on<F: Future>(future: F) -> F::Output {
    pool();
    futures::executor::block_on(future)
}
