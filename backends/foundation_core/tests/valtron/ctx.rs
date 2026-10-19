//! Integration tests for the engine context `Ctx` (feature 04-ctx).
//!
//! These pin the type-map contract (round-trip, replace, dedupe, wrong-type →
//! `None`), the `clone`-shares / `fork`-isolates split, and the reachability of the
//! three engine-seeded standard elements (clock, config, RNG). They are pure,
//! single-threaded unit tests of the `Ctx` value type — no pool, no executor.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_compact::rng::Rng;
use foundation_core::valtron::{Clock, Config, Ctx, MonotonicClock};

/// Compile-time assertion that a type is usable across threads (and on the
/// single-threaded engine). Fails to compile if it stops being `Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn ctx_is_send_and_sync() {
    assert_send_sync::<Ctx>();
    assert_send_sync::<MonotonicClock>();
    assert_send_sync::<Arc<dyn Clock>>();
    assert_send_sync::<Config>();
}

#[test]
fn empty_ctx_has_no_entries() {
    let ctx = Ctx::new();
    assert!(!ctx.contains::<u32>());
    assert!(ctx.get::<u32>().is_none());
    assert!(ctx.remove::<u32>().is_none());
}

#[test]
fn map_round_trip() {
    let ctx = Ctx::new();
    assert!(ctx.insert(Arc::new(42u32)).is_none());

    let got = ctx.get::<u32>().expect("inserted value should be reachable");
    assert_eq!(*got, 42);
    assert!(ctx.contains::<u32>());

    let removed = ctx.remove::<u32>().expect("remove should return the value");
    assert_eq!(*removed, 42);
    assert!(!ctx.contains::<u32>());
    assert!(ctx.get::<u32>().is_none());
}

#[test]
fn insert_replaces_and_returns_prior() {
    let ctx = Ctx::new();
    let prior = ctx.insert(Arc::new(1u32));
    assert!(prior.is_none(), "first insert has no prior");

    let prior = ctx.insert(Arc::new(2u32));
    assert_eq!(prior.map(|v| *v), Some(1), "replace returns the previous Arc");
    assert_eq!(*ctx.get::<u32>().unwrap(), 2);
}

#[test]
fn get_with_wrong_type_returns_none_not_panic() {
    let ctx = Ctx::new();
    ctx.insert(Arc::new(1u32));
    // Same key domain, different type — must be `None`, never a downcast panic.
    assert!(ctx.get::<u64>().is_none());
    assert!(ctx.get::<i32>().is_none());
    assert!(ctx.remove::<u64>().is_none());
    assert!(!ctx.contains::<u64>());
}

#[test]
fn get_or_insert_returns_live_arc_and_dedupes() {
    let ctx = Ctx::new();
    let a = ctx.get_or_insert(7u32);
    let b = ctx.get_or_insert(7u32);
    assert_eq!(*a, 7);
    assert!(
        Arc::ptr_eq(&a, &b),
        "get_or_insert must return the existing Arc, not re-insert"
    );
}

#[test]
fn clone_shares_the_map() {
    let ctx = Ctx::new();
    let shared = ctx.clone();

    ctx.insert(Arc::new(99u32));
    // The clone observes the parent's write — same underlying map.
    assert_eq!(*shared.get::<u32>().unwrap(), 99);

    shared.insert(Arc::new(100u32));
    // And vice versa: the parent observes the clone's write.
    assert_eq!(*ctx.get::<u32>().unwrap(), 100);
}

#[test]
fn fork_isolates_the_map() {
    let ctx = Ctx::new();
    let fork = ctx.fork();

    ctx.insert(Arc::new(1u32));
    // The fork has its own (copied) map: the parent's insert is invisible.
    assert!(!fork.contains::<u32>(), "a forked map must not see the parent's insert");

    fork.insert(Arc::new(2u32));
    // The parent's map is unaffected by the fork's insert: still holds value 1.
    assert_eq!(*ctx.get::<u32>().unwrap(), 1, "the parent must not see the fork's insert");
    assert_eq!(*fork.get::<u32>().unwrap(), 2);
}

#[test]
fn fork_shares_arc_values() {
    let ctx = Ctx::new();
    let counter = Arc::new(AtomicUsize::new(0));
    ctx.insert(counter.clone());

    let fork = ctx.fork();
    // The map structure is copied, but the `Arc` value is shared.
    fork.get::<AtomicUsize>()
        .expect("forked map retains the Arc entry")
        .fetch_add(1, Ordering::SeqCst);

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "mutating the shared Arc through the fork is visible through the parent"
    );
}

#[test]
fn standard_elements_are_shared_across_forks() {
    let ctx = Ctx::new();
    let fork = ctx.fork();

    assert!(Arc::ptr_eq(&ctx.clock(), &fork.clock()));
    assert!(Arc::ptr_eq(&ctx.config(), &fork.config()));
    assert!(Arc::ptr_eq(&ctx.rng(), &fork.rng()));
}

#[test]
fn standard_elements_are_reachable_and_rng_draws() {
    let ctx = Ctx::new();

    // Clock reads are live (elapsed grows, or at least is a valid Duration).
    let _ = ctx.clock().now();
    let _ = ctx.clock().elapsed();
    let _ = ctx.config();

    // The RNG is a real, working stream.
    let rng = ctx.rng();
    let mut guard = rng.lock().expect("rng lock poisoned");
    let first = guard.next_u64();
    let second = guard.next_u64();
    assert_ne!(first, second, "two draws from a live RNG stream must differ");
}

#[test]
fn fork_preserves_shared_rng_stream_state() {
    // The RNG is shared (same Arc) across a fork, so a draw through one
    // handle advances the stream seen through the other.
    let ctx = Ctx::new();
    let fork = ctx.fork();

    let a = ctx.rng().lock().expect("lock").next_u64();
    let b = fork.rng().lock().expect("lock").next_u64();
    assert_ne!(a, b, "shared RNG advances across fork handles");
}

/// A manual clock that lets a test move simulated time **forward** (`advance`)
/// or **backward** (`rewind`) — mock-only affordances the real `MonotonicClock`
/// deliberately lacks. It stays `Send + Sync` and object-safe, so the same value
/// seeds `Ctx` as `Arc<dyn Clock>` while the test keeps the concrete
/// `Arc<MockClock>` to drive it.
#[derive(Debug, Clone)]
struct MockClock {
    origin: foundation_compact::Instant,
    now: Arc<Mutex<foundation_compact::Instant>>,
}

impl MockClock {
    fn new() -> Self {
        let origin = foundation_compact::Instant::now();
        Self {
            origin,
            now: Arc::new(Mutex::new(origin)),
        }
    }

    /// Move simulated time forward by `d`.
    fn advance(&self, d: foundation_compact::Duration) {
        let mut now = self.now.lock().expect("mock clock poisoned");
        *now = *now + d;
    }

    /// Move simulated time backward by `d`. Once `now` falls below the origin,
    /// `Clock::elapsed` clamps to zero — a monotonic clock never reports
    /// negative elapsed.
    fn rewind(&self, d: foundation_compact::Duration) {
        let mut now = self.now.lock().expect("mock clock poisoned");
        *now = now.checked_sub(d).unwrap_or(self.origin);
    }
}

impl Clock for MockClock {
    fn now(&self) -> foundation_compact::Instant {
        *self.now.lock().expect("mock clock poisoned")
    }

    fn elapsed(&self) -> foundation_compact::Duration {
        self.now().saturating_duration_since(self.origin)
    }
}

#[test]
fn mock_clock_moves_time_forward_and_backward() {
    let clock = MockClock::new();

    clock.advance(foundation_compact::Duration::from_millis(5));
    assert_eq!(clock.elapsed(), foundation_compact::Duration::from_millis(5));

    clock.rewind(foundation_compact::Duration::from_millis(3));
    assert_eq!(clock.elapsed(), foundation_compact::Duration::from_millis(2));

    // Rewinding below the origin clamps elapsed to zero rather than going negative.
    clock.rewind(foundation_compact::Duration::from_millis(10));
    assert_eq!(clock.elapsed(), foundation_compact::Duration::ZERO);

    // Object-safe: the same manual clock seeds `Ctx` as an `Arc<dyn Clock>`.
    let erased: Arc<dyn Clock> = Arc::new(clock);
    assert_eq!(erased.elapsed(), foundation_compact::Duration::ZERO);
    assert_send_sync::<Arc<dyn Clock>>();
}
