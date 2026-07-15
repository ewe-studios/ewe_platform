//! Integration test for the `Deployable` implementation.
//!
//! Deploys a real container against dockerd, checks it is running, then
//! destroys it — `deploy`/`destroy` are async (`BoxFuture`), so the test just
//! `.await`s them. State (the container id) is persisted to a temp
//! `FileStateStore` between the two calls.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test deployable_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStore;
use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::Deployable;
use foundation_deployment_docker::{ContainerDeployment, DockerClient};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::dns::SystemDnsResolver;

const DOCKER_SOCK: &str = "/var/run/docker.sock";
const CONTAINER_NAME: &str = "ewe-deployable-it";

/// A `ProviderClient` whose state store is a fresh temp dir. The HTTP client is
/// unused by the Docker deployable (it dials its own Unix socket) but the trait
/// requires one.
fn provider_client() -> (
    ProviderClient<FileStateStore, SystemDnsResolver>,
    std::path::PathBuf,
) {
    let dir = std::env::temp_dir().join(format!("ewe-deployable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = FileStateStore::new(&dir, "ewe-test", "dev");
    store.init().expect("init state store");
    let http = NativeHttpClient::default();
    (ProviderClient::new("ewe-test", "dev", store, http), dir)
}

#[valtron_test]
async fn deploy_then_destroy_container() {
    // Skip cleanly if there's no docker daemon.
    if std::os::unix::net::UnixStream::connect(DOCKER_SOCK).is_err() {
        eprintln!("skipping: no docker daemon at {DOCKER_SOCK}");
        return;
    }

    let docker = DockerClient::connect_unix(DOCKER_SOCK);
    // Robustness: remove any container this test left behind on a prior run so a
    // stale name can never cause a 409 conflict on create.
    let _ = docker.remove_container(CONTAINER_NAME, true).await;

    let (client, state_dir) = provider_client();
    let deployment = ContainerDeployment::new("alpine:latest")
        .with_name(CONTAINER_NAME)
        .with_cmd(["sleep", "30"]);

    // Deploy: creates + starts the container, persists its id.
    let output = deployment
        .deploy(0, client.clone())
        .await
        .expect("deploy container");
    assert!(!output.container_id.is_empty(), "deploy should yield a container id");
    eprintln!("DEPLOY OK — container {}", output.container_id);

    // The container should be running.
    let inspect = docker
        .inspect_container(&output.container_id)
        .await
        .expect("inspect deployed container");
    let running = inspect.state.and_then(|s| s.running).unwrap_or(false);
    assert!(running, "deployed container should be running");

    // Destroy: reads the id back from state, stops + removes it.
    deployment.destroy(0, client).await.expect("destroy container");
    eprintln!("DESTROY OK — container removed");

    // It should be gone now.
    let gone = docker.inspect_container(&output.container_id).await.is_err();
    assert!(gone, "destroyed container should no longer exist");

    let _ = std::fs::remove_dir_all(&state_dir);
}
