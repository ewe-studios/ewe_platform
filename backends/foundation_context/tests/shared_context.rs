//! Tests for how a `Ctx` shares and copies its map.
//!
//! WHAT: `Ctx` is `Send + Sync`. `Ctx::clone` shares the map. `Ctx::fork` copies it.
//! `Ctx::with_clock` copies the map and swaps the clock, with a mock clock.

use std::sync::Arc;

use foundation_compact::{Duration, Instant, SystemTime};
use foundation_context::{Clock, Ctx};

fn assert_send_sync<T: Send + Sync>() {}

/// A clock with fixed values. It never reads the real time after it is built.
struct FixedClock {
    base: Instant,
}

impl Clock for FixedClock {
    fn now(&self) -> Instant {
        self.base
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }
}

fn fixed_clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock {
        base: Instant::now(),
    })
}

#[test]
fn ctx_is_send_and_sync() {
    assert_send_sync::<Ctx>();
}

#[test]
fn clone_shares_the_map() {
    let ctx = Ctx::fixed(fixed_clock());
    let shared = ctx.clone();

    ctx.insert(Arc::new(5u32));

    assert_eq!(*shared.get::<u32>().expect("the clone sees the insert"), 5);
    shared.insert(Arc::new(6u64));
    assert!(
        ctx.contains::<u64>(),
        "the insert through the clone is shared"
    );
}

#[test]
fn fork_copies_the_map() {
    let ctx = Ctx::fixed(fixed_clock());
    ctx.insert(Arc::new(1u32));

    let forked = ctx.fork();
    assert!(forked.contains::<u32>(), "the fork keeps the early entry");

    ctx.insert(Arc::new(2u64));
    forked.insert(Arc::new(3u8));

    assert!(!forked.contains::<u64>(), "a later parent insert is hidden");
    assert!(
        !ctx.contains::<u8>(),
        "a fork insert is hidden from the parent"
    );
    assert!(Arc::ptr_eq(&ctx.clock(), &forked.clock()));
    assert!(Arc::ptr_eq(&ctx.config(), &forked.config()));
    assert!(Arc::ptr_eq(&ctx.rng(), &forked.rng()));
}

#[test]
fn with_clock_swaps_the_clock_and_copies_the_map() {
    let ctx = Ctx::fixed(fixed_clock());
    let original = ctx.clock();
    ctx.insert(Arc::new(1u32));
    let other = fixed_clock();

    let swapped = ctx.with_clock(other.clone());

    assert!(Arc::ptr_eq(&swapped.clock(), &other));
    assert!(Arc::ptr_eq(&ctx.clock(), &original));
    assert!(Arc::ptr_eq(&swapped.config(), &ctx.config()));
    assert!(Arc::ptr_eq(&swapped.rng(), &ctx.rng()));
    assert!(swapped.contains::<u32>());
    ctx.insert(Arc::new(2u64));
    assert!(!swapped.contains::<u64>());
}
