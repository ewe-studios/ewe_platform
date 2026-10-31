//! Rule-8 config tests for the reactor drain poll timeout (feature 32).

use std::time::Duration;

use foundation_reactor::{BackendPreference, ReactorConfig, DEFAULT_DRAIN_POLL_TIMEOUT};

#[test]
fn default_drain_poll_timeout_is_100_ms() {
    assert_eq!(DEFAULT_DRAIN_POLL_TIMEOUT, Duration::from_millis(100));
    assert_eq!(ReactorConfig::default().drain_poll_timeout(), Duration::from_millis(100));
}

#[test]
fn builder_round_trips() {
    let config = ReactorConfig::default().with_drain_poll_timeout(Duration::from_millis(250));
    assert_eq!(config.drain_poll_timeout(), Duration::from_millis(250));
}

/// The config value reaches the shared reactor. This is the only test in this
/// binary that touches the singleton, so it always initialises the reactor
/// itself (first-call-wins in a fresh process) and the getter reflects the
/// non-default value it asked for.
#[test]
fn init_with_config_stores_the_timeout() {
    let reactor = foundation_reactor::Reactor::init_with_config(
        BackendPreference::Auto,
        ReactorConfig::default().with_drain_poll_timeout(Duration::from_millis(250)),
    )
    .expect("reactor initialises");

    assert_eq!(reactor.drain_poll_timeout(), Duration::from_millis(250));
}
