//! Worker channel and registry tests moved out of `src/worker.rs`
//! (spec 70 feature 59, rule 7).
//!
//! `WorkerChannel::queue_arc` was private; it is now public because the
//! registry test must hand the channel queue to `WorkerRegistry`.

use foundation_platform::{WorkerChannel, WorkerRegistry};

#[test]
fn channel_send_and_recv() {
    let (tx, mut rx) = WorkerChannel::<String>::new("test").capacity(4).build();
    assert!(tx.send("hello".into()));
    assert!(tx.send("world".into()));
    assert_eq!(rx.recv(), Some("hello".into()));
    assert_eq!(rx.recv(), Some("world".into()));
}

#[test]
fn channel_full_drops() {
    let (tx, _rx) = WorkerChannel::<i32>::new("full").capacity(2).build();
    assert!(tx.send(1));
    assert!(tx.send(2));
    assert!(!tx.send(3));
}

#[test]
fn channel_name() {
    let (tx, _) = WorkerChannel::<()>::new("sync_v2").build();
    assert_eq!(tx.name(), "sync_v2");
}

#[test]
fn channel_clone_dispatches_to_same_queue() {
    let (tx1, mut rx) = WorkerChannel::<i32>::new("shared").build();
    let tx2 = tx1.clone();
    tx1.send(10);
    tx2.send(20);
    drop(tx1);
    drop(tx2);
    let mut got = Vec::new();
    while let Some(v) = rx.try_recv() {
        got.push(v);
    }
    got.sort_unstable();
    assert_eq!(got, vec![10, 20]);
}

#[test]
fn registry_register_and_send() {
    let reg = WorkerRegistry::new();
    let (tx, mut rx) = WorkerChannel::<String>::new("echo").build();
    reg.register("echo", tx.queue_arc());
    assert!(reg.exists("echo"));
    assert!(reg.send("echo", "ping".to_string()));
    assert_eq!(rx.recv(), Some("ping".into()));
}

#[test]
fn registry_wrong_type_dropped() {
    let reg = WorkerRegistry::new();
    let (tx, _rx) = WorkerChannel::<String>::new("str").build();
    reg.register("str", tx.queue_arc());
    assert!(!reg.send("str", 42i32)); // wrong type
}

#[test]
fn registry_missing_worker() {
    let reg = WorkerRegistry::new();
    assert!(!reg.send("nope", 42i32));
}

#[test]
fn registry_names() {
    let reg = WorkerRegistry::new();
    let (t1, _) = WorkerChannel::<i32>::new("a").build();
    let (t2, _) = WorkerChannel::<String>::new("b").build();
    reg.register("a", t1.queue_arc());
    reg.register("b", t2.queue_arc());
    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn registry_deregister() {
    let reg = WorkerRegistry::new();
    let (tx, _) = WorkerChannel::<i32>::new("tmp").build();
    reg.register("tmp", tx.queue_arc());
    assert!(reg.exists("tmp"));
    reg.deregister("tmp");
    assert!(!reg.exists("tmp"));
}
