//! Contract tests for the effect traits and their implementations (spec 84 T1).
//!
//! These tests need no executor. They pin the seeded entropy, the counter and
//! scru128 ID sources, and the absence of an `Arc` cycle between `Ctx` and
//! `Env`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_compact::{Duration, Instant, SystemTime, UNIX_EPOCH};
use foundation_context::env::{
    CounterSource, EntropySource, Env, EnvError, IdSource, OsEntropy, Scru128Source, SeededEntropy,
    Timer, TimerFuture,
};
use foundation_context::{Clock, Ctx, MonotonicClock};

/// Compile-time assertion that a type is `Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

/// A clock that never moves, so the scru128 generator sees one timestamp.
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

/// Records that it was dropped, to prove the `Env` map is freed.
struct DropProbe(Arc<AtomicBool>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A timer that owns a drop probe.
struct ProbeTimer {
    _probe: DropProbe,
}

impl Timer for ProbeTimer {
    fn sleep_until(&self, _ctx: Ctx, _deadline: Instant) -> TimerFuture {
        Box::pin(async {})
    }
}

#[test]
fn env_effect_types_are_send_and_sync() {
    assert_send_sync::<OsEntropy>();
    assert_send_sync::<SeededEntropy>();
    assert_send_sync::<CounterSource>();
    assert_send_sync::<Scru128Source>();
    assert_send_sync::<Env>();
    assert_send_sync::<Arc<dyn EntropySource>>();
    assert_send_sync::<Arc<dyn IdSource>>();
    assert_send_sync::<Arc<dyn Timer>>();
}

#[test]
fn counter_ids() {
    let ids = CounterSource::new(1);
    assert_eq!(ids.next_id().to_u128(), 1);
    assert_eq!(ids.next_id().to_u128(), 2);
    assert_eq!(ids.next_id().to_u128(), 3);
}

#[test]
fn seeded_entropy_repeats() {
    let first = SeededEntropy::new(7);
    let second = SeededEntropy::new(7);

    let mut a = [0u8; 64];
    let mut b = [0u8; 64];
    first.fill(&mut a).expect("a seeded source never fails");
    second.fill(&mut b).expect("a seeded source never fails");
    assert_eq!(a, b);
    assert_eq!(first.seed(), 7);

    // Two draws from one source continue the stream and do not repeat.
    let mut c = [0u8; 64];
    first.fill(&mut c).expect("a seeded source never fails");
    assert_ne!(a, c);

    // A different seed gives different bytes.
    let other = SeededEntropy::new(8);
    let mut d = [0u8; 64];
    other.fill(&mut d).expect("a seeded source never fails");
    assert_ne!(a, d);
}

#[test]
fn ids_are_sortable() {
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new());
    let entropy = SeededEntropy::new(11);
    let source = Scru128Source::new(clock, &entropy).expect("seeded entropy never fails");

    let ids: Vec<_> = (0..1_000).map(|_| source.next_id()).collect();
    for pair in ids.windows(2) {
        assert!(pair[0] < pair[1], "ids must be strictly increasing");
    }
}

#[test]
fn seeded_ids_repeat() {
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new());

    let run = || {
        let entropy = SeededEntropy::new(21);
        let source = Scru128Source::new(Arc::clone(&clock), &entropy).expect("seeded");
        (0..100).map(|_| source.next_id()).collect::<Vec<_>>()
    };

    assert_eq!(run(), run());
}

#[test]
fn missing_env_is_typed() {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    let error = ctx.require_env().expect_err("the context holds no Env");
    assert_eq!(error, EnvError::Missing);
    assert!(
        error.to_string().contains("the context holds no Env"),
        "unexpected text: {error}"
    );
    assert!(ctx.env().is_none());
}

#[test]
fn env_does_not_hold_ctx() {
    let dropped = Arc::new(AtomicBool::new(false));
    let env = Env::new(
        Arc::new(ProbeTimer {
            _probe: DropProbe(Arc::clone(&dropped)),
        }),
        Arc::new(SeededEntropy::new(3)),
        Arc::new(CounterSource::new(1)),
    );

    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    ctx.store(env);
    assert_eq!(ctx.strong_count(), 1, "storing the Env copies no Ctx");

    let live = ctx.require_env().expect("the Env is installed");
    assert_eq!(
        ctx.strong_count(),
        1,
        "the Env must not hold the Ctx (no Arc cycle)"
    );
    drop(live);

    assert!(!dropped.load(Ordering::SeqCst));
    drop(ctx);
    assert!(
        dropped.load(Ordering::SeqCst),
        "dropping the last Ctx frees the map and the Env"
    );
}

#[test]
fn env_builders_replace_one_member() {
    let timer = Arc::new(ReadyTimer);
    let entropy = Arc::new(SeededEntropy::new(4));
    let ids = Arc::new(CounterSource::new(5));
    let env = Env::new(timer, entropy, ids);

    let ids = env.ids();
    assert_eq!(ids.next_id().to_u128(), 5);

    let first = Arc::new(SeededEntropy::new(6));
    let other = env.with_entropy(first);
    let mut bytes = [0u8; 8];
    other.entropy().fill(&mut bytes).expect("seeded");
    assert_eq!(
        other.ids().next_id().to_u128(),
        6,
        "with_entropy keeps the shared ID source"
    );
}

#[test]
fn env_debug_does_not_expose_members() {
    let env = Env::new(
        Arc::new(ReadyTimer),
        Arc::new(SeededEntropy::new(1)),
        Arc::new(CounterSource::new(1)),
    );
    assert_eq!(format!("{env:?}"), "Env { .. }");
}
