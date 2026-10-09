//! WHY: The reactive graph's correctness properties — glitch-free diamonds,
//! exactly-once evaluation, Check short-circuit, batch coalescing, cascading
//! disposal — are invariants downstream UI code silently relies on. Each spec
//! test group (feature 02 section "Testing") gets a direct probe.
//!
//! WHAT: Signal basics, effect tracking (incl. conditional deps), computeds,
//! diamond dependencies, three-state short-circuit, context disposal, the
//! callback registry, stabilize batching, re-entrant sets, cleanup ordering,
//! and height recomputation.
//!
//! HOW: Counters in `Rc<RefCell<...>>` record evaluation orders/counts; the
//! graph is driven only through the public API.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_signals::{Context, EventData, Runtime};

fn setup() -> (Rc<Runtime>, Context) {
    let runtime = Rc::new(Runtime::new());
    let ctx = Context::new(Rc::clone(&runtime));
    (runtime, ctx)
}

/// Shorthand: shared counter cell.
fn cell<T>(value: T) -> Rc<RefCell<T>> {
    Rc::new(RefCell::new(value))
}

// ─── Signal basics ─────────────────────────────────────────────────────────────

#[test]
fn signal_get_set_update() {
    let (_runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0);
    assert_eq!(count.get(), 0);
    set_count.set(5);
    assert_eq!(count.get(), 5);
    set_count.update(|v| *v += 3);
    assert_eq!(count.get(), 8);
}

#[test]
fn signals_are_independent() {
    let (_runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(1);
    let (b, _set_b) = ctx.signal(1);
    assert_ne!(a.node_id(), b.node_id());
    set_a.set(99);
    assert_eq!(a.get(), 99);
    assert_eq!(b.get(), 1);
}

// ─── Effect tracking ───────────────────────────────────────────────────────────

#[test]
fn effect_runs_immediately_with_initial_value() {
    let (_runtime, ctx) = setup();
    let (count, _set) = ctx.signal(0);
    let seen = cell(Vec::new());
    let log = Rc::clone(&seen);
    ctx.effect(move || log.borrow_mut().push(count.get()));
    // Decision 008: ran NOW, observed the initial value.
    assert_eq!(*seen.borrow(), vec![0]);
}

#[test]
fn effect_reruns_exactly_once_per_stabilize() {
    let (runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0);
    let seen = cell(Vec::new());
    let log = Rc::clone(&seen);
    ctx.effect(move || log.borrow_mut().push(count.get()));

    set_count.set(5);
    runtime.stabilize();
    assert_eq!(*seen.borrow(), vec![0, 5]);

    // No change — stabilize is a no-op for this effect.
    runtime.stabilize();
    assert_eq!(*seen.borrow(), vec![0, 5]);
}

#[test]
fn conditional_read_is_not_a_dependency_when_branch_is_cold() {
    let (runtime, ctx) = setup();
    let (flag, set_flag) = ctx.signal(false);
    let (value, set_value) = ctx.signal(0);
    let runs = cell(0);

    let counter = Rc::clone(&runs);
    let value_reader = value.clone();
    ctx.effect(move || {
        *counter.borrow_mut() += 1;
        if flag.get() {
            let _ = value_reader.get();
        }
    });
    assert_eq!(*runs.borrow(), 1);

    // flag is false — `value` was never read, so changing it must NOT re-run.
    set_value.set(10);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1, "cold-branch signal is not a dependency");

    // Flip the flag: effect re-runs and NOW tracks `value`.
    set_flag.set(true);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 2);
    set_value.set(20);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 3, "hot-branch signal became a dependency");
}

// ─── Computed ──────────────────────────────────────────────────────────────────

#[test]
fn computed_sums_and_reevaluates_once_for_two_inputs() {
    let (runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(1);
    let (b, set_b) = ctx.signal(2);
    let evals = cell(0);

    let counter = Rc::clone(&evals);
    let sum = ctx.computed(move || {
        *counter.borrow_mut() += 1;
        a.get() + b.get()
    });
    assert_eq!(sum.get(), 3);
    assert_eq!(*evals.borrow(), 1, "seed evaluation");

    set_a.set(10);
    runtime.stabilize();
    assert_eq!(sum.get(), 12);
    assert_eq!(*evals.borrow(), 2);

    // Both inputs change before one stabilize — ONE re-evaluation.
    set_a.set(100);
    set_b.set(200);
    runtime.stabilize();
    assert_eq!(sum.get(), 300);
    assert_eq!(*evals.borrow(), 3, "batched inputs evaluate once");
}

// ─── Diamond dependency ────────────────────────────────────────────────────────

#[test]
fn diamond_evaluates_the_join_exactly_once_with_fresh_inputs() {
    let (runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(1);
    let d_evals = cell(0);

    let a_for_b = a.clone();
    let b = ctx.computed(move || a_for_b.get() * 2);
    let a_for_c = a.clone();
    let c = ctx.computed(move || a_for_c.get() * 3);

    let counter = Rc::clone(&d_evals);
    let (b_read, c_read) = (b.clone(), c.clone());
    let d = ctx.computed(move || {
        *counter.borrow_mut() += 1;
        b_read.get() + c_read.get()
    });
    assert_eq!(d.get(), 5);
    assert_eq!(*d_evals.borrow(), 1);

    set_a.set(10);
    runtime.stabilize();
    // D ran once, at height 2, with BOTH branches fresh: 20 + 30.
    assert_eq!(d.get(), 50);
    assert_eq!(*d_evals.borrow(), 2, "join node evaluated exactly once");
}

// ─── ThreeState short-circuit ──────────────────────────────────────────────────

#[test]
fn check_state_short_circuits_when_intermediate_value_is_unchanged() {
    let (runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(1);
    let c_evals = cell(0);

    let threshold = ctx.computed(move || a.get() > 10);

    let counter = Rc::clone(&c_evals);
    let threshold_read = threshold.clone();
    let c = ctx.computed(move || {
        *counter.borrow_mut() += 1;
        if threshold_read.get() {
            "over"
        } else {
            "under"
        }
    });
    assert_eq!(c.get(), "under");
    assert_eq!(*c_evals.borrow(), 1);

    // 1 -> 2: threshold re-evaluates but its VALUE stays false; C got Check
    // and resolves to Clean WITHOUT re-evaluating.
    set_a.set(2);
    runtime.stabilize();
    assert_eq!(*c_evals.borrow(), 1, "Check short-circuited");
    assert_eq!(c.get(), "under");

    // 2 -> 11: threshold flips — C re-evaluates.
    set_a.set(11);
    runtime.stabilize();
    assert_eq!(*c_evals.borrow(), 2);
    assert_eq!(c.get(), "over");
}

// ─── Context disposal ──────────────────────────────────────────────────────────

#[test]
fn dropping_a_context_runs_cleanups_and_detaches_nodes() {
    let (runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0);
    let cleanups = cell(0);
    let runs = cell(0);

    let child = ctx.child();
    let cleanup_counter = Rc::clone(&cleanups);
    let run_counter = Rc::clone(&runs);
    let count_read = count.clone();
    let runtime_for_effect = Rc::clone(&runtime);
    child.effect(move || {
        *run_counter.borrow_mut() += 1;
        let _ = count_read.get();
        let c = Rc::clone(&cleanup_counter);
        // Inside an effect: attach to the ACTIVE effect via the runtime.
        let _ = runtime_for_effect.on_cleanup_active(Box::new(move || {
            *c.borrow_mut() += 1;
        }));
    });
    assert_eq!(*runs.borrow(), 1);

    drop(child);
    assert_eq!(*cleanups.borrow(), 1, "cleanup ran at disposal");

    // The disposed effect no longer observes the signal.
    set_count.set(5);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1, "disposed effect does not re-run");
}

#[test]
fn child_disposal_cascades_from_parent() {
    let (runtime, ctx) = setup();
    let parent = ctx.child();
    let grandchild = parent.child();

    let (count, set_count) = ctx.signal(0);
    let runs = cell(0);
    let counter = Rc::clone(&runs);
    grandchild.effect(move || {
        *counter.borrow_mut() += 1;
        let _ = count.get();
    });
    assert_eq!(*runs.borrow(), 1);

    // Dropping the PARENT cascades into the grandchild's nodes...
    drop(parent);
    set_count.set(1);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1, "cascaded disposal detached the effect");

    // ...and the grandchild handle dropping later is a no-op (double dispose).
    drop(grandchild);
}

#[test]
fn context_level_on_cleanup_runs_once_at_drop() {
    let (_runtime, ctx) = setup();
    let cleaned = cell(0);
    let scope = ctx.child();
    let counter = Rc::clone(&cleaned);
    scope.on_cleanup(move || *counter.borrow_mut() += 1);
    assert_eq!(*cleaned.borrow(), 0);
    drop(scope);
    assert_eq!(*cleaned.borrow(), 1);
}

// ─── Callback registry ─────────────────────────────────────────────────────────

#[test]
fn callback_ids_are_monotonic_and_distinct() {
    let (_runtime, ctx) = setup();
    let (_a, set_a) = ctx.signal(0);
    let (_b, set_b) = ctx.signal(0);
    assert_ne!(set_a.callback_id(), set_b.callback_id());
    assert!(set_b.callback_id() > set_a.callback_id());
}

#[test]
fn invoke_callback_updates_the_signal_through_the_default_dispatch() {
    let (runtime, ctx) = setup();
    let (name, set_name) = ctx.signal(String::new());

    // String signals get the G17 default callback at creation.
    let delivered = runtime.invoke_callback(
        set_name.callback_id(),
        EventData::with_value("change", "hello"),
    );
    assert!(delivered);
    assert_eq!(name.get(), "hello");

    // The JSON leg (what the WASM boundary calls).
    let json = r#"{"type":"change","value":"world","primalId":"7"}"#;
    assert!(runtime.invoke_callback_json(set_name.callback_id(), json));
    assert_eq!(name.get(), "world");
}

#[test]
fn numeric_signals_parse_event_values() {
    let (runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0u32);
    assert!(runtime.invoke_callback(set_count.callback_id(), EventData::with_value("input", "42")));
    assert_eq!(count.get(), 42);

    // Type mismatch (unparsable) — set is skipped, value retained.
    assert!(runtime.invoke_callback(
        set_count.callback_id(),
        EventData::with_value("input", "not a number")
    ));
    assert_eq!(count.get(), 42, "bad payload leaves the signal unchanged");
}

#[test]
fn stale_callback_ids_are_silently_dropped() {
    let (runtime, ctx) = setup();
    let scope = ctx.child();
    let (_name, set_name) = scope.signal(String::new());
    let id = set_name.callback_id();
    assert!(runtime.has_callback(id));

    drop(scope);
    assert!(!runtime.has_callback(id), "disposal removes the registration");
    assert!(
        !runtime.invoke_callback(id, EventData::with_value("change", "x")),
        "stale id returns false, no panic"
    );

    // Never-assigned ids are also just dropped.
    assert!(!runtime.invoke_callback(9999, EventData::default()));
}

// ─── stabilize() batching ──────────────────────────────────────────────────────

#[test]
fn multiple_sets_coalesce_into_one_effect_run() {
    let (runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0);
    let seen = cell(Vec::new());
    let log = Rc::clone(&seen);
    ctx.effect(move || log.borrow_mut().push(count.get()));

    set_count.set(1);
    set_count.set(2);
    set_count.set(3);
    runtime.stabilize();
    // One run, observing the LAST value.
    assert_eq!(*seen.borrow(), vec![0, 3]);
}

#[test]
fn effects_run_in_height_order_each_once() {
    let (runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(1);
    let order = cell(Vec::new());

    let b = ctx.computed(move || a.get() * 2); // height 1

    let log_low = Rc::clone(&order);
    let b_for_low = b.clone();
    ctx.effect(move || {
        let _ = b_for_low.get();
        log_low.borrow_mut().push("reader-of-b");
    }); // height 2

    let log_chain = Rc::clone(&order);
    let c = ctx.computed(move || b.get() + 1); // height 2
    ctx.effect(move || {
        let _ = c.get();
        log_chain.borrow_mut().push("reader-of-c");
    }); // height 3

    order.borrow_mut().clear();
    set_a.set(5);
    runtime.stabilize();
    assert_eq!(
        *order.borrow(),
        vec!["reader-of-b", "reader-of-c"],
        "lower heights flush first, each effect once"
    );
}

// ─── Re-entrant set ────────────────────────────────────────────────────────────

#[test]
fn effect_writing_a_signal_propagates_downstream_in_the_same_pass() {
    let (runtime, ctx) = setup();
    let (a, set_a) = ctx.signal(0);
    let (b, set_b) = ctx.signal(0);
    let b_seen = cell(Vec::new());

    // Effect 1 (height 1): reads A, writes B = A * 10.
    let writer = set_b.clone();
    ctx.effect(move || writer.set(a.get() * 10));

    // B's reader: a computed (height 1 over B) + effect (height 2).
    let b_doubled = ctx.computed(move || b.get() * 2);
    let log = Rc::clone(&b_seen);
    ctx.effect(move || log.borrow_mut().push(b_doubled.get()));
    assert_eq!(*b_seen.borrow(), vec![0]);

    set_a.set(7);
    runtime.stabilize();
    // B became 70 mid-pass; its observers (heights >= 1) ran in the SAME pass.
    assert_eq!(*b_seen.borrow(), vec![0, 140]);
}

// ─── Cleanup ordering ──────────────────────────────────────────────────────────

#[test]
fn cleanup_runs_before_each_reevaluation() {
    let (runtime, ctx) = setup();
    let (count, set_count) = ctx.signal(0);
    let order = cell(Vec::new());

    let log = Rc::clone(&order);
    let runtime_for_effect = Rc::clone(&runtime);
    ctx.effect(move || {
        let value = count.get();
        log.borrow_mut().push(format!("run {value}"));
        let inner = Rc::clone(&log);
        let _ = runtime_for_effect.on_cleanup_active(Box::new(move || {
            inner.borrow_mut().push(format!("cleanup after {value}"));
        }));
    });
    assert_eq!(*order.borrow(), vec!["run 0"]);

    set_count.set(1);
    runtime.stabilize();
    assert_eq!(
        *order.borrow(),
        vec!["run 0", "cleanup after 0", "run 1"],
        "cleanup precedes the re-run"
    );
}

// ─── Height recomputation ──────────────────────────────────────────────────────

#[test]
fn dependencies_discovered_later_still_order_correctly() {
    let (runtime, ctx) = setup();
    let (mode, set_mode) = ctx.signal(false);
    let (a, set_a) = ctx.signal(1);
    let evals = cell(Vec::new());

    // C: a chain over A — height 2.
    let a_for_c = a.clone();
    let c_inner = ctx.computed(move || a_for_c.get() + 1); // height 1
    let log_c = Rc::clone(&evals);
    let c = ctx.computed(move || {
        log_c.borrow_mut().push("c");
        c_inner.get() * 10
    }); // height 2

    // D initially reads only A (height 1); after `mode` flips it ALSO reads C,
    // so its height must recompute to 3 and it must see C fresh.
    let log_d = Rc::clone(&evals);
    let d = ctx.computed(move || {
        log_d.borrow_mut().push("d");
        if mode.get() {
            a.get() + c.get()
        } else {
            a.get()
        }
    });
    assert_eq!(d.get(), 1);

    set_mode.set(true);
    runtime.stabilize();
    assert_eq!(d.get(), 1 + 20);

    // Now change A: C (height 2) must evaluate BEFORE D (height 3).
    evals.borrow_mut().clear();
    set_a.set(5);
    runtime.stabilize();
    assert_eq!(d.get(), 5 + 60);
    let order = evals.borrow().clone();
    let c_pos = order.iter().position(|e| *e == "c").unwrap();
    let d_pos = order.iter().rposition(|e| *e == "d").unwrap();
    assert!(c_pos < d_pos, "recomputed height orders C before D: {order:?}");
    assert_eq!(
        order.iter().filter(|e| **e == "d").count(),
        1,
        "D evaluated once: {order:?}"
    );
}

// ─── NotificationManager ───────────────────────────────────────────────────────

#[test]
fn notification_managers_fire_after_stabilize_in_registration_order() {
    let (runtime, ctx) = setup();
    let order = cell(Vec::new());

    let first = Rc::clone(&order);
    runtime.add_notification_manager(Box::new(move || first.borrow_mut().push("first")));
    let second = Rc::clone(&order);
    runtime.add_notification_manager(Box::new(move || second.borrow_mut().push("second")));

    let (_count, set_count) = ctx.signal(0);
    set_count.set(1);
    runtime.stabilize();
    assert_eq!(*order.borrow(), vec!["first", "second"]);
}
