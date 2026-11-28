// Tests build the effect sources on purpose (spec 84, section 6, item 2).
#![allow(clippy::disallowed_methods)]

//! A drive from inside an engine task must return (spec 92 feature 01, stage C,
//! issue #1128).
//!
//! WHY: `run_until` counted the task that is being polled as runnable work.
//! That task is packed (its slot has no iterator while it runs), but it still
//! had a slot, so `has_runnable_work()` stayed true. A nested drive from inside
//! a task then never saw "no work" and looped forever. The deterministic
//! simulation test (DST) hit this when the ConnectRPC client's reader task
//! drove the turn.
//!
//! WHAT: a task calls `run_until_next_state()` from inside its own
//! `next_status()`. The nested drive must return, must still run another
//! runnable task, and must not count the executing task itself.
//!
//! HOW: the tests run in [`SimRuntime`], the single-threaded engine of the DST.
//! A watchdog thread fails a test in seconds instead of hanging continuous
//! integration (CI).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::sim::SimRuntime;
use foundation_core::valtron::{execute, run_until_next_state, send};
use foundation_core::valtron::{Stream, TaskIterator, TaskStatus};

/// The start of the simulated clock.
const START: Duration = Duration::from_secs(1_700_000_000);

/// How long a test may take before its watchdog fails it.
const WATCHDOG: Duration = Duration::from_secs(5);

/// Run `body` on its own thread and fail the test when it does not return in
/// [`WATCHDOG`]. A nested drive that loops forever cannot hang CI.
fn with_watchdog(name: &'static str, body: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    rx.recv_timeout(WATCHDOG).unwrap_or_else(|_| {
        panic!("watchdog: {name} did not return within {WATCHDOG:?}; a nested drive never stopped")
    });
}

/// A task that drives the engine from inside its own poll, then becomes ready.
struct NestedDrive {
    /// Set when the nested drive returned.
    drove: Arc<AtomicBool>,
    /// The work to schedule for the nested drive, if any.
    schedule: Option<Schedule>,
}

/// The kind of task the nested drive finds.
enum Schedule {
    /// A task that counts its polls and becomes ready once.
    Runnable(Arc<AtomicUsize>),
}

impl TaskIterator for NestedDrive {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u32, ()>> {
        if let Some(schedule) = self.schedule.take() {
            match schedule {
                Schedule::Runnable(polls) => {
                    send(CountedTask { polls }).expect("the counted task is scheduled");
                }
            }
        }
        // The nested drive: this very task is packed while it runs.
        run_until_next_state();
        self.drove.store(true, Ordering::SeqCst);
        Some(TaskStatus::Ready(7))
    }
}

/// A task that counts its polls and becomes ready once.
struct CountedTask {
    polls: Arc<AtomicUsize>,
}

impl TaskIterator for CountedTask {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u32, ()>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Some(TaskStatus::Ready(1))
    }
}

/// A drive from inside the only task must return. Nothing else is runnable, so
/// the nested loop has to see the executing task as no work.
#[test]
fn nested_drive_returns_when_nothing_else_runs() {
    with_watchdog("nested_drive_returns_when_nothing_else_runs", || {
        SimRuntime::run(42, START, |_sim| {
            let drove = Arc::new(AtomicBool::new(false));
            let mut stream = execute(
                NestedDrive {
                    drove: Arc::clone(&drove),
                    schedule: None,
                },
                None,
            )
            .expect("the engine is initialized");

            let item = stream.next();

            assert!(
                drove.load(Ordering::SeqCst),
                "the nested drive must return before the task reports ready"
            );
            assert!(
                matches!(item, Some(Stream::Next(7))),
                "the task must report ready after the nested drive; got {item:?}"
            );
        });
    });
}

/// The nested drive must still find and poll another runnable task. Excluding
/// the executing task must not hide real work.
#[test]
fn nested_drive_still_runs_another_task() {
    with_watchdog("nested_drive_still_runs_another_task", || {
        SimRuntime::run(7, START, |_sim| {
            let polls = Arc::new(AtomicUsize::new(0));
            let drove = Arc::new(AtomicBool::new(false));
            let mut stream = execute(
                NestedDrive {
                    drove: Arc::clone(&drove),
                    schedule: Some(Schedule::Runnable(Arc::clone(&polls))),
                },
                None,
            )
            .expect("the engine is initialized");

            let item = stream.next();

            assert!(drove.load(Ordering::SeqCst), "the nested drive must return");
            assert_eq!(
                polls.load(Ordering::SeqCst),
                1,
                "the nested drive must poll the other runnable task once"
            );
            assert!(
                matches!(item, Some(Stream::Next(7))),
                "the driving task must report ready; got {item:?}"
            );
        });
    });
}
