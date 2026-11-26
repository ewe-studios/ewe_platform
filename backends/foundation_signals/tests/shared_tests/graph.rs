use foundation_signals::{MultiThreadedRuntime, SingleThreadedRuntime};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Run the same observer contract against the two distinct public scope types.
macro_rules! check_diamond {
    ($runtime:expr) => {{
        let runtime = $runtime;
        let scope = runtime.scope();
        let (read, write) = scope.signal(0);
        let source = read.clone();
        let left = scope.computed(move || source.get() * 2);
        let right = scope.computed(move || read.get() * 3);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        scope.effect(move || log.lock().unwrap().push((left.get(), right.get())));
        write.set(1);
        write.set(3);
        assert_eq!(*seen.lock().unwrap(), [(0, 0)]);
        runtime.stabilize();
        write.set(3);
        runtime.stabilize();
        assert_eq!(*seen.lock().unwrap(), [(0, 0), (6, 9)]);
    }};
}

#[test]
fn both_runtimes_coalesce_a_diamond_and_ignore_equal_writes() {
    check_diamond!(Rc::new(SingleThreadedRuntime::new()));
    check_diamond!(Arc::new(MultiThreadedRuntime::new()));
}

#[test]
fn untracked_reads_restore_the_active_effect_after_nested_reads() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (before, set_before) = scope.signal(0);
    let (ignored, set_ignored) = scope.signal(0);
    let (after, set_after) = scope.signal(0);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let tracker = Arc::clone(&runtime);
    scope.effect(move || {
        let a = before.get();
        let b = tracker.untracked(|| tracker.untracked(|| ignored.get()));
        let c = after.get();
        log.lock().unwrap().push((a, b, c));
    });

    set_ignored.set(9);
    runtime.stabilize();
    set_before.set(1);
    runtime.stabilize();
    set_after.set(2);
    runtime.stabilize();
    assert_eq!(*seen.lock().unwrap(), [(0, 0, 0), (1, 9, 0), (1, 9, 2)]);
}

#[test]
fn switching_computed_dependencies_releases_the_old_input() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (switch, select) = scope.signal(false);
    let (left, write_left) = scope.signal(1);
    let (right, write_right) = scope.signal(2);
    let runs = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&runs);
    let selected = scope.computed(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        if switch.get() {
            right.get()
        } else {
            left.get()
        }
    });

    select.set(true);
    runtime.stabilize();
    assert_eq!(selected.get(), 2);
    write_left.set(4);
    runtime.stabilize();
    assert_eq!(runs.load(Ordering::SeqCst), 2);
    write_right.set(5);
    runtime.stabilize();
    assert_eq!(selected.get(), 5);
}

#[test]
fn effect_writes_another_signal_and_reads_without_holding_graph_locks() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (input, write_input) = scope.signal(2);
    let (output, write_output) = scope.signal(0);
    scope.effect(move || write_output.set(input.get() * 3));
    write_input.set(4);
    runtime.stabilize();
    assert_eq!(output.get(), 12);
}

#[test]
fn switching_to_a_dirty_deeper_computation_observes_the_stabilized_value() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (switch, select) = scope.signal(false);
    let (source, write) = scope.signal(0);
    let plus_one = scope.computed(move || source.get() + 1);
    let times_two = scope.computed(move || plus_one.get() * 2);
    let deeper = scope.computed(move || times_two.get() + 1);
    let selected = scope.computed(move || if switch.get() { deeper.get() } else { 0 });
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    scope.effect(move || log.lock().unwrap().push(selected.get()));

    select.set(true);
    write.set(2);
    runtime.stabilize();
    assert_eq!(*seen.lock().unwrap(), [0, 7]);
}

#[test]
fn nested_initial_evaluation_tracks_inner_and_outer_reads_separately() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let parent = runtime.scope();
    let child = parent.child();
    let (inner, write_inner) = parent.signal(0);
    let (outer, write_outer) = parent.signal(0);
    let (send_outer, outer_values) = std::sync::mpsc::channel();
    let (send_inner, inner_values) = std::sync::mpsc::channel();
    let mut install = Some(child.clone());
    parent.effect(move || {
        if let Some(scope) = install.take() {
            let input = inner.clone();
            let report = send_inner.clone();
            scope.effect(move || report.send(input.get()).unwrap());
        }
        send_outer.send(outer.get()).unwrap();
    });

    write_inner.set(1);
    runtime.stabilize();
    assert_eq!(outer_values.try_iter().collect::<Vec<_>>(), [0]);
    assert_eq!(inner_values.try_iter().collect::<Vec<_>>(), [0, 1]);
    write_outer.set(2);
    runtime.stabilize();
    assert_eq!(outer_values.try_iter().collect::<Vec<_>>(), [2]);
}
