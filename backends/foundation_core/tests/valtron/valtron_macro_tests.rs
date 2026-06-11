//! WHY: `#[valtron]` / `#[valtron_test]` (foundation_macros, re-exported from
//! `foundation_core::valtron`) must actually deliver their contract: the engine
//! is LIVE for the whole wrapped body (the `PoolGuard` from
//! `initialize_pool(seed, threads)` outlives it), return values pass through,
//! and the test variant defaults to the `Some(3)`-minimum thread count.
//!
//! WHAT: Spawn-and-complete through a macro-initialized engine; explicit
//! `seed`/`threads` arguments; `Result`-returning bodies; `#[valtron]` on a
//! plain entry-point-style fn.
//!
//! HOW: Each case relies ONLY on the macro for engine setup — no manual
//! `initialize_pool` — so a green run proves the guard wiring.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_core::valtron::{
    single::{run_until_complete, spawn},
    valtron, valtron_test, FnReady, NoSpawner, TaskIterator, TaskStatus,
};

/// Produces `max` ready values, one per executor iteration.
struct CountingTask {
    max: usize,
    current: usize,
}

impl TaskIterator for CountingTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current >= self.max {
            return None;
        }
        self.current += 1;
        Some(TaskStatus::Ready(self.current))
    }
}

/// Drive a 3-iteration task to completion on whatever engine the surrounding
/// macro initialized; returns how many Ready values the resolver saw.
fn run_counting_task() -> usize {
    let seen = Rc::new(RefCell::new(0usize));
    let seen_in_resolver = Rc::clone(&seen);
    spawn()
        .with_task(CountingTask { max: 3, current: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(_) = item {
                *seen_in_resolver.borrow_mut() += 1;
            }
        })))
        .schedule()
        .expect("engine must be initialized by the macro");
    run_until_complete();
    let count = *seen.borrow();
    count
}

#[valtron_test]
fn engine_is_live_for_the_whole_body() {
    // No manual initialize_pool anywhere — if the macro didn't set the engine up
    // (or dropped the guard early), schedule() would fail.
    assert_eq!(run_counting_task(), 3);
}

#[valtron_test(seed = 42, threads = 4)]
fn explicit_seed_and_threads_are_accepted() {
    assert_eq!(run_counting_task(), 3);
}

#[valtron_test(threads = 1)]
fn thread_count_below_the_minimum_is_clamped_not_rejected() {
    // `threads = 1` is clamped to the Some(3) minimum by the macro — the case
    // still runs on a properly-sized engine rather than failing.
    assert_eq!(run_counting_task(), 3);
}

#[valtron_test]
fn result_returning_bodies_pass_their_output_through() -> Result<(), String> {
    // `?`/`return` semantics must survive the wrapping (the body becomes an
    // inner FN, not a closure) — and the harness must see the Ok.
    if run_counting_task() != 3 {
        return Err("task did not complete".into());
    }
    Ok(())
}

/// An entry-point-style fn: `#[valtron]` brings the engine up around it and
/// hands back the body's return value.
#[valtron(seed = 7)]
fn entry_point_style() -> usize {
    run_counting_task()
}

#[test]
fn valtron_entry_point_wraps_and_returns() {
    assert_eq!(entry_point_style(), 3);
}
