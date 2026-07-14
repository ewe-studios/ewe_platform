//! Integration test for the `Deployable` implementation (Decision 05).
//!
//! Deploys a real container against dockerd, checks it is running, then
//! destroys it — driving the `deploy`/`destroy` `TaskIterator`s the same way a
//! user would (`drive_iterator`). State (the container id) is persisted to a
//! temp `FileStateStore` between the two calls.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test deployable_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::{drive_iterator, initialize_pool, TaskStatus};
use foundation_db::core::state::traits::StateStore;
use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::Deployable;
use foundation_deployment_docker::{ContainerDeployment, DockerClient};
use foundation_netio::http::NativeHttpClient;

/// Drive a `Deployable` task to its terminal `Ready` value.
fn run_to_ready<T>(task: T) -> T::Ready
where
    T: foundation_core::valtron::TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: foundation_core::valtron::ExecutionAction + Send + 'static,
{
    for status in drive_iterator(task) {
        if let TaskStatus::Ready(value) = status {
            return value;
        }
    }
    panic!("task ended without producing a Ready value");
}

/// A `ProviderClient` whose state store is a fresh temp dir. The HTTP client is
/// unused by the Docker deployable (it dials its own Unix socket) but the trait
/// requires one.
fn provider_client() -> (ProviderClient<FileStateStore, foundation_netio::shared::client::dns::SystemDnsResolver>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ewe-deployable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = FileStateStore::new(&dir, "ewe-test", "dev");
    store.init().expect("init state store");
    let http = NativeHttpClient::default();
    (ProviderClient::new("ewe-test", "dev", store, http), dir)
}

#[test]
fn deploy_then_destroy_container() {
    // Skip cleanly if there's no docker daemon.
    if std::os::unix::net::UnixStream::connect("/var/run/docker.sock").is_err() {
        eprintln!("skipping: no docker daemon at /var/run/docker.sock");
        return;
    }

    // The Docker client spawns its HTTP work onto the valtron pool; a plain
    // `#[test]` must stand one up (held for the test's lifetime).
    let _pool = initialize_pool(0, None);

    let (client, state_dir) = provider_client();
    let deployment = ContainerDeployment::new("alpine:latest")
        .with_name("ewe-deployable-it")
        .with_cmd(["sleep", "30"]);

    // Deploy: creates + starts the container, persists its id.
    let task = deployment.deploy(0, client.clone()).expect("build deploy task");
    let output = run_to_ready(task).expect("deploy container");
    assert!(!output.container_id.is_empty(), "deploy should yield a container id");
    eprintln!("DEPLOY OK — container {}", output.container_id);

    // The container should be running.
    let docker = DockerClient::connect_unix("/var/run/docker.sock");
    let inspect = futures::executor::block_on(docker.inspect_container(&output.container_id))
        .expect("inspect deployed container");
    let running = inspect
        .state
        .and_then(|s| s.running)
        .unwrap_or(false);
    assert!(running, "deployed container should be running");

    // Destroy: reads the id back from state, stops + removes it.
    let task = deployment.destroy(0, client).expect("build destroy task");
    run_to_ready(task).expect("destroy container");
    eprintln!("DESTROY OK — container removed");

    // It should be gone now.
    let gone = futures::executor::block_on(docker.inspect_container(&output.container_id)).is_err();
    assert!(gone, "destroyed container should no longer exist");

    let _ = std::fs::remove_dir_all(&state_dir);
}
