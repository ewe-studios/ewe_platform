//! WHY: The hub's contract is exactly what makes it worth existing: FIFO
//! command application, ONE stabilize per pump, and — the headline property —
//! remote readers never observing a mid-stabilize (glitchy) state.
//!
//! WHAT: Feature 18 §5 rows 1-3, 5-8, 10 driven with real worker threads
//! (`std::thread` — the valtron-pool e2e additionally runs under the `valtron`
//! feature via the `TaskIterator` impls).

#![cfg(not(target_arch = "wasm32"))]

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_signals::{Context, HubGone, Runtime, SignalHub};

fn setup() -> (Rc<Runtime>, Context, SignalHub) {
    let runtime = Rc::new(Runtime::new());
    let ctx = Context::new(Rc::clone(&runtime));
    let hub = SignalHub::new(&runtime);
    (runtime, ctx, hub)
}

/// Rows 1 + 10 — worker set + update land FIFO; one stabilize; effect ran once.
#[test]
fn remote_set_and_update_apply_fifo() {
    let (_runtime, ctx, hub) = setup();
    let (count, set_count) = ctx.signal(0i64);
    let remote = hub.expose_setter(&set_count);

    let runs = Rc::new(std::cell::RefCell::new(0));
    let counter = Rc::clone(&runs);
    let read = count.clone();
    ctx.effect(move || {
        let _ = read.get();
        *counter.borrow_mut() += 1;
    });

    let worker = std::thread::spawn(move || {
        remote.set(1).unwrap();
        remote.update(|v| *v += 1).unwrap(); // FIFO: set(1) THEN +1 = 2
    });
    worker.join().unwrap();

    let report = hub.pump();
    assert_eq!(report.commands_applied, 2);
    assert!(report.stabilized);
    assert_eq!(count.get(), 2, "set then update, in order");
    assert_eq!(*runs.borrow(), 2, "initial run + ONE re-run for the whole pump");
}

/// Row 2 — many sets from several workers: all applied, ONE stabilize.
#[test]
fn many_workers_coalesce_into_one_stabilize() {
    let (_runtime, ctx, hub) = setup();
    let (total, set_total) = ctx.signal(0u64);
    let remote = hub.expose_setter(&set_total);

    let mut workers = Vec::new();
    for w in 0..4u64 {
        let handle = remote.clone();
        workers.push(std::thread::spawn(move || {
            for i in 0..25u64 {
                handle.update(move |v| *v += w * 25 + i + 1 - (w * 25 + i)).unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }

    let report = hub.pump();
    assert_eq!(report.commands_applied, 100);
    assert_eq!(total.get(), 100, "every update applied exactly once");
}

/// Row 3 — THE property: remote snapshots never see a half-propagated diamond.
#[test]
fn snapshots_are_never_glitchy() {
    let (_runtime, ctx, hub) = setup();
    let (a, set_a) = ctx.signal(1i64);
    let a2 = ctx.computed({
        let a = a.clone();
        move || a.get() * 2
    });
    let a3 = ctx.computed({
        let a = a.clone();
        move || a.get() * 3
    });
    // D = 2a + 3a = 5a — ANY observed snapshot must be divisible by 5.
    let d = ctx.computed(move || a2.get() + a3.get());
    let (d_mirror, set_d_mirror) = ctx.signal(5i64);
    {
        let d = d.clone();
        let set = set_d_mirror.clone();
        ctx.effect(move || set.set(d.get()));
    }
    let remote_d = hub.expose(&d_mirror);
    let remote_a = hub.expose_setter(&set_a);

    let stop = Arc::new(AtomicBool::new(false));
    let watcher_stop = Arc::clone(&stop);
    let watcher = std::thread::spawn(move || {
        let mut observed = 0u32;
        while !watcher_stop.load(Ordering::Relaxed) {
            if let Some(value) = remote_d.snapshot() {
                assert_eq!(value % 5, 0, "torn diamond observed: {value}");
                observed += 1;
            }
        }
        observed
    });

    for i in 2..200i64 {
        remote_a.set(i).unwrap();
        let _ = hub.pump();
    }
    stop.store(true, Ordering::Relaxed);
    let observed = watcher.join().unwrap();
    assert!(observed > 0, "the watcher actually sampled");
    assert_eq!(d_mirror.get(), 199 * 5);
}

/// Rows 4 + 5 — streams yield each post-stabilize value; version counts
/// actual changes only.
#[test]
fn streams_and_versions_track_published_changes() {
    let (_runtime, ctx, hub) = setup();
    let (value, set_value) = ctx.signal(0i32);
    let remote = hub.expose(&value);
    let stream = remote.changes();
    let setter = hub.expose_setter(&set_value);

    // The exposure's publisher effect ran at install (decision 008).
    assert_eq!(remote.snapshot(), Some(0));
    let v0 = remote.version();

    setter.set(7).unwrap();
    let _ = hub.pump();
    setter.set(7).unwrap(); // PartialEq dedup — no change, no publish
    let _ = hub.pump();
    setter.set(9).unwrap();
    let _ = hub.pump();

    assert_eq!(remote.version(), v0 + 2, "two ACTUAL changes");
    let mut seen = Vec::new();
    while let Ok(Some(v)) = stream.try_next() {
        seen.push(v);
    }
    assert_eq!(seen, vec![7, 9], "each post-stabilize value, no dupes");
}

/// Row 6 — remote signal creation via `run_on_hub` (§3.4 pattern).
#[test]
fn run_on_hub_creates_and_exposes() {
    let (_runtime, _ctx, hub) = setup();
    let handle = hub.handle();

    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        handle
            .run_on_hub(move |scope| {
                let (getter, setter) = scope.context().signal(0u64);
                let pair = (scope.expose(&getter), scope.expose_setter(&setter));
                let _ = reply_tx.send(pair);
            })
            .unwrap();
    });
    worker.join().unwrap();
    let _ = hub.pump();

    let (remote_get, remote_set) = reply_rx.recv().expect("handles minted on the hub");
    remote_set.set(41).unwrap();
    remote_set.update(|v| *v += 1).unwrap();
    let _ = hub.pump();
    assert_eq!(remote_get.snapshot(), Some(42));
}

/// Row 8 — hub drop: handles report `HubGone`; streams end.
#[test]
fn hub_drop_is_observable() {
    let (_runtime, ctx, hub) = setup();
    let (value, set_value) = ctx.signal(0i32);
    let remote_get = hub.expose(&value);
    let remote_set = hub.expose_setter(&set_value);
    let handle = hub.handle();
    let stream = remote_get.changes();

    drop(hub);
    drop(ctx); // disposes the publisher effect → stream sender dropped

    assert_eq!(remote_set.set(1), Err(HubGone));
    assert_eq!(handle.run_on_hub(|_| {}), Err(HubGone));
    assert_eq!(stream.try_next(), Err(HubGone), "stream ended");
    assert_eq!(remote_get.snapshot(), Some(0), "snapshot stays at last value");
}

/// Empty pump: no commands, no stabilize.
#[test]
fn idle_pump_is_a_noop() {
    let (_runtime, _ctx, hub) = setup();
    let report = hub.pump();
    assert_eq!(report.commands_applied, 0);
    assert!(!report.stabilized);
}

/// Row 11 — the valtron leg: `SignalStream` + `HubDriver` as `TaskIterator`s on the
/// single executor, no sleeps or spins in test code.
#[cfg(feature = "valtron")]
#[test]
fn valtron_stream_and_driver_integration() {
    use foundation_core::valtron::{NoSpawner, TaskIterator, TaskStatus};
    use foundation_signals::HubDriver;

    let (_runtime, ctx, hub) = setup();
    let (value, set_value) = ctx.signal(0i32);
    let remote = hub.expose(&value);
    let mut stream = remote.changes();
    let setter = hub.expose_setter(&set_value);

    setter.set(5).unwrap();
    let mut driver = HubDriver::new(hub);

    // Drive manually as the executor would: pump until Ready, then drain.
    let status = driver.next_status().expect("driver alive");
    assert!(matches!(status, TaskStatus::Ready(report) if report.commands_applied == 1));
    assert!(
        matches!(driver.next_status(), Some(TaskStatus::Pending(()))),
        "idle tick is Pending, never blocking"
    );

    let next: Option<TaskStatus<i32, (), NoSpawner>> = stream.next_status();
    assert!(matches!(next, Some(TaskStatus::Ready(5))));
    assert!(matches!(stream.next_status(), Some(TaskStatus::Pending(()))));
}
