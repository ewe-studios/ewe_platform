//! Tests for the `env = ...` macro argument (spec 84 T3).
//!
//! The single-threaded engine is thread-local, so these tests run only without
//! the `multi` feature. A multi-threaded pool has no thread-local root to read.

use std::sync::Arc;

use foundation_context::env::{CounterSource, Env, SeededEntropy};
use foundation_core::env::ValtronTimer;
use foundation_core::valtron::{single, valtron, valtron_test};

/// An `Env` with a counter ID source, so a test can assert on the value.
fn counter_env() -> Env {
    Env::new(
        Arc::new(ValtronTimer),
        Arc::new(SeededEntropy::new(7)),
        Arc::new(CounterSource::new(1)),
    )
}

#[cfg(not(feature = "multi"))]
#[test]
fn valtron_installs_the_env_expression() {
    #[valtron(env = counter_env(), seed = 7)]
    fn entry() {
        let env = single::root_ctx()
            .require_env()
            .expect("the macro installed the Env");
        assert_eq!(env.ids().next_id().to_u128(), 1);
        assert_eq!(env.ids().next_id().to_u128(), 2);
    }

    entry();
}

#[cfg(not(feature = "multi"))]
#[valtron_test(env = counter_env(), seed = 7)]
fn valtron_test_installs_the_env_expression() {
    let env = single::root_ctx()
        .require_env()
        .expect("the macro installed the Env");
    assert_eq!(env.ids().next_id().to_u128(), 1);
}

#[cfg(not(feature = "multi"))]
#[valtron_test(seed = 7)]
fn valtron_test_builds_a_simulated_env() {
    let env = single::root_ctx()
        .require_env()
        .expect("the macro built the default Env");
    // A simulated Env has a seeded ID source, so an ID is readable.
    let _ = env.ids().next_id();
}
