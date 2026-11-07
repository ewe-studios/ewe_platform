//! Docker-backed integration test for `DockerProvider` (F10) — exercises the
//! `Provider` trait path (`launch`/`is_running`/`stop`), which uses the
//! *synchronous* `ContainerHandle::start` that `container_integration` (async)
//! does not cover.

use foundation_core::valtron::initialize_pool;
use foundation_deployment_platform::docker::{ContainerConfig, WaitFor};
use foundation_deployment_platform::providers::Provider;
use foundation_deployment_platform::providers::docker::DockerProvider;
use foundation_deployment_platform::testing::docker_available;

#[test]
#[ignore = "requires Docker daemon running"]
fn docker_provider_launch_run_and_stop() {
    // The Docker client is valtron-based — a pool must be live for the ping.
    let _pool = initialize_pool(56, Some(4));
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    let provider = DockerProvider;
    assert_eq!(provider.name(), "docker");

    let config = ContainerConfig::new("redis:7-alpine")
        .clock(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        ))
        .port(6379)
        .wait(WaitFor::stdout("Ready to accept connections"))
        .stop_timeout_secs(3);

    let handle = provider.launch(&config).expect("Provider::launch");
    assert!(
        provider.is_running(&handle),
        "container runs after Provider::launch"
    );
    assert!(
        handle.host_port(6379).unwrap_or(0) > 0,
        "port 6379 is mapped"
    );

    provider.stop(&handle).expect("Provider::stop");
    assert!(
        !provider.is_running(&handle),
        "container stops after Provider::stop"
    );
}
