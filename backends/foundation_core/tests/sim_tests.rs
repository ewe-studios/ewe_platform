//! The deterministic simulation runtime.
//!
//! One seed gives one clock, one random stream, one identifier stream and one
//! task order.

// The runtime exists only with the `testing` feature and the single-threaded
// engine. Without this gate the target tries to import a module that the
// feature set does not build.
#![cfg(all(feature = "testing", not(feature = "multi")))]

use std::time::Duration;

use foundation_compact::rng::Rng;
use foundation_core::valtron::sim::SimRuntime;

const START: Duration = Duration::from_secs(1_700_000_000);

/// The random draws and identifiers of one run.
fn draws(sim: &SimRuntime) -> (Vec<u64>, Vec<String>) {
    let rng = sim.ctx().rng();
    let mut rng = rng
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let numbers = (0..4).map(|_| rng.next_u64()).collect();
    drop(rng);
    let env = sim.ctx().require_env().expect("the simulated Env");
    let ids = (0..4)
        .map(|_| env.ids().next_id().to_string())
        .collect::<Vec<_>>();
    (numbers, ids)
}

#[test]
fn sim_clock_moves_only_when_the_test_advances_it() {
    SimRuntime::run(7, START, |sim| {
        let before = sim.ctx().clock().now();
        assert_eq!(sim.ctx().clock().now() - before, Duration::ZERO);
        sim.advance(Duration::from_secs(5));
        assert_eq!(
            sim.ctx().clock().now() - before,
            Duration::from_secs(5),
            "the simulated clock moves by exactly the advance"
        );
        assert_eq!(
            sim.ctx().clock().wall_now(),
            sim.ctx().clock().wall_now(),
            "the wall clock does not move on its own"
        );
    });
}

#[test]
fn sim_same_seed_gives_the_same_streams() {
    let first = SimRuntime::run(42, START, |sim| draws(&sim));
    let second = SimRuntime::run(42, START, |sim| draws(&sim));
    assert_eq!(first, second, "seed 42 must replay exactly");
}

#[test]
fn sim_different_seeds_give_different_streams() {
    let first = SimRuntime::run(1, START, |sim| draws(&sim));
    let second = SimRuntime::run(2, START, |sim| draws(&sim));
    assert_ne!(first, second, "two seeds must not draw the same values");
}

#[test]
fn sim_ids_are_deterministic_and_sortable() {
    let ids = SimRuntime::run(9, START, |sim| {
        let env = sim.ctx().require_env().expect("the simulated Env");
        (0..8)
            .map(|_| env.ids().next_id().to_string())
            .collect::<Vec<_>>()
    });
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(
        ids, sorted,
        "scru128 identifiers are sortable in time order"
    );
}
