//! Shared test helpers: one fixed-seed property configuration and a
//! simulated clock.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::SimulatedClock;
use proptest::prelude::ProptestConfig;
use proptest::test_runner::RngSeed;

/// The number of cases that each property test runs.
pub const CASES: u32 = 64;

/// One property configuration with a fixed seed.
#[must_use]
pub fn config(seed: u64) -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        rng_seed: RngSeed::Fixed(seed),
        ..ProptestConfig::default()
    }
}

/// A clock that never reads the real clock.
#[must_use]
pub fn clock() -> Arc<SimulatedClock> {
    Arc::new(SimulatedClock::new(Duration::ZERO))
}
