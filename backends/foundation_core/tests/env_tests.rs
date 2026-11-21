//! Tests for the effect environment of `foundation_core::env` (spec 84 T2).
//!
//! These tests pin the host builders, the `Ctx` accessors and replay from one
//! seed. They need no executor: the timer is checked in the executor tests.

use std::sync::Arc;

use foundation_compact::{Duration, Instant, SystemTime, UNIX_EPOCH};
use foundation_context::env::{CounterSource, Env, EnvError, Timer, TimerFuture};
use foundation_context::{Clock, Ctx};
use foundation_core::env;

/// A clock that never moves, so a replay is exact without an executor.
struct FixedClock {
    now: Instant,
    wall: SystemTime,
}

impl FixedClock {
    fn new() -> Self {
        Self {
            now: Instant::now(),
            wall: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Instant {
        self.now
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wall_now(&self) -> SystemTime {
        self.wall
    }
}

/// A timer that resolves at once, for tests that never wait.
struct ReadyTimer;

impl Timer for ReadyTimer {
    fn sleep_until(&self, _ctx: Ctx, _deadline: Instant) -> TimerFuture {
        Box::pin(async {})
    }
}

fn fixed_ctx() -> Ctx {
    Ctx::fixed(Arc::new(FixedClock::new()))
}

#[test]
fn install_stores_and_returns_the_env() {
    let ctx = fixed_ctx();
    assert!(ctx.env().is_none());

    let env = env::simulated(&ctx, 7).expect("a seeded env never fails");
    ctx.store(env.clone());

    let stored = ctx.require_env().expect("the host installed the Env");
    assert!(Arc::ptr_eq(&env.ids(), &stored.ids()));
    assert!(Arc::ptr_eq(&env.entropy(), &stored.entropy()));
    assert!(Arc::ptr_eq(&env.timer(), &stored.timer()));
}

#[test]
fn install_helper_returns_the_same_env() {
    let ctx = fixed_ctx();
    let installed = env::install(&ctx, |ctx| env::simulated(ctx, 5)).expect("seeded");

    let stored = ctx.require_env().expect("installed");
    assert!(Arc::ptr_eq(&installed.ids(), &stored.ids()));
}

#[test]
fn missing_env_is_a_typed_error() {
    let ctx = fixed_ctx();
    assert_eq!(ctx.require_env().expect_err("no Env"), EnvError::Missing);
}

#[test]
fn simulated_replay() {
    let trace = || {
        let ctx = fixed_ctx();
        let env = env::simulated(&ctx, 7).expect("seeded");
        let ids = env.ids();
        let clock = ctx.clock();
        let mut values = Vec::new();
        let mut deadlines = Vec::new();
        for _ in 0..100 {
            values.push(ids.next_id().to_u128());
            let deadline = clock.now() + Duration::from_secs(5);
            // A run compares its deadlines as offsets from its own start: two
            // runs create their clocks at different wall moments.
            deadlines.push(deadline.saturating_duration_since(clock.now()));
        }
        (values, deadlines)
    };

    let first = trace();
    let second = trace();
    assert_eq!(first, second, "one seed replays the run exactly");
}

#[test]
fn simulated_env_uses_the_ctx_clock() {
    let ctx = fixed_ctx();
    let env = env::simulated(&ctx, 9).expect("seeded");
    let id = env.ids().next_id();
    let expected = foundation_compact::time::unix_millis(ctx.clock().wall_now());
    assert_eq!(i64::try_from(id.timestamp()).expect("fits"), expected);
}

#[test]
fn with_helpers_keep_the_other_members() {
    let ctx = fixed_ctx();
    let env = env::simulated(&ctx, 3).expect("seeded");
    let ids = env.ids();
    // One draw from the original source, then the same source through the copy.
    let first = ids.next_id();
    let second = ids.next_id();
    assert!(first < second);

    let with_counter = env.clone().with_ids(Arc::new(CounterSource::new(10)));
    assert_eq!(with_counter.ids().next_id().to_u128(), 10);
    assert_eq!(with_counter.ids().next_id().to_u128(), 11);

    let with_timer = env.with_timer(Arc::new(ReadyTimer));
    let mut bytes = [0u8; 8];
    with_timer
        .entropy()
        .fill(&mut bytes)
        .expect("a seeded source never fails");
}

#[test]
fn env_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Env>();
    assert_send_sync::<Arc<Env>>();
}
