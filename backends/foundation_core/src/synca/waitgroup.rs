//! WaitGroup - Thread completion tracking primitive.
//!
//! WHY: Provides a counting barrier for tracking N outstanding work items.
//!
//! WHAT: `WaitGroup` blocks until count reaches 0. `WaitGroupGuard` is a
//! RAII guard that calls `done()` on drop, ensuring cleanup even on panic.
//!
//! HOW: Uses `LockSignal` (CondVar-based) for blocking. Each worker gets
//! a guard that decrements on drop.

use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};

use crate::synca::LockSignal;

/// Tracks N outstanding work items. `wait()` blocks until count reaches 0.
///
/// Uses the existing `LockSignal` (CondVar-based) for the blocking mechanism.
/// Each worker thread gets a `WaitGroupGuard` that calls `done()` on drop,
/// ensuring cleanup even if the thread panics.
///
/// # Examples
///
/// ```
/// use foundation_core::synca::WaitGroup;
/// use std::sync::Arc;
///
/// let wg = WaitGroup::new();
/// wg.add(3); // 3 workers
///
/// for i in 0..3 {
///     let wg = wg.clone();
///     std::thread::spawn(move || {
///         let _guard = wg.guard(); // done() called on drop
///         // do work
///     });
/// }
///
/// wg.wait(); // block until all workers done
/// ```
#[derive(Clone)]
pub struct WaitGroup {
    count: Arc<AtomicUsize>,
    signal: Arc<LockSignal>,
}

impl WaitGroup {
    /// Create a new `WaitGroup` with count 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            signal: Arc::new(LockSignal::new()),
        }
    }

    /// Increment the counter by n.
    pub fn add(&self, n: usize) {
        self.count.fetch_add(n, Ordering::SeqCst);
    }

    /// Decrement the counter. If it reaches 0, signal all waiters.
    pub fn done(&self) {
        let prev = self.count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // count just reached 0
            self.signal.signal_all();
        }
    }

    /// Block until count reaches 0.
    pub fn wait(&self) {
        loop {
            if self.count.load(Ordering::SeqCst) == 0 {
                return;
            }
            self.signal.lock_and_wait();
        }
    }

    /// Create a RAII guard that calls `done()` on drop.
    #[must_use]
    pub fn guard(&self) -> WaitGroupGuard {
        WaitGroupGuard(self.clone())
    }
}

impl Default for WaitGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Calls `WaitGroup::done()` on drop — ensures threads that panic still decrement.
pub struct WaitGroupGuard(WaitGroup);

impl Drop for WaitGroupGuard {
    fn drop(&mut self) {
        self.0.done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread, time::Duration};

    #[test]
    fn test_waitgroup_basic() {
        let wg = WaitGroup::new();
        wg.add(3);

        for _ in 0..3 {
            let wg = wg.clone();
            thread::spawn(move || {
                let _guard = wg.guard();
                thread::sleep(Duration::from_millis(10));
            });
        }

        wg.wait(); // Should block until all threads complete
    }

    #[test]
    fn test_waitgroup_zero() {
        let wg = WaitGroup::new();
        // Should return immediately when count is 0
        wg.wait();
    }

    #[test]
    fn test_waitgroup_add_done() {
        let wg = WaitGroup::new();
        wg.add(2);
        wg.done();
        wg.done();
        // Count should be 0 now
        wg.wait();
    }

    #[test]
    fn test_waitgroup_guard_drop() {
        let wg = WaitGroup::new();
        wg.add(1);

        {
            let _guard = wg.guard();
            // Guard created, count still 1
        }
        // Guard dropped, done() called, count should be 0

        wg.wait(); // Should return immediately
    }

    #[test]
    fn test_waitgroup_concurrent() {
        let wg = WaitGroup::new();
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));

        for i in 0..10 {
            let wg = wg.clone();
            let results = Arc::clone(&results);
            wg.add(1);
            thread::spawn(move || {
                let _guard = wg.guard();
                thread::sleep(Duration::from_millis(i as u64));
                results.lock().unwrap().push(i);
            });
        }

        wg.wait();

        let results = results.lock().unwrap();
        assert_eq!(results.len(), 10);
    }
}
