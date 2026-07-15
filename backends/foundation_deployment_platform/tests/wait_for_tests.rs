//! Tests for `WaitFor` strategies — constructors, timeouts, and live readiness loops.

use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use foundation_deployment_platform::docker::WaitFor;

// ── Constructor / timeout (sync, no executor needed) ──────────────────

#[test]
fn test_port_wait_constructors() {
    let w = WaitFor::port(6379);
    assert!(matches!(w, WaitFor::Port { port: 6379, timeout } if timeout == Duration::from_secs(30)));

    let w = WaitFor::port_with_timeout(5432, Duration::from_secs(60));
    assert!(matches!(w, WaitFor::Port { port: 5432, timeout } if timeout == Duration::from_secs(60)));
}

#[test]
fn test_stdout_wait() {
    let w = WaitFor::stdout("Ready to accept connections");
    assert!(matches!(w, WaitFor::Stdout { ref message, .. } if message == "Ready to accept connections"));
}

#[test]
fn test_http_wait() {
    let w = WaitFor::http("http://localhost:8080/health");
    assert!(matches!(w, WaitFor::Http { ref url, .. } if url == "http://localhost:8080/health"));
}

#[test]
fn test_composite_wait() {
    let w = WaitFor::all(vec![
        WaitFor::port(6379),
        WaitFor::stdout("Ready"),
    ]);
    assert!(matches!(w, WaitFor::Composite { ref strategies } if strategies.len() == 2));
}

#[test]
fn test_default_is_none() {
    assert!(matches!(WaitFor::default(), WaitFor::None));
}

#[test]
fn test_timeout_none_is_none() {
    assert_eq!(WaitFor::None.timeout(), None);
}

#[test]
fn test_timeout_port() {
    let w = WaitFor::port_with_timeout(6379, Duration::from_secs(30));
    assert_eq!(w.timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn test_timeout_composite_sums() {
    let w = WaitFor::all(vec![
        WaitFor::port_with_timeout(6379, Duration::from_secs(10)),
        WaitFor::stdout("Ready"),
    ]);
    assert!(w.timeout().unwrap() >= Duration::from_secs(40));
}

// ── Live readiness loops (exercises valtron sleep_async via wait_for_port) ──

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker::wait_for_port;

/// `wait_for_port` succeeds immediately when the port is already listening.
#[valtron_test]
async fn port_wait_succeeds_on_open_port() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let port = addr.port();

    let mut ports = HashMap::new();
    ports.insert(format!("{port}/tcp"), port);

    let start = Instant::now();
    wait_for_port(port, Duration::from_secs(5), &ports)
        .await
        .expect("open port should connect immediately");
    assert!(start.elapsed() < Duration::from_millis(500), "took too long: {:?}", start.elapsed());

    drop(listener);
}

/// `wait_for_port` times out when the port is never opened.
#[valtron_test]
async fn port_wait_times_out_on_closed_port() {
    let bogus_port = 19999u16;
    let mut ports = HashMap::new();
    ports.insert(format!("{bogus_port}/tcp"), bogus_port);

    let start = Instant::now();
    let result = wait_for_port(bogus_port, Duration::from_millis(200), &ports).await;
    assert!(result.is_err(), "should time out");
    assert!(start.elapsed() >= Duration::from_millis(150), "timed out too fast: {start:?} elapsed {elapsed:?}", elapsed = start.elapsed());
}
