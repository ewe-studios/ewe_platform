//! Feature 30 "remaining valtron limits become configuration": the new config
//! fields, their builders, and their documented defaults.
//!
//! For each item R1..R8 a test builds the config with a non-default value and
//! asserts the code path reads it back, mirroring
//! `pool_config_overrides_yield_and_backoff`. Where the field has no public
//! getter (or is `pub(crate)`), the test asserts the documented default and the
//! comment says why the path cannot be observed cheaply.

use std::time::Duration;

use foundation_core::valtron::{
    DEFAULT_DELIVERY_CAPACITY, DEFAULT_MAX_TURNS, DEFAULT_NOTIFY_QUEUE_MAX_SPINS,
    DEFAULT_PARK_DURATION, DEFAULT_READINESS_POLL_QUANTUM, DEFAULT_READINESS_WAIT,
    DEFAULT_WAIT_CHECK_INTERVAL, DEFAULT_WAIT_CYCLE, NotifyQueueConfig, StreamConfig,
};

// ---------------------------------------------------------------------------
// R1: `readiness_poll_quantum` on `LocalThreadExecutor`
// ---------------------------------------------------------------------------

/// The field drives the busy-spin guard inside `block_on` and has no getter, so
/// the code path is not observable cheaply. Test the documented default.
#[test]
fn readiness_poll_quantum_default_matches_documented_value() {
    assert_eq!(DEFAULT_READINESS_POLL_QUANTUM, Duration::from_millis(1));
}

// ---------------------------------------------------------------------------
// R4: `max_spins` on `NotifyQueueConfig`
// ---------------------------------------------------------------------------

/// A custom `max_spins` is stored on the config. The value is public, so the
/// builder round-trips. The spin count itself lives in a private `AtomicUsize`
/// on `NotifyQueue`, so the wait path is asserted through the config instead.
#[test]
fn notify_queue_config_max_spins_builder_round_trips() {
    let default = NotifyQueueConfig::default();
    assert_eq!(default.max_spins, DEFAULT_NOTIFY_QUEUE_MAX_SPINS);

    let custom = default.with_max_spins(3);
    assert_eq!(custom.max_spins, 3);
}

// ---------------------------------------------------------------------------
// R5: `readiness_wait` on `LocalThreadExecutor`
// ---------------------------------------------------------------------------

/// A custom readiness wait is stored on the executor state and read back
/// through `readiness_wait()`. This is the real code-path value: the getter
/// reads `Sleepers::max_readiness_wait`, not a copy kept on the builder.
#[test]
fn local_executor_readiness_wait_round_trips() {
    let executor = new_executor();
    assert_eq!(
        executor.readiness_wait(),
        DEFAULT_READINESS_WAIT,
        "default readiness wait"
    );

    let executor = executor.with_readiness_wait(Duration::from_millis(33));
    assert_eq!(executor.readiness_wait(), Duration::from_millis(33));
}

// ---------------------------------------------------------------------------
// R6: `delivery_capacity` on `TaskSpawnConfig`
// ---------------------------------------------------------------------------

/// The field is `pub(crate)` and is consumed by the `sequenced` delivery
/// terminal, which hides the queue capacity after dispatch. The code path is
/// not observable cheaply, so test the documented default.
#[test]
fn delivery_capacity_default_matches_documented_value() {
    assert_eq!(DEFAULT_DELIVERY_CAPACITY, 8);
}

// ---------------------------------------------------------------------------
// R7: `wait_check_interval` on `LocalThreadExecutor`
// ---------------------------------------------------------------------------

/// The field drives the JS event-loop yield in `block_on`'s `Wait` arms and has
/// no getter, so the code path is not observable cheaply. Test the documented
/// default.
#[test]
fn wait_check_interval_default_matches_documented_value() {
    assert_eq!(DEFAULT_WAIT_CHECK_INTERVAL, Duration::from_millis(4));
}

// ---------------------------------------------------------------------------
// R8: `wait_cycle` / `park_duration` on `StreamConfig`
// ---------------------------------------------------------------------------

/// The `StreamConfig` defaults name their sources, and the builders override
/// them. These fields are public, so the builders round-trip.
#[test]
fn stream_config_defaults_and_builders_round_trip() {
    let default = StreamConfig::default();
    assert_eq!(default.wait_cycle, DEFAULT_WAIT_CYCLE);
    assert_eq!(default.park_duration, DEFAULT_PARK_DURATION);
    assert_eq!(default.max_turns, DEFAULT_MAX_TURNS);

    let custom = StreamConfig::default()
        .with_wait_cycle(Duration::from_micros(777))
        .with_park_duration(Duration::from_millis(9))
        .with_max_turns(41);
    assert_eq!(custom.wait_cycle, Duration::from_micros(777));
    assert_eq!(custom.park_duration, Duration::from_millis(9));
    assert_eq!(custom.max_turns, 41);
}

// ---------------------------------------------------------------------------
// R2: `op_read_time` on `PoolConfig` (multi)
// ---------------------------------------------------------------------------

#[cfg(feature = "multi")]
mod multi_limits {
    use super::*;
    use foundation_core::valtron::multi::PoolConfig;
    use foundation_core::valtron::DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT;
    use foundation_core::valtron::DEFAULT_OP_READ_TIME;
    use foundation_core::valtron::ThreadedIterFutureConfig;

    /// The pool config stores `op_read_time`. The value is public, so the
    /// builder round-trips. The read path is `activity_receiver.recv_timeout`,
    /// which cannot be observed cheaply from an integration test.
    #[test]
    fn pool_config_op_read_time_round_trips() {
        let default = PoolConfig::default();
        assert_eq!(default.op_read_time, DEFAULT_OP_READ_TIME);

        let custom = default.with_op_read_time(Duration::from_millis(123));
        assert_eq!(custom.op_read_time, Duration::from_millis(123));
    }

    /// `ThreadedIterFutureConfig` stores `notify_queue_wait_timeout`. It is a
    /// public field (no dedicated builder); the code path is the consumer's
    /// `NotifyQueue::wait_for_item` timeout, which is not observable cheaply.
    #[test]
    fn threaded_iter_config_notify_queue_wait_timeout_round_trips() {
        let default = ThreadedIterFutureConfig::default();
        assert_eq!(default.notify_queue_wait_timeout, DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT);

        let custom = ThreadedIterFutureConfig {
            notify_queue_wait_timeout: Duration::from_millis(321),
            ..ThreadedIterFutureConfig::default()
        };
        assert_eq!(custom.notify_queue_wait_timeout, Duration::from_millis(321));
    }
}

// ---------------------------------------------------------------------------
// Boilerplate: a no-op process controller and a local executor factory, the
// same shape used by `builder_tests` and `local_executor`.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::retries::ExponentialBackoffDecider;
use foundation_core::synca::{IdleMan, SleepyMan};
use foundation_core::valtron::{
    LocalThreadExecutor, PriorityOrder, ProcessController, SharedTaskQueue,
};

#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _duration: Duration) {}
}

fn new_executor() -> LocalThreadExecutor<NoYielder> {
    let seed = fastrand::u64(..);
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));
    LocalThreadExecutor::from_seed(
        seed,
        "test".into(),
        global.clone(),
        IdleMan::new(3, None, SleepyMan::new(3, ExponentialBackoffDecider::default())),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
    )
}
