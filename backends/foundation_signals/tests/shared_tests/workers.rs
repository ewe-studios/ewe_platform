use super::receive;
use foundation_signals::MultiThreadedRuntime;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Barrier, Mutex};
use std::thread;

#[derive(Clone, Default, Debug, PartialEq)]
struct Todos {
    last: u64,
    items: Vec<(u64, &'static str)>,
}

#[test]
fn concurrent_writers_receive_distinct_committed_snapshots() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (read, write) = scope.signal(Todos::default());
    let start = Arc::new(Barrier::new(2));
    let workers = ["Milk", "Bread"].map(|title| {
        let start = Arc::clone(&start);
        let write = write.clone();
        thread::spawn(move || {
            start.wait();
            let snapshot = write.update_with(|state| {
                state.last += 1;
                state.items.push((state.last, title));
                state.clone()
            });
            assert_eq!(snapshot.items.last(), Some(&(snapshot.last, title)));
            snapshot
        })
    });
    let mut snapshots = workers.map(|worker| worker.join().unwrap());
    snapshots.sort_by_key(|snapshot| snapshot.last);
    assert_eq!(snapshots[0].last, 1);
    assert_eq!(snapshots[0].items.len(), 1);
    assert_eq!(snapshots[1].last, 2);
    assert_eq!(snapshots[1].items.len(), 2);
    assert_eq!(snapshots[1].items[..1], snapshots[0].items);
    assert_eq!(read.get(), snapshots[1]);
}

#[test]
fn writer_panic_restores_prior_value_and_allows_future_updates() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (read, write) = scope.signal(0);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    scope.effect(move || log.lock().unwrap().push(read.get()));

    let panic = catch_unwind(AssertUnwindSafe(|| {
        write.update_with(|value| {
            *value = 3;
            panic!("test interrupted write");
        });
    }));
    assert!(panic.is_err());
    runtime.stabilize();
    assert_eq!(*seen.lock().unwrap(), [0]);
    assert_eq!(
        write.update_with(|v| {
            *v += 5;
            *v
        }),
        5
    );
    runtime.stabilize();
    assert_eq!(*seen.lock().unwrap(), [0, 5]);
}

#[test]
fn a_fallible_return_value_does_not_roll_back_a_completed_write() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (read, write) = scope.signal(0);
    let result = write.update_with(|state| {
        *state = 7;
        Err::<(), _>("application result")
    });
    assert_eq!(result, Err("application result"));
    assert_eq!(read.get(), 7);
}

#[test]
fn a_write_during_an_effect_is_observed() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (source, write_source) = scope.signal(0);
    let (entered, running) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    scope.effect(move || {
        let value = source.get();
        log.lock().unwrap().push(value);
        if value == 1 {
            entered.send(()).unwrap();
            receive(&resume);
        }
    });

    write_source.set(1);
    let worker_runtime = Arc::clone(&runtime);
    let worker = thread::spawn(move || worker_runtime.stabilize());
    receive(&running);
    write_source.set(2);
    release.send(()).unwrap();
    worker.join().unwrap();
    runtime.stabilize();
    assert_eq!(*seen.lock().unwrap(), [0, 1, 2]);
}

#[test]
fn disposal_defers_cleanup_until_the_running_effect_returns() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let child = scope.child();
    let (source, write) = scope.signal(0);
    let finished = Arc::new(AtomicUsize::new(0));
    let runs = Arc::clone(&finished);
    let (entered, running) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (cleanup, cleaned) = mpsc::channel();
    let finished_runs = Arc::clone(&finished);
    child.on_cleanup(move || cleanup.send(finished_runs.load(Ordering::SeqCst)).unwrap());
    child.effect(move || {
        if source.get() == 1 {
            entered.send(()).unwrap();
            receive(&resume);
        }
        runs.fetch_add(1, Ordering::SeqCst);
    });
    write.set(1);
    let owner = Arc::clone(&runtime);
    let worker = thread::spawn(move || owner.stabilize());
    receive(&running);
    child.dispose();
    assert_eq!(cleaned.try_recv(), Err(mpsc::TryRecvError::Empty));
    write.set(2);
    release.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(receive(&cleaned), 2);
    child.dispose();
    runtime.stabilize();
    assert_eq!(finished.load(Ordering::SeqCst), 2);
    assert_eq!(cleaned.try_recv(), Err(mpsc::TryRecvError::Disconnected));
}

#[test]
fn competing_stabilizations_serialize_callbacks() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (first, write_first) = scope.signal(false);
    let (second, write_second) = scope.signal(false);
    let active = Arc::new(AtomicUsize::new(0));
    let active_first = Arc::clone(&active);
    let (entered, running) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    scope.effect(move || {
        if first.get() {
            assert_eq!(active_first.fetch_add(1, Ordering::SeqCst), 0);
            entered.send(()).unwrap();
            receive(&resume);
            active_first.fetch_sub(1, Ordering::SeqCst);
        }
    });
    let second_runs = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&second_runs);
    let active_second = Arc::clone(&active);
    scope.effect(move || {
        if second.get() {
            assert_eq!(active_second.fetch_add(1, Ordering::SeqCst), 0);
            observed.fetch_add(1, Ordering::SeqCst);
            active_second.fetch_sub(1, Ordering::SeqCst);
        }
    });

    write_first.set(true);
    let first_runtime = Arc::clone(&runtime);
    let first_worker = thread::spawn(move || first_runtime.stabilize());
    receive(&running);
    write_second.set(true);
    let (started, ready) = mpsc::channel();
    let second_runtime = Arc::clone(&runtime);
    let second_worker = thread::spawn(move || {
        started.send(()).unwrap();
        second_runtime.stabilize();
    });
    receive(&ready);
    release.send(()).unwrap();
    first_worker.join().unwrap();
    second_worker.join().unwrap();
    runtime.stabilize();
    assert_eq!(second_runs.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
struct Article {
    text: &'static str,
    views: u64,
}

impl PartialEq for Article {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

#[test]
fn writes_commit_unobserved_metadata_without_notifying() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (read, write) = scope.signal(Article {
        text: "News",
        views: 0,
    });
    let observed = read.clone();
    let runs = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&runs);
    scope.effect(move || {
        assert_eq!(observed.get().text, "News");
        counter.fetch_add(1, Ordering::SeqCst);
    });

    let views = write.update_with(|article| {
        article.views += 1;
        article.views
    });
    assert_eq!(views, 1);
    assert_eq!(read.get().views, 1);
    runtime.stabilize();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[test]
fn a_read_on_another_thread_cannot_subscribe_the_active_effect() {
    let runtime = Arc::new(MultiThreadedRuntime::new());
    let scope = runtime.scope();
    let (source, write_source) = scope.signal(0);
    let (unrelated, write_unrelated) = scope.signal(0);
    let (entered, running) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let runs = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&runs);
    scope.effect(move || {
        let value = source.get();
        let previous = counter.fetch_add(1, Ordering::SeqCst);
        if previous == 1 && value == 1 {
            entered.send(()).unwrap();
            receive(&resume);
        }
    });

    write_source.set(1);
    let owner = Arc::clone(&runtime);
    let worker = thread::spawn(move || owner.stabilize());
    receive(&running);
    assert_eq!(unrelated.get(), 0);
    release.send(()).unwrap();
    worker.join().unwrap();
    write_unrelated.set(2);
    runtime.stabilize();
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}
