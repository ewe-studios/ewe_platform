//! Platform wall times read the injected clock (spec 70 feature 134).
//!
//! WHY: the platform stamps cached entries, offline mutations and session ids
//! with a wall time. A direct `SystemTime::now` read made a deterministic
//! simulation test impossible.
//!
//! WHAT: a `SimulatedClock` sets the wall origin; each test stores or enqueues
//! a value, asserts the stored stamp is the simulated time, advances the clock
//! and asserts the next stamp moves with it.
//!
//! HOW: `SimulatedClock::advance_wall` moves the wall clock by hand. Before this
//! feature each src file read the real clock, so the simulated origin never
//! appeared in a stored value.

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_platform::{CacheManager, MutationQueue, PlatformSession, Profile};

/// A simulated clock whose wall time starts `secs` seconds after the UNIX epoch.
fn clock_at(secs: u64) -> Arc<SimulatedClock> {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(secs));
    clock
}

#[test]
fn a_cache_entry_is_stamped_from_the_injected_wall_clock() {
    let clock = clock_at(1_700_000_000);
    let cache = CacheManager::in_memory(Ctx::fixed(clock.clone()));

    cache.store(Profile::App, "/app/home", b"body", "text/html");
    assert_eq!(
        cache
            .get(Profile::App, "/app/home")
            .expect("cached entry")
            .cached_at,
        1_700_000_000,
        "the entry must carry the simulated wall origin, not the real time"
    );

    clock.advance_wall(Duration::from_secs(60));
    cache.store(Profile::App, "/app/about", b"body", "text/html");
    assert_eq!(
        cache
            .get(Profile::App, "/app/about")
            .expect("cached entry")
            .cached_at,
        1_700_000_060,
        "a wall-clock advance must move the next entry's stamp"
    );
}

#[test]
fn a_mutation_is_stamped_from_the_injected_wall_clock() {
    let clock = clock_at(1_700_000_000);
    let queue = MutationQueue::in_memory(Ctx::fixed(clock.clone()));

    let _ = queue.enqueue("order_update", serde_json::json!({ "id": 1 }));
    assert_eq!(
        queue.all()[0].created_at,
        1_700_000_000_000,
        "the mutation must carry the simulated wall origin in milliseconds"
    );

    clock.advance_wall(Duration::from_millis(500));
    let _ = queue.enqueue("order_update", serde_json::json!({ "id": 2 }));
    let stamps: Vec<u64> = queue.all().iter().map(|m| m.created_at).collect();
    assert!(
        stamps.contains(&1_700_000_000_500),
        "the second stamp must follow the advanced clock: {stamps:?}"
    );
}

#[test]
fn a_session_id_is_the_injected_wall_time_in_milliseconds() {
    let clock = clock_at(1_700_000_000);
    let session =
        PlatformSession::new_test(std::path::PathBuf::from("."), Ctx::fixed(clock.clone()));
    assert_eq!(session.session_id().0, 1_700_000_000_000);

    clock.advance_wall(Duration::from_millis(1_000));
    let later = PlatformSession::new_test(std::path::PathBuf::from("."), Ctx::fixed(clock));
    assert_eq!(
        later.session_id().0,
        1_700_000_001_000,
        "a wall-clock advance must move the next session id"
    );
}

/// A session reads its wall time from the clock of the `Ctx` it is given. An
/// advance of the simulated clock shows through the session's own `ctx()`.
#[test]
fn a_session_reads_its_wall_time_through_the_ctx_it_was_given() {
    let clock = clock_at(1_700_000_000);
    let session =
        PlatformSession::new_test(std::path::PathBuf::from("."), Ctx::fixed(clock.clone()));
    let before = session.ctx().clock().wall_now();

    clock.advance_wall(Duration::from_secs(42));
    let after = session.ctx().clock().wall_now();

    assert_eq!(
        after
            .duration_since(before)
            .expect("the wall clock moved forward"),
        Duration::from_secs(42),
        "the session must see the advance through its ctx"
    );
}
