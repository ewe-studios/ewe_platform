//! The deterministic simulation testing (DST) replay harness (spec 84 T6).
//!
//! A replay builds one effect environment from one seed and records a trace of
//! the values that the run produced. The same seed must give the same trace in
//! two separate processes. The test `tests/dst_replay.rs` proves that with two
//! child processes.

use std::sync::Arc;

use foundation_context::env::{Env, Scru128Source};
use foundation_context::{Clock, Ctx, Duration};
use foundation_core::env::ValtronTimer;
use foundation_core::valtron::SimulatedClock;

/// The number of IDs that a replay creates.
pub const REPLAY_IDS: usize = 100;

/// The number of waits that a replay records.
pub const REPLAY_WAITS: usize = 10;

/// A minimal replay that needs no executor.
///
/// The trace has one line for each ID and one line for each wait. A wait is a
/// deadline on the simulated clock, so the trace shows both the value sequence
/// and the time sequence.
#[must_use]
pub fn replay(seed: u64) -> Vec<String> {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone() as Arc<dyn Clock>);
    let env = simulated_env(&ctx, seed);
    ctx.store(env.clone());

    let ids = env.ids();
    let mut trace = Vec::new();
    let mut wait = 0;
    for index in 0..REPLAY_IDS {
        trace.push(format!("id {}", ids.next_id()));
        if index % (REPLAY_IDS / REPLAY_WAITS) == 0 {
            clock.advance(Duration::from_millis(10));
            trace.push(format!("wait {} at {}", wait, clock.elapsed().as_millis()));
            wait += 1;
        }
    }
    trace
}

/// The simulated environment of a replay.
///
/// It uses the clock of `ctx` and a seeded entropy source, so both the clock
/// and the randomness come from the seed.
#[must_use]
pub fn simulated_env(ctx: &Ctx, seed: u64) -> Env {
    let entropy = Arc::new(foundation_context::env::SeededEntropy::new(seed));
    let ids = Arc::new(
        Scru128Source::new(ctx.clock(), entropy.as_ref()).expect("a seeded source never fails"),
    );
    Env::new(Arc::new(ValtronTimer), entropy, ids)
}
