//! Tests for `Runner` execution strategies — Docker-free shape tests.
//!
//! **WHY:** `Runner` fans a command across many hosts.  These tests verify the
//! strategy code paths — result ordering, batching, per-host builder invocation —
//! by pointing at unreachable hosts.  The Docker-backed strategy test lives in
//! `foundation_deployment_platform/tests/runner_integration_tests.rs`.
//!
//! **WHAT:** Parallel/Sequential/Group result counts and error paths; builder
//! invocation proves the closure receives each host.
//!
//! **HOW:** Unreachable-host tests are pure and synchronous.  No container needed.

use std::sync::Arc;
use std::time::Duration;

use foundation_sshkit::{Command, ConnectionPool, Host, Runner, Ssh2Backend};

fn unreachable_hosts(count: usize) -> Vec<Host> {
    (0..count)
        .map(|i| Host::parse(&format!("root@127.0.0.1:{}", i + 1)))
        .collect()
}

#[test]
fn test_parallel_returns_one_result_per_host() {
    let hosts = unreachable_hosts(3);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results = Runner::parallel().run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "one result per host");
    assert!(
        results.iter().all(Result::is_err),
        "every unreachable host should yield a connect error"
    );
}

#[test]
fn test_sequential_returns_one_result_per_host() {
    let hosts = unreachable_hosts(3);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results =
        Runner::sequential(Duration::from_millis(1)).run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "one result per host, in order");
    assert!(results.iter().all(Result::is_err));
}

#[test]
fn test_group_batches_cover_all_hosts() {
    let hosts = unreachable_hosts(5);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results = Runner::group(2).run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "grouping must not drop hosts");
    assert!(results.iter().all(Result::is_err));
}

#[test]
fn test_command_builder_receives_each_host() {
    let hosts = vec![
        Host::parse("root@127.0.0.1:1"),
        Host::parse("root@127.0.0.1:2"),
    ];
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);
    let seen = std::sync::Mutex::new(Vec::new());

    let results = Runner::parallel().run(&hosts, &backend, |h| {
        seen.lock().unwrap().push(h.port);
        Command::new("true")
    });

    assert_eq!(results.len(), 2);
    assert_eq!(*seen.lock().unwrap(), vec![1u16, 2u16], "builder saw both hosts");
}
