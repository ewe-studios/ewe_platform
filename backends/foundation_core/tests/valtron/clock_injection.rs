#![cfg(feature = "test-clock")]

//! A host installs its own clock in the pool [`Ctx`], and every task sees it.
//!
//! WHY: the engine clock was fixed when a pool started, so a deterministic
//! simulation test (DST) could not put the whole pool on one simulated clock.
//!
//! WHAT: `Ctx::with_clock` returns a new handle with the given clock. The
//! startup entry points and the `ctx = expr` macro argument carry that handle as
//! the pool root. Each task reads a fork of the root.
//!
//! HOW: the `test-clock` feature provides [`SimulatedClock`]. A static clock
//! lets a test hold the `Arc` and advance it. The body reaches the engine root
//! through the public `fork_ctx` helper.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_core::valtron::{
    execute, fork_ctx, valtron_test, Clock, Ctx, SimulatedClock, StreamIteratorExt, TaskIterator,
    TaskStatus,
};

/// Build a pool root with `clock` as its engine clock.
fn ctx_with(clock: &Arc<SimulatedClock>) -> Ctx {
    let clock: Arc<dyn Clock> = clock.clone();
    Ctx::fixed(clock)
}

/// The pool-root test clock: about one hour ahead of real time.
static ROOT_CLOCK: LazyLock<Arc<SimulatedClock>> =
    LazyLock::new(|| Arc::new(SimulatedClock::new(Duration::from_secs(3600))));

/// The task-context test clock: about two hours ahead of real time.
static TASK_CLOCK: LazyLock<Arc<SimulatedClock>> =
    LazyLock::new(|| Arc::new(SimulatedClock::new(Duration::from_secs(7200))));

/// The timeout-path test clock: about thirty minutes ahead of real time.
static TIMEOUT_CLOCK: LazyLock<Arc<SimulatedClock>> =
    LazyLock::new(|| Arc::new(SimulatedClock::new(Duration::from_secs(1800))));

#[test]
fn with_clock_returns_the_given_clock_and_keeps_the_original() {
    let ctx = Ctx::host();
    let original = ctx.clock();
    let simulated: Arc<dyn Clock> = Arc::new(SimulatedClock::new(Duration::from_secs(5)));

    let with_simulated = ctx.with_clock(simulated.clone());

    assert!(
        Arc::ptr_eq(&with_simulated.clock(), &simulated),
        "the new handle must hold the given clock"
    );
    assert!(
        Arc::ptr_eq(&ctx.clock(), &original),
        "the original handle must keep its own clock"
    );
    assert!(
        !Arc::ptr_eq(&ctx.clock(), &simulated),
        "the original handle must not switch to the given clock"
    );
}

#[test]
fn with_clock_shares_config_and_rng() {
    let ctx = Ctx::host();
    let simulated: Arc<dyn Clock> = Arc::new(SimulatedClock::new(Duration::ZERO));

    let with_simulated = ctx.with_clock(simulated);

    assert!(Arc::ptr_eq(&with_simulated.config(), &ctx.config()));
    assert!(Arc::ptr_eq(&with_simulated.rng(), &ctx.rng()));
}

#[test]
fn with_clock_copies_the_map() {
    let ctx = Ctx::host();
    ctx.insert(Arc::new(1u32));

    let simulated: Arc<dyn Clock> = Arc::new(SimulatedClock::new(Duration::ZERO));
    let with_simulated = ctx.with_clock(simulated);

    assert_eq!(
        *with_simulated
            .get::<u32>()
            .expect("the copied map keeps the early entry"),
        1
    );

    ctx.insert(Arc::new(2u64));

    assert!(
        !with_simulated.contains::<u64>(),
        "an insert after the call is invisible to the new handle"
    );
    assert!(ctx.contains::<u64>(), "the original sees its own insert");
    assert!(
        ctx.contains::<u32>(),
        "the original keeps both its early and late entries"
    );
}

#[valtron_test(ctx = ctx_with(&ROOT_CLOCK))]
fn the_pool_root_carries_the_injected_clock() {
    ROOT_CLOCK.advance(Duration::from_secs(7));

    let ctx = fork_ctx();
    assert_eq!(
        ctx.clock().now(),
        ROOT_CLOCK.now(),
        "a task-side fork must read the simulated pool clock"
    );

    let simulated = ctx.clock().now();
    let real = Instant::now();
    let apart = simulated
        .checked_duration_since(real)
        .or_else(|| real.checked_duration_since(simulated))
        .expect("two instants are always ordered");
    assert!(
        apart >= Duration::from_secs(600),
        "the pool clock must be the simulated one, not real time (apart by {apart:?})"
    );
}

/// Yields the instant it reads from the task context, then ends.
struct ReadsClock {
    done: bool,
}

impl TaskIterator for ReadsClock {
    type Ready = Instant;
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        self.done = true;
        Some(TaskStatus::Ready(ctx.clock().now()))
    }
}

#[valtron_test(ctx = ctx_with(&TASK_CLOCK))]
async fn the_task_ctx_inherits_the_pool_clock() {
    let got = execute(ReadsClock { done: false }, None)
        .expect("the macro starts the pool")
        .into_collect_future()
        .await;

    assert_eq!(
        got,
        vec![TASK_CLOCK.now()],
        "the task context must read the simulated pool clock"
    );
}

#[valtron_test(timeout = 10_000, ctx = ctx_with(&TIMEOUT_CLOCK))]
fn the_macro_passes_the_ctx_to_the_timeout_path() {
    let ctx = fork_ctx();
    assert_eq!(
        ctx.clock().now(),
        TIMEOUT_CLOCK.now(),
        "the timeout path must keep the injected clock"
    );
}
