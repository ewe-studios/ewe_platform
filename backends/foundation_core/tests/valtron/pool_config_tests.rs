#![cfg(feature = "multi")]

//! Feature 09 "limits-as-config": the `PoolConfig` yield and back-off knobs.
//!
//! The worker re-polls an idle `Wait` task at most every `max_yield_duration`
//! instead of sleeping the full `yield_wait_time` (default 4 s). This test
//! proves the setting reaches the worker by timing the re-poll cadence.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::multi::{initialize_pool_with_config, spawn, PoolConfig};
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};

/// A task that records each poll and always parks as `Wait` (no deadline).
/// The worker must wake it again once `max_yield_duration` elapses.
struct IdlePollTask {
    polls: Arc<Mutex<Vec<Instant>>>,
}

impl TaskIterator for IdlePollTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.lock().unwrap().push(Instant::now());
        Some(TaskStatus::Wait)
    }
}

fn median_ms(mut vals: Vec<f64>) -> f64 {
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = vals.len();
    match n {
        0 => 0.0,
        _ if n % 2 == 1 => vals[n / 2],
        _ => (vals[n / 2 - 1] + vals[n / 2]) / 2.0,
    }
}

/// A pool built with a custom `max_yield_duration` uses it. An idle `Wait`
/// task is re-polled every ~20 ms, not every `yield_wait_time` (4 s default).
#[test]
fn pool_config_overrides_yield_and_backoff() {
    let config = PoolConfig::default().with_max_yield_duration(Duration::from_millis(20));
    let _guard = initialize_pool_with_config(42, Some(3), config);

    let polls = Arc::new(Mutex::new(Vec::new()));
    spawn()
        .with_task(IdlePollTask {
            polls: polls.clone(),
        })
        .schedule()
        .expect("schedule idle task");

    std::thread::sleep(Duration::from_millis(300));

    let polls = polls.lock().unwrap();
    assert!(
        polls.len() >= 3,
        "idle task should be re-polled several times, got {} polls",
        polls.len()
    );

    // Drop the first interval: it can include the worker's initial pickup.
    let intervals: Vec<f64> = polls
        .windows(2)
        .skip(1)
        .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
        .collect();

    let median = median_ms(intervals);
    assert!(
        (10.0..=30.0).contains(&median),
        "idle re-check median should be ~20 ms (10..30 ms), got {median:.1} ms",
    );
}

/// `PoolConfig` defaults match the documented values Feature 09 moved out of
/// hard-coded constants.
#[test]
fn pool_config_defaults_match_documented_values() {
    let c = PoolConfig::default();
    assert_eq!(c.background_shutdown_timeout, Duration::from_secs(30));
    assert_eq!(c.max_yield_duration, Duration::from_millis(100));
    assert_eq!(c.yield_wait_time, Duration::from_millis(4000));
    assert_eq!(c.back_off_thread_factor, 6);
    assert_eq!(c.back_off_jitter, 0.75);
    assert_eq!(c.back_off_min_duration, Duration::from_millis(1));
    assert_eq!(c.back_off_max_duration, Duration::from_millis(1000));
}
