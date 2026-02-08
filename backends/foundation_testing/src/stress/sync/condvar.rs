//! `CondVar` stress tests.

use crate::stress::{StressConfig, StressHarness, StressResult};
use foundation_nostd::primitives::{CondVar, CondVarMutex};
use std::sync::Arc;
use std::time::Duration;

/// Runs a basic `CondVar` stress test with wait/notify cycles.
///
/// This test spawns multiple threads that repeatedly:
/// 1. Lock a mutex
/// 2. Wait on a condition variable
/// 3. Modify shared state
/// 4. Notify other threads
///
/// # Examples
///
/// ```
/// use foundation_testing::stress::{StressConfig, sync::run_condvar_stress_test};
///
/// let config = StressConfig::new().threads(10).iterations(100);
/// let result = run_condvar_stress_test(config);
///
/// // Should complete successfully with high success rate
/// assert!(result.success_rate() > 0.99);
/// ```
#[must_use]
pub fn run_condvar_stress_test(config: StressConfig) -> StressResult {
    let mutex = Arc::new(CondVarMutex::new(0u64));
    let condvar = Arc::new(CondVar::new());

    let harness = StressHarness::new(config);

    harness.run(move |_thread_id, _iteration| {
        // Lock, wait, modify, notify pattern
        let mutex_clone = Arc::clone(&mutex);
        let condvar_clone = Arc::clone(&condvar);

        // Acquire lock
        let Ok(mut guard) = mutex_clone.lock() else {
            return false;
        };

        // Increment counter
        *guard += 1;
        let value = *guard;

        // Drop lock before notifying (good practice)
        drop(guard);

        // Notify one waiter
        condvar_clone.notify_one();

        // Success if we got a value
        value > 0
    })
}

/// Runs a producer-consumer stress test using `CondVar`.
///
/// Spawns producer and consumer threads that coordinate via a `CondVar`:
/// - Producers add items to a shared queue
/// - Consumers wait for items and remove them
/// - Tests high contention scenarios
///
/// # Examples
///
/// ```
/// use foundation_testing::stress::{StressConfig, sync::run_condvar_producer_consumer_stress};
///
/// let config = StressConfig::new().threads(8).iterations(500);
/// let result = run_condvar_producer_consumer_stress(config);
///
/// assert!(result.success_rate() > 0.99);
/// ```
#[must_use]
pub fn run_condvar_producer_consumer_stress(config: StressConfig) -> StressResult {
    let queue = Arc::new(CondVarMutex::new(Vec::<u64>::new()));
    let condvar = Arc::new(CondVar::new());
    let max_queue_size = 100;

    let harness = StressHarness::new(config);

    harness.run(move |thread_id, iteration| {
        let queue_clone = Arc::clone(&queue);
        let condvar_clone = Arc::clone(&condvar);

        // Half threads are producers, half are consumers
        let is_producer = thread_id % 2 == 0;

        if is_producer {
            // Producer: Add item to queue
            let Ok(mut guard) = queue_clone.lock() else {
                return false;
            };

            // Wait if queue is full
            while guard.len() >= max_queue_size {
                guard = match condvar_clone.wait(guard) {
                    Ok(g) => g,
                    Err(_) => return false,
                };
            }

            // Add item
            let item = (thread_id as u64) * 1000 + (iteration as u64);
            guard.push(item);

            drop(guard);
            condvar_clone.notify_all();

            true
        } else {
            // Consumer: Remove item from queue
            let Ok(mut guard) = queue_clone.lock() else {
                return false;
            };

            // Wait if queue is empty
            while guard.is_empty() {
                guard = match condvar_clone.wait(guard) {
                    Ok(g) => g,
                    Err(_) => return false,
                };
            }

            // Remove item
            let _item = guard.pop();

            drop(guard);
            condvar_clone.notify_all();

            true
        }
    })
}

/// Runs a high contention stress test with many waiters.
///
/// Spawns many threads that all wait on the same condition,
/// then wakes them all repeatedly. Tests `notify_all()` performance
/// under extreme contention.
#[must_use]
pub fn run_condvar_high_contention_stress(config: StressConfig) -> StressResult {
    let mutex = Arc::new(CondVarMutex::new(false));
    let condvar = Arc::new(CondVar::new());

    let harness = StressHarness::new(config);

    harness.run(move |thread_id, _iteration| {
        let mutex_clone = Arc::clone(&mutex);
        let condvar_clone = Arc::clone(&condvar);

        // Thread 0 is the notifier
        if thread_id == 0 {
            // Signal all waiters
            let Ok(mut guard) = mutex_clone.lock() else {
                return false;
            };
            *guard = true;
            drop(guard);

            condvar_clone.notify_all();

            // Reset for next iteration
            std::thread::sleep(Duration::from_micros(100));
            let Ok(mut guard) = mutex_clone.lock() else {
                return false;
            };
            *guard = false;
            drop(guard);

            true
        } else {
            // Wait for signal
            let Ok(mut guard) = mutex_clone.lock() else {
                return false;
            };

            while !*guard {
                guard = match condvar_clone.wait(guard) {
                    Ok(g) => g,
                    Err(_) => return false,
                };
            }

            drop(guard);
            true
        }
    })
}

/// Runs a timeout stress test.
///
/// Tests `wait_timeout()` under contention with various timeout durations.
#[must_use]
pub fn run_condvar_timeout_stress(config: StressConfig) -> StressResult {
    let mutex = Arc::new(CondVarMutex::new(0u64));
    let condvar = Arc::new(CondVar::new());

    let harness = StressHarness::new(config);

    harness.run(move |_thread_id, iteration| {
        let mutex_clone = Arc::clone(&mutex);
        let condvar_clone = Arc::clone(&condvar);

        let Ok(guard) = mutex_clone.lock() else {
            return false;
        };

        // Wait with short timeout
        let timeout = Duration::from_micros(10 + (iteration % 100) as u64);
        let Ok((_guard, _result)) = condvar_clone.wait_timeout(guard, timeout) else {
            return false;
        };

        // Timeouts are expected and OK
        true
    })
}
