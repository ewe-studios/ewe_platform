//! Unit tests for WaitFor strategies.

use std::time::Duration;
use foundation_deployment_platform::docker::WaitFor;

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
    // Default stdout timeout is 30s, port is 10s → total ~40s
    assert!(w.timeout().unwrap() >= Duration::from_secs(40));
}
