//! Probe and health state machine tests (spec 82 feature 01, T5).
//!
//! Every transition of the diagram in section 4.4 runs here with a simulated
//! clock and a scripted probe executor.

mod support;

use std::sync::Arc;
use std::time::Duration;

use foundation_registrar::proto::v1::BackendHealth;
use foundation_registrar::{
    CommandError, CommandOutput, CommandRunner, NetworkProber, ProbeCheck, ProbeExecutor,
    ProbeVerdict, Prober, SystemTcpConnector,
};
use support::{entry, probe_spec, probes, FakeExecutor, Sim};

fn running(handle: foundation_registrar::RunHandle) -> foundation_registrar::Discovered {
    foundation_registrar::Discovered::Running {
        version: "1".to_string(),
        handle,
        started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
        restarts: 0,
        container_ip: None,
    }
}

fn stopped() -> foundation_registrar::Discovered {
    foundation_registrar::Discovered::Stopped {
        version: "1".to_string(),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one story follows every state of the diagram"
)]
fn every_health_transition_of_the_diagram() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 256);

    // Startup gates liveness and readiness; the readiness probe drives
    // READY/NOT_READY; liveness has a long period so it stays out of the way.
    let startup = probe_spec(ProbeCheck::Tcp { port: 1 }, 1, 3, 0);
    let liveness = probe_spec(
        ProbeCheck::Grpc {
            port: 1,
            service: String::new(),
        },
        1000,
        1,
        1000,
    );
    let readiness = probe_spec(
        ProbeCheck::Http {
            port: 1,
            path: "/healthz".to_string(),
        },
        1,
        1,
        0,
    );
    let app = entry(
        "app",
        "1",
        "docker",
        None,
        Some(probes(Some(startup), Some(liveness), Some(readiness))),
    );

    // NotRunning: discovery found no process.
    let outcome = futures_lite::future::block_on(prober.observe(&app, &stopped(), "127.0.0.1"));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_RUNNING);

    // NotRunning -> Starting: a running app with a startup probe that has not
    // passed yet.
    executor.push_failure("starting");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(10)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_STARTING);

    // Starting -> FailedToStart after the failure threshold.
    sim.advance(Duration::from_secs(1));
    executor.push_failure("still starting");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(10)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_STARTING);
    sim.advance(Duration::from_secs(1));
    executor.push_failure("did not start");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(10)),
        "127.0.0.1",
    ));
    assert_eq!(
        outcome.health,
        BackendHealth::BACKEND_HEALTH_FAILED_TO_START
    );
    assert_ne!(outcome.detail, "");

    // FailedToStart -> Starting on a restart (a new PID resets the machine).
    executor.push_failure("starting again");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(11)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_STARTING);

    // Starting -> Ready on a second restart: startup passes, then readiness
    // passes in the same observation.
    executor.push_pass(); // startup
    executor.push_pass(); // readiness
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(12)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);

    // Ready -> NotReady: readiness fails its threshold.
    sim.advance(Duration::from_secs(1));
    executor.push_failure("readiness failed");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(12)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_READY);

    // NotReady -> Ready: readiness passes again.
    sim.advance(Duration::from_secs(1));
    executor.push_pass();
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(12)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);

    // A restart on a new container starts again at STARTING and becomes READY
    // in the same observation when both probes pass.
    executor.push_pass();
    executor.push_pass();
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::container("new-container")),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);

    // Starting -> NotReady when startup passes but readiness fails.
    executor.push_pass();
    executor.push_failure("not ready");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::container("third-container")),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_READY);

    // Ready/NotReady -> NotRunning when the app stops.
    let outcome = futures_lite::future::block_on(prober.observe(&app, &stopped(), "127.0.0.1"));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_RUNNING);
}

#[test]
fn liveness_failures_give_not_live_and_recovery_gives_ready() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 256);
    let liveness = probe_spec(
        ProbeCheck::Grpc {
            port: 1,
            service: String::new(),
        },
        1,
        1,
        0,
    );
    let app = entry(
        "live",
        "1",
        "systemd",
        None,
        Some(probes(None, Some(liveness), None)),
    );

    executor.push_failure("liveness failed");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_LIVE);

    sim.advance(Duration::from_secs(1));
    executor.push_pass();
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);

    sim.advance(Duration::from_secs(1));
    executor.push_failure("stuck");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_LIVE);

    sim.advance(Duration::from_secs(1));
    executor.push_pass();
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);
}

#[test]
fn an_app_without_a_probes_block_is_ready_when_it_runs() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 256);
    let app = entry("plain", "1", "systemd", None, None);
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_READY);
    assert!(executor.calls().is_empty(), "no probe runs");
}

#[test]
fn an_initial_delay_holds_the_first_check() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 256);
    let readiness = probe_spec(ProbeCheck::Tcp { port: 1 }, 10, 1, 30);
    let app = entry(
        "delayed",
        "1",
        "systemd",
        None,
        Some(probes(None, None, Some(readiness))),
    );
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_READY);
    assert!(
        executor.calls().is_empty(),
        "the initial delay holds the check"
    );
}

#[test]
fn a_probe_timeout_fails_that_check_with_its_timeout() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 256);
    let readiness = probe_spec(ProbeCheck::Tcp { port: 1 }, 10, 1, 0);
    let app = entry(
        "slow",
        "1",
        "systemd",
        None,
        Some(probes(None, None, Some(readiness))),
    );
    executor.push_failure("TCP probe timed out after 1s");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert_eq!(outcome.health, BackendHealth::BACKEND_HEALTH_NOT_READY);
    assert!(outcome.detail.contains("timed out"));
    let calls = executor.calls();
    assert_eq!(calls[0].2, Duration::from_secs(1), "the spec timeout");
}

#[test]
fn the_detail_is_cut_to_the_configured_limit() {
    let sim = Sim::new();
    let executor = Arc::new(FakeExecutor::new());
    let mut prober = Prober::new(sim.ctx.clone(), executor.clone(), 8);
    let readiness = probe_spec(ProbeCheck::Tcp { port: 1 }, 10, 1, 0);
    let app = entry(
        "loud",
        "1",
        "systemd",
        None,
        Some(probes(None, None, Some(readiness))),
    );
    executor.push_failure("a failure text that is much longer than eight bytes");
    let outcome = futures_lite::future::block_on(prober.observe(
        &app,
        &running(support::process(1)),
        "127.0.0.1",
    ));
    assert!(outcome.detail.len() <= 8, "detail was {:?}", outcome.detail);
}

#[test]
fn the_verdict_type_is_public_with_its_three_states() {
    assert_ne!(ProbeVerdict::Unknown, ProbeVerdict::Passing);
    assert_ne!(ProbeVerdict::Passing, ProbeVerdict::Failing);
}

/// A command runner whose program never returns.
struct HangingCommands;

#[async_trait::async_trait]
impl CommandRunner for HangingCommands {
    async fn run(&self, program: &str, _args: &[String]) -> Result<CommandOutput, CommandError> {
        std::thread::sleep(Duration::from_secs(3600));
        Err(CommandError {
            program: program.to_string(),
            reason: "unreachable".to_string(),
        })
    }
}

#[test]
fn a_hanging_exec_probe_fails_its_check_without_delaying_another() {
    let ctx = support::host_ctx();
    let prober = NetworkProber::with_clients(
        ctx,
        foundation_netio::HttpClientBuilder::new().build(),
        Arc::new(SystemTcpConnector),
        Arc::new(HangingCommands),
        false,
    );
    let error = futures_lite::future::block_on(prober.run(
        &ProbeCheck::Exec {
            command: vec!["hangs".to_string()],
        },
        "127.0.0.1",
        Duration::from_millis(50),
    ))
    .expect_err("the timeout fails the check");
    assert!(error.contains("timed out"), "{error}");
}
