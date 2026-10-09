//! End-to-end tests for F03 — networking and volumes (Decisions 04 and 10).
//!
//! WHY: `NetworkHandle` had no consumer and no test anywhere in the workspace,
//! and the container-level networking/volume options were only ever asserted at
//! the builder level (`config_tests`) — nothing proved Docker honoured them.
//! That hid four defects: aliases, named volumes, read-only mounts and the
//! `subnet` argument were all silently dropped, and a container naming a network
//! that did not exist yet failed to start.
//!
//! WHAT is proven, against a real daemon:
//! - a network is created (honouring an explicit subnet), found idempotently,
//!   and removed;
//! - two containers on it resolve each other by **container name** and by
//!   **network alias**, through Docker's embedded DNS — the service-discovery
//!   story Decision 10 is for;
//! - `NetworkHandle::connect` attaches a running container with aliases;
//! - a bind mount is readable inside the container, a read-only mount really is
//!   read-only, and a named volume persists data across two containers.
//!
//! HOW: `docker exec`-free — each assertion runs a command *as the container's
//! own command* and reads its stdout via the logs API, so the tests need nothing
//! but the Docker socket.
//!
//! Run: `cargo test -p foundation_deployment_platform --test
//! network_volume_integration -- --ignored` (needs a Docker daemon).

use std::path::PathBuf;

use foundation_core::valtron::initialize_pool;
use foundation_deployment_platform::docker::{
    ContainerConfig, ContainerHandle, DockerClient, NetworkHandle, WaitFor,
};

/// Unique-ish suffix so parallel runs never collide on a global Docker name.
fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", std::process::id())
}

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

fn client() -> DockerClient {
    DockerClient::connect_with_defaults().expect("docker client")
}

/// Runs `sh -c <script>` in a container on `network`, waits for it to exit, and
/// returns its stdout. The container is removed before returning.
async fn run_script(network: Option<&str>, script: &str) -> String {
    let mut cfg = ContainerConfig::new("alpine:3")
        .command(vec!["sh".into(), "-c".into(), script.into()])
        .stop_timeout_secs(2);
    if let Some(net) = network {
        cfg = cfg.network(net);
    }

    let handle = ContainerHandle::start_async(cfg)
        .await
        .expect("script container should start");
    handle
        .wait_for_exit_async()
        .await
        .expect("script container should exit and yield logs")
}

#[test]
#[ignore = "requires Docker daemon running"]
fn network_dns_resolves_container_name_and_alias() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        let docker = client();
        let net_name = unique("ewe-f03-dns");

        // An explicit subnet must be honoured — it used to be ignored.
        let net = NetworkHandle::create_or_find(&docker, &net_name, Some("172.31.240.0/24"))
            .await
            .expect("create network")
            .remove_on_drop();

        // Idempotent: a second create_or_find finds the same network.
        let again = NetworkHandle::create_or_find(&docker, &net_name, None)
            .await
            .expect("create_or_find is idempotent");
        assert_eq!(again.id(), net.id(), "should find the existing network");

        // The subnet reached Docker.
        let inspect = docker.network_inspect(net.id(), None, None).await.expect("inspect");
        let json = serde_json::to_value(&inspect).expect("inspect json");
        let subnet = json["IPAM"]["Config"][0]["Subnet"].as_str().unwrap_or_default();
        assert_eq!(subnet, "172.31.240.0/24", "explicit subnet should be applied");

        // A server container on the network, answering to an alias as well as
        // its own name.
        let server_name = unique("ewe-f03-server");
        let server = ContainerHandle::start_async(
            ContainerConfig::new("redis:7-alpine")
                .name(&server_name)
                .network(&net_name)
                .network_alias("cache-alias")
                .port(6379)
                .wait(WaitFor::stdout("Ready to accept connections"))
                .stop_timeout_secs(2),
        )
        .await
        .expect("server container should start");

        // Resolve by container name, from a second container on the network.
        let by_name = run_script(
            Some(&net_name),
            &format!("getent hosts {server_name} | head -1"),
        )
        .await;
        assert!(
            by_name.contains(&server_name),
            "embedded DNS should resolve the container name, got: {by_name:?}"
        );

        // Resolve by network alias — this silently did nothing before.
        let by_alias = run_script(Some(&net_name), "getent hosts cache-alias | head -1").await;
        assert!(
            by_alias.contains("cache-alias"),
            "embedded DNS should resolve the network alias, got: {by_alias:?}"
        );

        drop(server);
        drop(again);
        // `net` removes itself on drop.
    });
}

#[test]
#[ignore = "requires Docker daemon running"]
fn network_connect_attaches_running_container_with_alias() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        let docker = client();
        let net_name = unique("ewe-f03-connect");
        let net = NetworkHandle::create_or_find(&docker, &net_name, None)
            .await
            .expect("create network")
            .remove_on_drop();

        // Started with no network, then attached after the fact.
        let server = ContainerHandle::start_async(
            ContainerConfig::new("redis:7-alpine")
                .name(unique("ewe-f03-late"))
                .port(6379)
                .wait(WaitFor::stdout("Ready to accept connections"))
                .stop_timeout_secs(2),
        )
        .await
        .expect("server should start");

        net.connect(server.id(), &["late-alias"])
            .await
            .expect("connect should attach the container with its alias");

        let resolved = run_script(Some(&net_name), "getent hosts late-alias | head -1").await;
        assert!(
            resolved.contains("late-alias"),
            "alias from connect() should resolve, got: {resolved:?}"
        );

        drop(server);
    });
}

#[test]
#[ignore = "requires Docker daemon running"]
fn bind_mount_is_readable_and_read_only_is_enforced() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    // A host dir with a known file.
    let dir: PathBuf = std::env::temp_dir().join(unique("ewe-f03-bind"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create host dir");
    std::fs::write(dir.join("hello.txt"), b"from-the-host").expect("write host file");

    let mount_dir = dir.clone();
    foundation_core::valtron::block_on_future(async move {
        let dir = mount_dir;
        // Read-write bind: the file is visible inside the container.
        let handle = ContainerHandle::start_async(
            ContainerConfig::new("alpine:3")
                .volume(&dir, "/data")
                .command(vec!["sh".into(), "-c".into(), "cat /data/hello.txt".into()])
                .stop_timeout_secs(2),
        )
        .await
        .expect("bind-mount container should start");
        let out = handle.wait_for_exit_async().await.expect("logs");
        assert!(
            out.contains("from-the-host"),
            "bind-mounted file should be readable inside the container, got: {out:?}"
        );
        drop(handle);

        // Read-only bind: a write must fail. `read_only` used to be dropped, so
        // this mount was silently writable.
        let ro = ContainerHandle::start_async(
            ContainerConfig::new("alpine:3")
                .volume_read_only(&dir, "/data")
                .command(vec![
                    "sh".into(),
                    "-c".into(),
                    "touch /data/nope 2>&1 || echo WRITE_REFUSED".into(),
                ])
                .stop_timeout_secs(2),
        )
        .await
        .expect("read-only container should start");
        let ro_out = ro.wait_for_exit_async().await.expect("logs");
        assert!(
            ro_out.contains("WRITE_REFUSED") || ro_out.contains("Read-only"),
            "a read-only mount must refuse writes, got: {ro_out:?}"
        );
        drop(ro);

        // The host file system was not modified by the refused write.
        assert!(
            !dir.join("nope").exists(),
            "the refused write must not have reached the host"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "requires Docker daemon running"]
fn named_volume_persists_data_across_containers() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        let vol = unique("ewe-f03-vol");

        // Writer: puts a file in the named volume, then exits. Named volumes
        // used to be dropped entirely, so this wrote to the container's own
        // layer and the reader below saw nothing.
        let writer = ContainerHandle::start_async(
            ContainerConfig::new("alpine:3")
                .named_volume(&vol, "/vol")
                .command(vec![
                    "sh".into(),
                    "-c".into(),
                    "echo persisted-value > /vol/state.txt".into(),
                ])
                .stop_timeout_secs(2),
        )
        .await
        .expect("writer should start");
        writer.wait_for_exit_async().await.expect("writer logs");
        drop(writer);

        // Reader: a *different* container mounting the same named volume.
        let reader = ContainerHandle::start_async(
            ContainerConfig::new("alpine:3")
                .named_volume(&vol, "/vol")
                .command(vec!["sh".into(), "-c".into(), "cat /vol/state.txt".into()])
                .stop_timeout_secs(2),
        )
        .await
        .expect("reader should start");
        let out = reader.wait_for_exit_async().await.expect("reader logs");
        assert!(
            out.contains("persisted-value"),
            "a named volume must carry data between containers, got: {out:?}"
        );
        drop(reader);

        // Clean up the volume we created.
        let docker = client();
        let _ = docker.volume_delete(&vol, Some(true)).await;
    });
}
