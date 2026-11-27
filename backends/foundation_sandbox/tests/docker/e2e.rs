//! The real Docker Engine tests (feature 10, section 7).
//!
//! Each test asks the driver what this host can do. A host without a Docker
//! Engine, or without the slave program, prints the report and returns. The
//! Docker continuous integration (CI) job sets `EWE_SANDBOX_E2E_REQUIRED=1`,
//! so a skipped run fails there.
//!
//! On Linux the tests use the Unix domain socket (UDS) link with
//! `NetworkMode::None`. On another host they use the Transmission Control
//! Protocol (TCP) link with `NetworkMode::Host`, because Docker Desktop does
//! not carry a UDS across its file sharing (section 6.2).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::env;
use foundation_deployment_docker::error::DockerError;
use foundation_deployment_docker::DockerClient;
use foundation_sandbox::native::docker::driver::{
    container_name, DockerDriver, DockerDriverConfig, DockerLink, SlaveMode,
};
use foundation_sandbox::native::record::FileRecordStore;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::{
    spawn_detached, DropPolicy, ManagerConfig, SandboxHandle, SandboxManager, SandboxProvider,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::spec::{NetworkMode, RootFs, SandboxSpec};

/// The link of this host.
fn link() -> DockerLink {
    if cfg!(target_os = "linux") {
        DockerLink::Unix
    } else {
        DockerLink::Tcp
    }
}

/// The network mode that the default link needs.
fn default_network() -> NetworkMode {
    if cfg!(target_os = "linux") {
        NetworkMode::None
    } else {
        NetworkMode::Host
    }
}

/// The configuration of the driver that the tests use.
fn driver_config(mode: SlaveMode) -> DockerDriverConfig {
    DockerDriverConfig::default()
        .with_link(link())
        .with_slave_mode(mode)
}

/// A host that can start Docker sandboxes, or `None` with the reason.
fn usable_host(tag: &str, mode: SlaveMode) -> Option<(Ctx, SandboxHost)> {
    crate::pool::pool();
    let ctx = Ctx::host();
    let effects = env::system(&ctx).ok()?;
    ctx.store(effects);
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut config = HostConfig::discover(|name| std::env::var(name).ok(), &exe_dir);
    // A directory of this test alone, so a stale run directory never mixes
    // with another test.
    config.state_dir =
        std::env::temp_dir().join(format!("ewe-docker-e2e-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&config.state_dir);
    let host = SandboxHost::native(config);
    ctx.store(host.clone());
    let driver = DockerDriver::with_config(driver_config(mode));
    let report = crate::pool::block_on(driver.probe(ctx.clone()));
    if !report.usable {
        assert!(
            std::env::var("EWE_SANDBOX_E2E_REQUIRED").as_deref() != Ok("1"),
            "the CI runner cannot run Docker: {report:?}"
        );
        eprintln!("skip: the host cannot run Docker: {report:?}");
        return None;
    }
    Some((ctx, host))
}

/// The image that the tests start. The default is the pinned image of the
/// feature. A host with the image already local, and no registry, sets
/// `EWE_DOCKER_E2E_IMAGE`.
fn image() -> String {
    std::env::var("EWE_DOCKER_E2E_IMAGE").unwrap_or_else(|_| "alpine:3.22".to_string())
}

/// The sidecar image that serves HTTP on port 80.
fn sidecar_image() -> String {
    std::env::var("EWE_DOCKER_E2E_SIDECAR_IMAGE").unwrap_or_else(|_| "nginx:alpine".to_string())
}

/// Whether an image is ready to use.
///
/// An image of the local store is ready. A missing image is ready only when
/// `EWE_DOCKER_E2E_ALLOW_PULL=1`: the Docker continuous integration (CI) job
/// sets it, and a local run with no registry then skips instead of waiting for
/// a pull that cannot finish.
fn image_ready(image: &str) -> bool {
    let Ok(client) = DockerClient::connect_with_defaults() else {
        return false;
    };
    match crate::pool::block_on(client.image_inspect(image)) {
        Ok(_) => true,
        Err(_) => std::env::var("EWE_DOCKER_E2E_ALLOW_PULL").as_deref() == Ok("1"),
    }
}

/// Prints the reason and returns `true` when an image is not ready.
fn skip_without_images(images: &[&str]) -> bool {
    for image in images {
        if !image_ready(image) {
            eprintln!(
                "skip: the image {image} is not in the local store and a pull is not allowed"
            );
            return true;
        }
    }
    false
}

/// The limits of one e2e spec: a cold image pull must fit the boot deadline.
fn e2e_spec(image: &str, network: NetworkMode) -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::ContainerImage(image.to_string()));
    spec.network = network;
    spec.limits.boot_timeout = Duration::from_secs(300);
    spec
}

/// A manager with this driver.
fn manager(
    ctx: &Ctx,
    host: &SandboxHost,
    mode: SlaveMode,
    store: Arc<dyn foundation_sandbox::shared::control::RecordStore>,
    on_drop: DropPolicy,
) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(DockerDriver::with_config(driver_config(mode)))],
        host.connector.clone(),
        store,
        ManagerConfig {
            on_drop,
            ..ManagerConfig::default()
        },
    ))
}

/// A guard that kills every sandbox that a test made when the test ends.
struct KillOnDrop {
    handles: Mutex<Vec<SandboxHandle>>,
}

impl KillOnDrop {
    fn new() -> Self {
        Self {
            handles: Mutex::new(Vec::new()),
        }
    }

    fn track(&self, handle: &SandboxHandle) {
        self.handles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(handle.clone());
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let handles =
            std::mem::take(&mut *self.handles.lock().unwrap_or_else(PoisonError::into_inner));
        for handle in handles {
            spawn_detached(async move {
                let _ = handle.kill().await;
            });
        }
    }
}

/// Whether a container with this name exists on the Engine.
fn container_exists(name: &str) -> bool {
    let Ok(client) = DockerClient::connect_with_defaults() else {
        return false;
    };
    !matches!(
        crate::pool::block_on(client.inspect_container(name)),
        Err(DockerError::Api { status: 404, .. })
    )
}

#[test]
fn e2e_docker_acceptance() {
    let Some((ctx, host)) = usable_host("acceptance", SlaveMode::Main) else {
        return;
    };
    if skip_without_images(&[&image()]) {
        return;
    }
    let manager = manager(
        &ctx,
        &host,
        SlaveMode::Main,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::default()),
        DropPolicy::KillAll,
    );
    let guard = KillOnDrop::new();
    let sandbox = crate::pool::block_on(manager.create(e2e_spec(&image(), default_network())))
        .expect("the sandbox starts from an image with no slave");
    guard.track(&sandbox);
    let output = crate::pool::block_on(sandbox.run(Command::new(["echo", "hello"])))
        .expect("the command runs");
    assert_eq!(output.stdout, b"hello\n");
    assert_eq!(output.status.exit_code(), Some(0));
    // A file call uses the same slave API as on a microVM.
    crate::pool::block_on(sandbox.write_file(
        "/workspace/f",
        0o644,
        futures::stream::iter([Ok(b"data".to_vec())]),
    ))
    .expect("the write succeeds");
    let read = crate::pool::block_on(sandbox.run(Command::new(["cat", "/workspace/f"])))
        .expect("the file is readable");
    assert_eq!(read.stdout, b"data");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_docker_probe_no_daemon_message() {
    // The probe always names the daemon check, so a caller can see why a host
    // is not usable.
    let Some((ctx, _host)) = usable_host("probe", SlaveMode::Main) else {
        return;
    };
    let report = crate::pool::block_on(DockerDriver::new().probe(ctx));
    assert!(report.checks.iter().any(|check| check.name == "daemon"));
    assert!(report.checks.iter().any(|check| check.name == "slave"));
}

#[cfg(target_os = "linux")]
#[test]
fn e2e_docker_no_network() {
    let Some((ctx, host)) = usable_host("no_network", SlaveMode::Main) else {
        return;
    };
    if skip_without_images(&[&image()]) {
        return;
    }
    let manager = manager(
        &ctx,
        &host,
        SlaveMode::Main,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::default()),
        DropPolicy::KillAll,
    );
    let guard = KillOnDrop::new();
    let sandbox = crate::pool::block_on(manager.create(e2e_spec(&image(), NetworkMode::None)))
        .expect("the sandbox starts with no network");
    guard.track(&sandbox);
    let output =
        crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "ls /sys/class/net"])))
            .expect("the command runs");
    let interfaces: Vec<&str> = std::str::from_utf8(&output.stdout)
        .expect("the output is text")
        .split_whitespace()
        .collect();
    assert_eq!(interfaces, vec!["lo"], "the only interface is lo");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_docker_sidecar() {
    let Some((ctx, host)) = usable_host("sidecar", SlaveMode::Sidecar) else {
        return;
    };
    if skip_without_images(&[&sidecar_image()]) {
        return;
    }
    let manager = manager(
        &ctx,
        &host,
        SlaveMode::Sidecar,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::default()),
        DropPolicy::KillAll,
    );
    let guard = KillOnDrop::new();
    let sandbox =
        crate::pool::block_on(manager.create(e2e_spec(&sidecar_image(), default_network())))
            .expect("the sidecar sandbox starts");
    guard.track(&sandbox);
    let output = crate::pool::block_on(sandbox.run(Command::new([
        "sh",
        "-c",
        "wget -q -O- http://127.0.0.1/ | head -c 15",
    ])))
    .expect("the image server answers");
    assert!(
        output.stdout.starts_with(b"<!DOCTYPE html>"),
        "the image's own server answers: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let comm = crate::pool::block_on(sandbox.run(Command::new(["cat", "/proc/1/comm"])))
        .expect("the command runs");
    assert!(
        String::from_utf8_lossy(&comm.stdout).contains("nginx"),
        "process 1 is the image's main process"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[cfg(target_os = "linux")]
#[test]
fn e2e_docker_sidecar_main_exit() {
    let Some((ctx, host)) = usable_host("sidecar_exit", SlaveMode::Sidecar) else {
        return;
    };
    if skip_without_images(&[&image()]) {
        return;
    }
    let manager = manager(
        &ctx,
        &host,
        SlaveMode::Sidecar,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::default()),
        DropPolicy::KillAll,
    );
    let guard = KillOnDrop::new();
    // The default command of this image exits at once with no input.
    let sandbox = crate::pool::block_on(manager.create(e2e_spec(&image(), default_network())))
        .expect("the sidecar sandbox starts");
    guard.track(&sandbox);
    let id = sandbox.id().clone();
    std::thread::sleep(Duration::from_secs(2));
    // The watcher marks the sandbox Failed, removes the container, then marks
    // it Stopped and removes the record.
    let gone = crate::pool::block_on(manager.get(id.clone())).is_err();
    assert!(gone, "the sandbox is stopped after the main process exits");
    assert!(
        !container_exists(&container_name(&id)),
        "the container is removed"
    );
}

#[test]
fn e2e_docker_restore() {
    let Some((ctx, host)) = usable_host("restore", SlaveMode::Main) else {
        return;
    };
    if skip_without_images(&[&image()]) {
        return;
    }
    let store = Arc::new(
        FileRecordStore::new(Arc::clone(&host.fs), host.config.state_dir.clone())
            .expect("the record store"),
    );
    let first: Arc<dyn foundation_sandbox::shared::control::RecordStore> = store.clone();
    let manager_one = manager(&ctx, &host, SlaveMode::Main, first, DropPolicy::Detach);
    let sandbox = crate::pool::block_on(manager_one.create(e2e_spec(&image(), default_network())))
        .expect("the sandbox starts");
    let id = sandbox.id().clone();
    let guard = KillOnDrop::new();
    guard.track(&sandbox);
    drop(sandbox);
    drop(manager_one);

    // A new control plane process reads the records and finds its container.
    let second: Arc<dyn foundation_sandbox::shared::control::RecordStore> = store;
    let manager_two = manager(&ctx, &host, SlaveMode::Main, second, DropPolicy::KillAll);
    crate::pool::block_on(manager_two.restore()).expect("the restore succeeds");
    let restored = crate::pool::block_on(manager_two.get(id))
        .expect("the restarted control plane finds its sandbox");
    guard.track(&restored);
    let output = crate::pool::block_on(restored.run(Command::new(["echo", "again"])))
        .expect("a command runs in the restored sandbox");
    assert_eq!(output.stdout, b"again\n");
    crate::pool::block_on(restored.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_docker_stop() {
    let Some((ctx, host)) = usable_host("stop", SlaveMode::Main) else {
        return;
    };
    if skip_without_images(&[&image()]) {
        return;
    }
    let manager = manager(
        &ctx,
        &host,
        SlaveMode::Main,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::default()),
        DropPolicy::KillAll,
    );
    let guard = KillOnDrop::new();
    let sandbox = crate::pool::block_on(manager.create(e2e_spec(&image(), default_network())))
        .expect("the sandbox starts");
    guard.track(&sandbox);
    let id: SandboxId = sandbox.id().clone();
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    assert!(
        !container_exists(&container_name(&id)),
        "stop removes the container"
    );
}
