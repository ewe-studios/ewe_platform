//! The deterministic simulation runtime.
//!
//! WHY: a deterministic simulation test (DST) must replay one arbitrary run
//! from one seed. The time, the random bytes, the identifiers, the timers and
//! the task order must all come from that seed.
//!
//! WHAT: [`SimRuntime`] (one thread, one simulated clock, one seeded effect
//! bundle and one seeded engine) and [`SimRuntime::run`] (one seed on its own
//! thread, so many seeds run in one test).
//!
//! HOW: [`SimulatedClock`] moves only through [`SimRuntime::advance`].
//! [`SeededEntropy`](foundation_context::env::SeededEntropy) and the scru128
//! source read the seed, and the single-threaded engine picks the next task
//! with its seeded random number generator. Build this module only when the
//! `multi` feature is off: a multi-threaded pool has no fixed task order.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;

use super::ctx::SimulatedClock;
use super::executors::single;

/// One deterministic run: one thread, one clock, one seed.
pub struct SimRuntime {
    seed: u64,
    clock: Arc<SimulatedClock>,
    ctx: Ctx,
}

impl std::fmt::Debug for SimRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimRuntime")
            .field("seed", &self.seed)
            .finish_non_exhaustive()
    }
}

impl SimRuntime {
    /// Start the runtime on the current thread.
    ///
    /// The single-threaded engine starts once per thread. A second call on the
    /// same thread keeps the first engine, so use [`SimRuntime::run`] for a
    /// second seed.
    #[must_use]
    pub fn start(seed: u64, start: Duration) -> Self {
        let clock = Arc::new(SimulatedClock::new(start));
        let ctx = Ctx::with_seed(clock.clone(), seed);
        let env =
            crate::env::simulated(&ctx, seed).expect("the seeded effect environment cannot fail");
        ctx.store(env);
        single::initialize_pool_with_ctx(seed, ctx.clone());
        Self { seed, clock, ctx }
    }

    /// Run `body` with one seed on a fresh thread.
    ///
    /// The new thread gets its own single-threaded engine, so many seeds run in
    /// one test and each seed gets a separate clock and effect bundle.
    ///
    /// # Panics
    /// Panics when `body` panics; the panic carries the seed in its message.
    pub fn run<T, F>(seed: u64, start: Duration, body: F) -> T
    where
        F: FnOnce(SimRuntime) -> T + Send + 'static,
        T: Send + 'static,
    {
        std::thread::spawn(move || body(SimRuntime::start(seed, start)))
            .join()
            .unwrap_or_else(|panic| {
                std::panic::resume_unwind(panic);
            })
    }

    /// The seed of this run.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The simulated clock. Move it with [`advance`](Self::advance).
    #[must_use]
    pub fn clock(&self) -> &Arc<SimulatedClock> {
        &self.clock
    }

    /// The context: the clock, the seeded random number generator and the
    /// simulated effect bundle.
    #[must_use]
    pub fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    /// Move the clock forward by `by` and wake the parked tasks.
    pub fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// Drive `future` to completion on the single-threaded engine.
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        super::block_on_future(future)
    }

    /// Run every task that is ready, until no task remains runnable.
    pub fn run_until_complete(&self) {
        single::run_until_complete();
    }
}
