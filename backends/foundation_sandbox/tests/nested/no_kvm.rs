//! The no-KVM host form of the nested report (issue #972).
//!
//! The Docker daemon on this host has no `/dev/kvm` (for example Docker
//! Desktop on macOS). The parent container therefore starts without the
//! device, and its inner Firecracker probe must report the missing Kernel-based
//! Virtual Machine (KVM). The test skips when the daemon has the device or the
//! nested host image is absent.
//!
//! # How to run it
//!
//! 1. Build the nested image for the daemon's architecture. On Linux,
//!    `mise run sandbox:nested-image` builds the pinned `linux/amd64` image.
//!    On Apple silicon, build `linux/arm64` and pass the aarch64 Firecracker
//!    SHA-256: `docker build --platform linux/arm64 --build-arg
//!    FIRECRACKER_SHA256=<aarch64 sha> -t ewe/sandbox-nested:latest
//!    infrastructure/sandbox_nested_image`.
//! 2. Set `EWE_NESTED_STATE_DIR` to a directory that Docker can bind-mount
//!    (Docker Desktop shares the user's home by default, for example
//!    `EWE_NESTED_STATE_DIR="$HOME/.cache/ewe-nested"`). The parent's control
//!    socket lives there.
//! 3. Run
//!    `cargo test -p foundation_sandbox --features "docker testing"
//!    --test mod e2e_nested_no_kvm_host -- --nocapture`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::env;
use foundation_deployment_docker::DockerClient;
use foundation_sandbox::native::connect::ConnectAddr;
use foundation_sandbox::native::docker::driver::{
    DockerDriver, DockerDriverConfig, DockerLink, SlaveMode,
};
use foundation_sandbox::shared::control::{
    spawn_detached, ManagerConfig, MemoryRecordStore, SandboxHandle, SandboxManager,
    SandboxProvider, SandboxRpcClient,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::spec::{NetworkMode, RootFs, SandboxSpec};

/// The nested host image. `mise run sandbox:nested-image` builds it.
fn image() -> String {
    std::env::var("EWE_NESTED_IMAGE").unwrap_or_else(|_| "ewe/sandbox-nested:latest".to_string())
}

/// Whether the CI job requires the test to run.
fn required() -> bool {
    std::env::var("EWE_SANDBOX_E2E_REQUIRED").as_deref() == Ok("1")
}

/// A host with a directory of this test alone.
fn usable_host(tag: &str) -> Option<(Ctx, SandboxHost)> {
    crate::pool::pool();
    let ctx = Ctx::host();
    let effects = env::system(&ctx).ok()?;
    ctx.store(effects);
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut config = HostConfig::discover(|name| std::env::var(name).ok(), &exe_dir);
    let base =
        std::env::var_os("EWE_NESTED_STATE_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    config.state_dir = base.join(format!("ewe-nested-no-kvm-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&config.state_dir);
    let host = SandboxHost::native(config);
    ctx.store(host.clone());
    Some((ctx, host))
}

/// The Docker driver configuration. The Unix link works on both hosts.
fn driver_config() -> DockerDriverConfig {
    DockerDriverConfig::default()
        .with_link(DockerLink::Unix)
        .with_slave_mode(SlaveMode::Main)
}

/// Whether the image is ready to use. A missing image needs
/// `EWE_DOCKER_E2E_ALLOW_PULL=1`, as in the Docker end-to-end tests.
fn image_ready(image: &str) -> bool {
    let Ok(client) = DockerClient::connect_with_defaults() else {
        return false;
    };
    match crate::pool::block_on(client.image_inspect(image)) {
        Ok(_) => true,
        Err(_) => std::env::var("EWE_DOCKER_E2E_ALLOW_PULL").as_deref() == Ok("1"),
    }
}

/// The outer manager: the Docker driver and the nested driver.
fn manager(ctx: &Ctx, host: &SandboxHost) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::with_nested_driver(
        ctx.clone(),
        vec![Arc::new(DockerDriver::with_config(driver_config()))],
        host.connector.clone(),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ))
}

/// A parent sandbox that is a nested host.
fn parent_spec() -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::ContainerImage(image()));
    spec.driver = Some(DriverId::Docker);
    spec.nested_host = true;
    spec.network = NetworkMode::None;
    spec.limits.boot_timeout = Duration::from_secs(300);
    spec
}

/// A guard that kills the parent when the test ends.
struct KillOnDrop {
    handles: Mutex<Vec<SandboxHandle>>,
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

/// A daemon without `/dev/kvm` starts the parent without the device, and the
/// inner Firecracker probe reports the failed `kvm` check.
#[test]
fn e2e_nested_no_kvm_host() {
    let Some((ctx, host)) = usable_host("no-kvm-host") else {
        return;
    };
    // The daemon probe names whether its host has the device.
    let probe =
        crate::pool::block_on(DockerDriver::with_config(driver_config()).probe(ctx.clone()));
    if !probe.usable {
        assert!(!required(), "the CI runner cannot run Docker: {probe:?}");
        eprintln!("skip: the host cannot run Docker: {probe:?}");
        return;
    }
    let daemon_has_kvm = probe
        .checks
        .iter()
        .find(|check| check.name == "kvm_for_nested")
        .is_some_and(|check| check.ok);
    if daemon_has_kvm {
        eprintln!(
            "skip: the Docker daemon's host has /dev/kvm; the no-KVM host form does not apply"
        );
        return;
    }
    if !image_ready(&image()) {
        assert!(!required(), "the CI runner has no image {}", image());
        eprintln!(
            "skip: the image {} is not in the local store and a pull is not allowed",
            image()
        );
        return;
    }

    let manager = manager(&ctx, &host);
    let parent = crate::pool::block_on(manager.create(parent_spec()))
        .expect("the parent starts without /dev/kvm");
    let id: SandboxId = parent.id().clone();
    let guard = KillOnDrop {
        handles: Mutex::new(Vec::new()),
    };
    guard
        .handles
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(parent.clone());

    assert_eq!(
        parent.info().slave.as_ref().map(|slave| slave.can_control),
        Some(false),
        "the slave of a container without KVM cannot control"
    );

    let path = host.config.state_dir.join(id.as_str()).join("slave.sock");
    let client = SandboxRpcClient::connect(
        ctx.clone(),
        ConnectAddr::Unix(path),
        SandboxLimits::default(),
    )
    .expect("the control client builds");
    let reports = crate::pool::block_on(client.probe()).expect("Probe answers");
    let firecracker = reports
        .iter()
        .find(|report| report.driver == DriverId::Firecracker)
        .expect("the Firecracker report");
    assert!(!firecracker.usable);
    assert!(
        firecracker
            .checks
            .iter()
            .any(|check| check.name == "kvm" && !check.ok),
        "the failed check is kvm: {firecracker:?}"
    );

    crate::pool::block_on(manager.stop(id)).expect("the parent stops");
    drop(guard);
}
