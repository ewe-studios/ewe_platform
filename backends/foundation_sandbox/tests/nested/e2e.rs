//! The real nested sandbox tests (feature 11, section 7).
//!
//! One parent container runs a slave that also serves the control plane. The
//! tests create micro virtual machines (VMs) in it through the `NestedDriver`,
//! with no socket of their own. A host without Docker, Kernel-based Virtual
//! Machine (KVM), the Firecracker program or the nested host image prints the
//! probe report and returns. The continuous integration (CI) job
//! `sandbox-e2e` sets `EWE_SANDBOX_E2E_REQUIRED=1`, so a skip fails there.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::env;
use foundation_deployment_docker::error::DockerError;
use foundation_deployment_docker::DockerClient;
use foundation_sandbox::native::artifacts::manifest;
use foundation_sandbox::native::connect::ConnectAddr;
use foundation_sandbox::native::docker::driver::{
    container_name, DockerDriver, DockerDriverConfig, DockerLink, SlaveMode,
};
use foundation_sandbox::native::firecracker::driver::FirecrackerDriver;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::{
    spawn_detached, ManagerConfig, MemoryRecordStore, SandboxHandle, SandboxManager,
    SandboxProvider, SandboxRpcClient,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::spec::{GuestArch, NetworkMode, RootFs, SandboxSpec};

/// The nested host image. `mise run sandbox:nested-image` builds it.
fn image() -> String {
    std::env::var("EWE_NESTED_IMAGE").unwrap_or_else(|_| "ewe/sandbox-nested:latest".to_string())
}

/// Whether an image is ready to use. A missing image needs
/// `EWE_DOCKER_E2E_ALLOW_PULL=1`, as in the Docker e2e tests.
fn image_ready(image: &str) -> bool {
    let Ok(client) = DockerClient::connect_with_defaults() else {
        return false;
    };
    match crate::pool::block_on(client.image_inspect(image)) {
        Ok(_) => true,
        Err(_) => std::env::var("EWE_DOCKER_E2E_ALLOW_PULL").as_deref() == Ok("1"),
    }
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
    config.state_dir =
        std::env::temp_dir().join(format!("ewe-nested-e2e-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&config.state_dir);
    let host = SandboxHost::native(config);
    ctx.store(host.clone());
    Some((ctx, host))
}

/// Whether the Docker driver can run here; prints its report.
fn docker_usable(ctx: &Ctx) -> bool {
    let driver = DockerDriver::with_config(
        DockerDriverConfig::default()
            .with_link(DockerLink::Unix)
            .with_slave_mode(SlaveMode::Main),
    );
    let report = crate::pool::block_on(driver.probe(ctx.clone()));
    if !report.usable {
        assert!(!required(), "the CI runner cannot run Docker: {report:?}");
        eprintln!("skip: the host cannot run Docker: {report:?}");
    }
    report.usable
}

/// Whether a micro virtual machine (VM) can run here; prints its report.
fn firecracker_usable(ctx: &Ctx) -> bool {
    let report = crate::pool::block_on(FirecrackerDriver::new().probe(ctx.clone()));
    if !report.usable {
        assert!(
            !required(),
            "the CI runner cannot run Firecracker: {report:?}"
        );
        eprintln!("skip: the host cannot run Firecracker: {report:?}");
    }
    report.usable
}

/// The outer manager: the Docker driver and the nested driver.
///
/// `map_kvm` is false in the no-device form, so the parent starts without
/// `/dev/kvm` even on a host that has it.
fn manager(ctx: &Ctx, host: &SandboxHost, map_kvm: bool) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::with_nested_driver(
        ctx.clone(),
        vec![Arc::new(DockerDriver::with_config(
            DockerDriverConfig::default()
                .with_link(DockerLink::Unix)
                .with_slave_mode(SlaveMode::Main)
                .with_map_kvm_device(map_kvm),
        ))],
        host.connector.clone(),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
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

/// A parent sandbox that is a nested host.
fn parent_spec() -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::ContainerImage(image()));
    spec.driver = Some(DriverId::Docker);
    spec.nested_host = true;
    spec.network = NetworkMode::None;
    spec.limits.boot_timeout = Duration::from_secs(300);
    spec
}

/// The root file system of a child, as a path in the parent container.
///
/// The nested host image holds the Firecracker continuous integration (CI)
/// root image in its artifact cache (section 6.4).
fn child_rootfs() -> PathBuf {
    let rootfs = manifest::firecracker_ci_rootfs(GuestArch::host());
    let kind = rootfs.kind.dir_name();
    let payload = rootfs.kind.payload_name();
    PathBuf::from(format!(
        "/var/cache/ewe-sandbox/{kind}/{}/{payload}",
        &rootfs.sha256[..16]
    ))
}

/// A child of `parent`.
fn child_spec(parent: &SandboxId) -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::SquashfsImage(child_rootfs()));
    spec.driver = Some(DriverId::Nested);
    spec.parent = Some(parent.clone());
    spec.limits.boot_timeout = Duration::from_secs(300);
    spec
}

/// The set-up of one test.
struct Setup {
    ctx: Ctx,
    host: SandboxHost,
    manager: Arc<SandboxManager>,
    guard: KillOnDrop,
}

/// Builds the set-up, or prints why it cannot run.
///
/// `map_kvm` controls whether a parent maps `/dev/kvm`. The no-KVM report
/// starts the parent without the device.
fn setup(tag: &str, need_kvm: bool, map_kvm: bool) -> Option<Setup> {
    let (ctx, host) = usable_host(tag)?;
    if !docker_usable(&ctx) {
        return None;
    }
    if !image_ready(&image()) {
        assert!(!required(), "the CI runner has no image {}", image());
        eprintln!(
            "skip: the image {} is not in the local store and a pull is not allowed",
            image()
        );
        return None;
    }
    if need_kvm && !firecracker_usable(&ctx) {
        return None;
    }
    let manager = manager(&ctx, &host, map_kvm);
    Some(Setup {
        ctx,
        host,
        manager,
        guard: KillOnDrop::new(),
    })
}

/// Creates the parent and gives its identifier.
fn create_parent(setup: &Setup) -> SandboxId {
    let parent = crate::pool::block_on(setup.manager.create(parent_spec()))
        .expect("the parent container starts");
    setup.guard.track(&parent);
    parent.id().clone()
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

/// I1: a child runs a program, and its slave is process identifier 1.
#[test]
fn e2e_nested_acceptance() {
    let Some(setup) = setup("acceptance", true, true) else {
        return;
    };
    let parent = create_parent(&setup);
    let child = crate::pool::block_on(setup.manager.create(child_spec(&parent)))
        .expect("the child starts in the parent");
    setup.guard.track(&child);
    assert_eq!(
        child.info().slave.as_ref().map(|slave| slave.is_pid1),
        Some(true)
    );
    let output = crate::pool::block_on(child.run(Command::new(["echo", "hello"])))
        .expect("the command runs");
    assert_eq!(output.stdout, b"hello\n");
    assert_eq!(output.status.exit_code(), Some(0));
    crate::pool::block_on(child.stop()).expect("the child stops");
}

/// I1: two children are separate file systems.
#[test]
fn e2e_nested_two_children() {
    let Some(setup) = setup("two-children", true, true) else {
        return;
    };
    let parent = create_parent(&setup);
    let first = crate::pool::block_on(setup.manager.create(child_spec(&parent)))
        .expect("the first child starts");
    let second = crate::pool::block_on(setup.manager.create(child_spec(&parent)))
        .expect("the second child starts");
    setup.guard.track(&first);
    setup.guard.track(&second);
    crate::pool::block_on(first.write_file(
        "/tmp/a",
        0o644,
        futures::stream::iter([Ok(b"mark".to_vec())]),
    ))
    .expect("the first child writes");
    let read = crate::pool::block_on(second.run(Command::new(["cat", "/tmp/a"])))
        .expect("the second child answers");
    assert_ne!(read.status.exit_code(), Some(0), "/tmp/a is absent");
}

/// The no-KVM report: a parent without the device fails the `kvm` check.
///
/// The parent starts without `/dev/kvm` even when this host has the device, so
/// the test runs on the Kernel-based Virtual Machine (KVM) runner. A daemon
/// host that has no device proves the same report through the ordinary driver
/// in [`no_kvm`](crate::nested::no_kvm).
#[test]
fn e2e_nested_no_kvm() {
    let Some(setup) = setup("no-kvm", false, false) else {
        return;
    };
    let parent = create_parent(&setup);
    assert_eq!(
        parent_can_control(&setup, &parent),
        Some(false),
        "the slave of a container without KVM cannot control"
    );
    let path = setup
        .host
        .config
        .state_dir
        .join(parent.as_str())
        .join("slave.sock");
    let client = SandboxRpcClient::connect(
        setup.ctx.clone(),
        ConnectAddr::Unix(path),
        foundation_sandbox::shared::limits::SandboxLimits::default(),
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
}

/// `SlaveInfo.can_control` of one parent, when the record carries it.
fn parent_can_control(setup: &Setup, parent: &SandboxId) -> Option<bool> {
    let handle = crate::pool::block_on(setup.manager.get(parent.clone())).ok()?;
    handle.info().slave.map(|slave| slave.can_control)
}

/// I3: stopping the parent stops its children and removes the container.
#[test]
fn e2e_nested_stop_parent() {
    let Some(setup) = setup("stop-parent", true, true) else {
        return;
    };
    let parent = create_parent(&setup);
    let first = crate::pool::block_on(setup.manager.create(child_spec(&parent)))
        .expect("the first child starts");
    let second = crate::pool::block_on(setup.manager.create(child_spec(&parent)))
        .expect("the second child starts");
    setup.guard.track(&first);
    setup.guard.track(&second);
    crate::pool::block_on(setup.manager.stop(parent.clone())).expect("the parent stops");
    assert!(
        crate::pool::block_on(setup.manager.list())
            .expect("list")
            .is_empty(),
        "every record is gone"
    );
    assert!(
        !container_exists(&container_name(&parent)),
        "the parent container is removed"
    );
}
