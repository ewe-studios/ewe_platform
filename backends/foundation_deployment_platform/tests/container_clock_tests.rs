//! Clock injection for Docker containers (spec 72, feature 04 — issue #220).
//!
//! WHY: a `ContainerConfig` built without an explicit clock used to fall back to
//! the root clock of the valtron pool. That panics when no pool is initialized.
//! The readiness wait must read the clock the caller injected, and a missing
//! clock must be a typed error, never a panic and never the real clock.
//!
//! WHAT: the first test starts a config with no clock and asserts the typed
//! `DockerError::MissingClock`; it never reaches Docker. The second test builds
//! a config with a `SimulatedClock` and proves the wait's deadline and reported
//! elapsed time come from that clock.
//!
//! HOW: no valtron pool is initialized in this binary. The wait runs on
//! `futures_lite::block_on`; a helper thread advances the simulated clock so the
//! retry loop sees its simulated deadline pass.

use std::collections::HashMap;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;
use foundation_deployment_platform::docker::{
    ContainerConfig, ContainerHandle, DockerError, WaitFor, wait_for_port_at,
};

/// A config with no clock does not start. It returns the typed error before it
/// touches Docker, so this test needs neither a daemon nor a pool.
#[test]
fn a_container_without_a_clock_does_not_start() {
    let config = ContainerConfig::new("redis:7-alpine")
        .port(6379)
        .wait(WaitFor::port_with_timeout(6379, Duration::from_secs(1)));

    let err = futures_lite::future::block_on(ContainerHandle::start_async(config))
        .expect_err("a config without a clock must not start");

    assert!(
        matches!(err.current_context(), DockerError::MissingClock),
        "a missing clock must be a typed error, got {err:?}"
    );
}

/// The config keeps the clock the caller injected, and the readiness wait reads
/// it for its deadline and its reported elapsed time.
#[test]
fn a_container_config_uses_the_injected_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let clock: Arc<dyn Clock> = simulated.clone();
    let ctx = Ctx::fixed(Arc::clone(&clock));

    let config = ContainerConfig::new("redis:7-alpine").ctx(ctx.clone());
    let injected = config
        .ctx
        .as_ref()
        .expect("the config must keep the injected context");
    assert!(
        Arc::ptr_eq(&injected.clock(), &clock),
        "the config must hold the exact clock the caller injected"
    );

    // A port nobody listens on: the wait retries until its clock deadline.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind a probe port");
    let port = listener.local_addr().expect("probe address").port();
    drop(listener);

    let mut ports = HashMap::new();
    ports.insert(format!("{port}/tcp"), port);

    let timeout = Duration::from_secs(5);
    let advance = {
        let simulated = Arc::clone(&simulated);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            simulated.advance(timeout);
        })
    };

    let err = futures_lite::future::block_on(wait_for_port_at(
        Ipv4Addr::LOCALHOST,
        port,
        timeout,
        &ports,
        &ctx,
    ))
    .expect_err("the closed port must time out");

    advance
        .join()
        .expect("the clock advance thread must finish");

    match err.current_context() {
        DockerError::WaitTimeout { elapsed, .. } => assert_eq!(
            *elapsed, timeout,
            "the timeout must report the simulated clock's elapsed time, not the real one"
        ),
        other => panic!("expected WaitTimeout, got {other:?}"),
    }
}
