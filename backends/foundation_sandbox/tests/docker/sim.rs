//! The Docker driver simulation tests (feature 10, section 7).
//!
//! The driver and the container control run against the fake Engine of
//! `support.rs`, a fake host file system and the injected simulated clock. No
//! daemon and no container is used.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::{Clock, Ctx};
use foundation_core::env;
use foundation_core::valtron::{SimulatedClock, Stream};
use foundation_sandbox::native::docker::api::{LABEL_CONTAINER_CONTROL, LABEL_SANDBOX_ID};
use foundation_sandbox::native::docker::control::DockerContainerControl;
use foundation_sandbox::native::docker::driver::{
    container_name, sandbox_body, slave_command, slave_command_for, DockerDriver,
    DockerDriverConfig, DockerHandle, DockerLink, DockerPlan, SlaveMode,
};
use foundation_sandbox::native::host::NativeConnector;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::container::{
    ContainerControl, ContainerResources, ContainerSpec, ContainerStatus,
};
use foundation_sandbox::shared::control::testing::{FakeHostFs, FakeProcessSpawner};
use foundation_sandbox::shared::driver::{
    CreateProgress, SandboxDriver, SlaveEndpoint, StoredRecord, Substrate, SubstrateHandle,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::{HostConfig, HostFs as _, SandboxHost};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits};
use foundation_sandbox::shared::spec::{HostMount, NetworkMode, RootFs, SandboxSpec};

use super::support::{Call, Failure, FakeContainer, FakeDockerApi, Op};

/// A simulated context with a fake host that has the slave file.
fn simulated(tag: &str) -> (Ctx, FakeHostFs, DockerDriverConfig) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed(tag));
    let environment = env::simulated(&ctx, seed(tag)).expect("the simulated environment builds");
    ctx.store(environment);
    let fs = FakeHostFs::new();
    fs.insert_file("/bin/agent-slave", b"slave");
    fs.insert_file("/bin/agent-slave-x86_64", b"slave");
    fs.insert_file("/bin/agent-slave-aarch64", b"slave");
    let host = SandboxHost {
        processes: Arc::new(FakeProcessSpawner::new()),
        fs: Arc::new(fs.clone()),
        connector: Arc::new(NativeConnector),
        config: Arc::new(host_config()),
    };
    ctx.store(host);
    // Unix link: the default on Linux, and the one that `NetworkMode::None`
    // needs.
    (
        ctx,
        fs,
        DockerDriverConfig::default().with_link(DockerLink::Unix),
    )
}

/// A stable seed from a test name.
fn seed(tag: &str) -> u64 {
    tag.bytes().fold(1_u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(byte))
    })
}

fn host_config() -> HostConfig {
    HostConfig {
        state_dir: PathBuf::from("/state"),
        cache_dir: PathBuf::from("/cache"),
        slave_path: Some(PathBuf::from("/bin/agent-slave")),
        firecracker_path: None,
        krun_runner_path: None,
        libkrun_path: None,
        mkfs_ext4_path: None,
        driver_order: vec![DriverId::Docker],
        artifact_limits: ArtifactLimits::default(),
        image_limits: ImageLimits::default(),
    }
}

fn sandbox_id() -> SandboxId {
    "000000000001".parse().expect("a valid sandbox id")
}

fn run_dir() -> PathBuf {
    PathBuf::from("/state/000000000001")
}

fn spec(image: &str) -> SandboxSpec {
    SandboxSpec::new(RootFs::ContainerImage(image.to_string()))
}

/// Runs one launch to its one result and returns every item.
fn run_launch(
    driver: &DockerDriver,
    ctx: &Ctx,
    spec: SandboxSpec,
    deadline: Instant,
) -> Vec<Stream<foundation_sandbox::shared::error::SandboxResult<Substrate>, CreateProgress>> {
    crate::pool::pool();
    let stream = driver.launch(ctx.clone(), sandbox_id(), spec, deadline);
    let mut items = Vec::new();
    for item in stream {
        let done = matches!(item, Stream::Next(_));
        items.push(item);
        if done {
            break;
        }
    }
    items
}

/// The one result of a launch.
fn result_of(
    items: Vec<Stream<foundation_sandbox::shared::error::SandboxResult<Substrate>, CreateProgress>>,
) -> foundation_sandbox::shared::error::SandboxResult<Substrate> {
    items
        .into_iter()
        .find_map(|item| match item {
            Stream::Next(result) => Some(result),
            _ => None,
        })
        .expect("the launch gives one result")
}

/// The progress values of a launch.
fn progress_of(
    items: &[Stream<foundation_sandbox::shared::error::SandboxResult<Substrate>, CreateProgress>],
) -> Vec<CreateProgress> {
    items
        .iter()
        .filter_map(|item| match item {
            Stream::Pending(progress) => Some(*progress),
            _ => None,
        })
        .collect()
}

/// The driver over the fake Engine.
fn driver(api: &FakeDockerApi, config: DockerDriverConfig) -> DockerDriver {
    DockerDriver::with_api_and_config(api.api(), config)
}

/// The plan of one container body, for the body test.
fn plan(
    link: DockerLink,
    mode: SlaveMode,
    network: NetworkMode,
    run_dir: Option<PathBuf>,
) -> DockerPlan {
    let id = sandbox_id();
    let mut spec = self::spec("alpine:3.22");
    spec.network = network;
    spec.vcpus = 2;
    spec.memory_mib = 256;
    spec.mounts = vec![HostMount {
        host_path: PathBuf::from("/host/data"),
        guest_path: "/data".to_string(),
        read_only: true,
    }];
    DockerPlan {
        id: id.clone(),
        image: "alpine:3.22".to_string(),
        container_name: container_name(&id),
        run_dir,
        slave_path: PathBuf::from("/bin/agent-slave"),
        config: DockerDriverConfig::default()
            .with_link(link)
            .with_slave_mode(mode),
        spec,
    }
}

#[test]
fn create_body() {
    // Unix link, main process, no network.
    let body = serde_json::to_value(sandbox_body(&plan(
        DockerLink::Unix,
        SlaveMode::Main,
        NetworkMode::None,
        Some(run_dir()),
    )))
    .expect("the body serializes");
    assert_eq!(body["Image"], "alpine:3.22");
    assert_eq!(
        body["Entrypoint"],
        serde_json::json!([
            "/.ewe/agent-slave",
            "--listen",
            "unix:/.ewe/run/slave.sock",
            "--socket-mode",
            "666"
        ])
    );
    assert_eq!(body["Cmd"], serde_json::json!([]));
    assert!(
        body.get("Env").is_none_or(serde_json::Value::is_null),
        "invariant I2: no environment"
    );
    assert_eq!(body["Labels"][LABEL_SANDBOX_ID], "000000000001");
    assert_eq!(body["Labels"]["ewe.sandbox.driver"], "docker");
    let host = &body["HostConfig"];
    assert_eq!(host["NetworkMode"], "none");
    assert_eq!(
        host["Binds"],
        serde_json::json!(["/state/000000000001:/.ewe/run", "/host/data:/data:ro"])
    );
    assert_eq!(host["Memory"], 256_i64 * 1_048_576);
    assert_eq!(host["NanoCpus"], 2_i64 * 1_000_000_000);
    assert_eq!(host["PidsLimit"], 4096);
    assert_eq!(
        host["SecurityOpt"],
        serde_json::json!(["no-new-privileges"])
    );
    assert_eq!(host["Privileged"], false);
    assert_eq!(host["AutoRemove"], false);
    assert!(
        host["PortBindings"].is_null(),
        "the Unix link publishes no port"
    );
    assert!(body["ExposedPorts"].is_null());

    // TCP link, main process, host network.
    let body = serde_json::to_value(sandbox_body(&plan(
        DockerLink::Tcp,
        SlaveMode::Main,
        NetworkMode::Host,
        None,
    )))
    .expect("the body serializes");
    assert_eq!(body["HostConfig"]["NetworkMode"], "bridge");
    assert_eq!(
        body["Entrypoint"],
        serde_json::json!(["/.ewe/agent-slave", "--listen", "tcp:0.0.0.0:1024"])
    );
    assert_eq!(body["ExposedPorts"], serde_json::json!({ "1024/tcp": {} }));
    assert_eq!(
        body["HostConfig"]["PortBindings"],
        serde_json::json!({
            "1024/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "0" }]
        })
    );
    // The TCP link on a host that is not the Docker host only binds the host
    // mount; there is no run directory bind.
    assert_eq!(
        body["HostConfig"]["Binds"],
        serde_json::json!(["/host/data:/data:ro"])
    );

    // Sidecar: the image keeps its own entry point and command.
    let body = serde_json::to_value(sandbox_body(&plan(
        DockerLink::Unix,
        SlaveMode::Sidecar,
        NetworkMode::None,
        Some(run_dir()),
    )))
    .expect("the body serializes");
    assert!(body["Entrypoint"].is_null());
    assert!(body["Cmd"].is_null());
    assert!(body["Env"].is_null());
}

#[test]
fn slave_command_of_each_link() {
    assert_eq!(
        slave_command(DockerLink::Unix),
        vec![
            "/.ewe/agent-slave",
            "--listen",
            "unix:/.ewe/run/slave.sock",
            "--socket-mode",
            "666"
        ]
    );
    assert_eq!(
        slave_command(DockerLink::Tcp),
        vec!["/.ewe/agent-slave", "--listen", "tcp:0.0.0.0:1024"]
    );
    assert_eq!(
        slave_command_for(DockerLink::Unix, true),
        vec![
            "/.ewe/agent-slave",
            "--listen",
            "unix:/.ewe/run/slave.sock",
            "--socket-mode",
            "666",
            "--control"
        ]
    );
}

/// I6 and section 6.3: a nested host gets `/dev/kvm`, the control argument and
/// the artifact paths. It is still not privileged.
#[test]
fn create_body_nested_host() {
    let mut nested_plan = plan(
        DockerLink::Unix,
        SlaveMode::Main,
        NetworkMode::None,
        Some(run_dir()),
    );
    nested_plan.spec.nested_host = true;
    let body = serde_json::to_value(sandbox_body(&nested_plan)).expect("the body serializes");
    assert_eq!(
        body["Entrypoint"],
        serde_json::json!([
            "/.ewe/agent-slave",
            "--listen",
            "unix:/.ewe/run/slave.sock",
            "--socket-mode",
            "666",
            "--control"
        ])
    );
    assert_eq!(
        body["Env"],
        serde_json::json!([
            "EWE_FIRECRACKER=/usr/local/bin/firecracker",
            "EWE_SANDBOX_SLAVE=/.ewe/agent-slave",
            "EWE_SANDBOX_STATE_DIR=/run/ewe-sbx",
            "EWE_SANDBOX_CACHE_DIR=/var/cache/ewe-sandbox"
        ])
    );
    assert_eq!(
        body["HostConfig"]["Devices"],
        serde_json::json!([{
            "CgroupPermissions": "rwm",
            "PathInContainer": "/dev/kvm",
            "PathOnHost": "/dev/kvm"
        }])
    );
    assert_eq!(body["HostConfig"]["Privileged"], false);

    // A normal container gets no device and no environment variable.
    let plain = plan(
        DockerLink::Unix,
        SlaveMode::Main,
        NetworkMode::None,
        Some(run_dir()),
    );
    let body = serde_json::to_value(sandbox_body(&plain)).expect("the body serializes");
    assert!(body["HostConfig"]["Devices"].is_null());
    assert!(body["Env"].is_null());
}

#[test]
fn pull_when_missing() {
    let (ctx, _fs, config) = simulated("pull_when_missing");
    let api = FakeDockerApi::new();
    let driver = driver(&api, config);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let items = run_launch(&driver, &ctx, spec("alpine:3.22"), deadline);
    let progress = progress_of(&items);
    let substrate = match result_of(items) {
        Ok(substrate) => substrate,
        Err(error) => panic!("the launch failed: {error:?}"),
    };
    assert_eq!(
        substrate.endpoint,
        SlaveEndpoint::Unix {
            path: run_dir().join("slave.sock")
        }
    );
    assert!(
        progress
            .iter()
            .filter(|value| **value == CreateProgress::Preparing)
            .count()
            > 1,
        "the pull emits Preparing more than one time before SubstrateStarted"
    );
    let calls = api.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, Call::ImageInspect(name) if name == "alpine:3.22")));
    assert!(calls
        .iter()
        .any(|call| matches!(call, Call::ImagePull(name) if name == "alpine:3.22")));
    let create = calls
        .iter()
        .find_map(|call| match call {
            Call::Create { body, name } => Some((body, name)),
            _ => None,
        })
        .expect("the driver creates the container");
    assert_eq!(create.1.as_deref(), Some("ewe-sbx-000000000001"));
    assert_eq!(create.0["Labels"][LABEL_SANDBOX_ID], "000000000001");
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, Call::Upload(id, path) if id.len() == 64 && path == "/")),
        "the slave goes into the container"
    );
    assert!(api.has_container("ewe-sbx-000000000001"));
}

#[test]
fn pull_404() {
    let (ctx, fs, config) = simulated("pull_404");
    let api = FakeDockerApi::new();
    api.fail(
        Op::ImagePull,
        Failure::PullFailed("manifest unknown".to_string()),
    );
    let driver = driver(&api, config);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let Err(error) = result_of(run_launch(
        &driver,
        &ctx,
        spec("ewe/does-not-exist:0"),
        deadline,
    )) else {
        panic!("the launch must fail");
    };
    assert!(
        matches!(
            error.current_context(),
            SandboxError::SubstrateApi { status: 404, .. }
        ),
        "a missing image is SubstrateApi with status 404, got {error:?}"
    );
    assert!(
        !api.calls()
            .iter()
            .any(|call| matches!(call, Call::Create { .. })),
        "no container is created"
    );
    assert!(!fs.has_dir(&run_dir()), "the run directory is removed");
}

#[allow(
    clippy::too_many_lines,
    reason = "one case for each row of the fault table"
)]
#[test]
fn launch_fault_matrix() {
    // Prepare: the slave file is missing.
    {
        let (ctx, _fs, config) = simulated("fault_prepare");
        let api = FakeDockerApi::new();
        let fs = FakeHostFs::new();
        // Replace the host file system with one that has no slave.
        let host = SandboxHost {
            processes: Arc::new(FakeProcessSpawner::new()),
            fs: Arc::new(fs.clone()),
            connector: Arc::new(NativeConnector),
            config: Arc::new(host_config()),
        };
        ctx.store(host);
        let driver = driver(&api, config);
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let Err(error) = result_of(run_launch(&driver, &ctx, spec("alpine:3.22"), deadline)) else {
            panic!("the launch must fail");
        };
        assert!(matches!(
            error.current_context(),
            SandboxError::BinaryNotFound { .. }
        ));
        assert!(!api
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Create { .. })));
        assert!(!fs.has_dir(&run_dir()));
    }

    // Create: the daemon refuses the container.
    {
        let (ctx, fs, config) = simulated("fault_create");
        let api = FakeDockerApi::new();
        api.add_image("alpine:3.22");
        api.fail(
            Op::Create,
            Failure::Status(500, "create failed".to_string()),
        );
        let driver = driver(&api, config);
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let Err(error) = result_of(run_launch(&driver, &ctx, spec("alpine:3.22"), deadline)) else {
            panic!("the launch must fail");
        };
        assert!(matches!(
            error.current_context(),
            SandboxError::SubstrateApi { status: 500, .. }
        ));
        assert!(
            !fs.has_dir(&run_dir()),
            "a failed launch removes the run directory"
        );
    }

    // Inject: the upload fails after the container exists.
    {
        let (ctx, fs, config) = simulated("fault_inject");
        let api = FakeDockerApi::new();
        api.add_image("alpine:3.22");
        api.fail(
            Op::Upload,
            Failure::Status(500, "upload failed".to_string()),
        );
        let driver = driver(&api, config);
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let Err(error) = result_of(run_launch(&driver, &ctx, spec("alpine:3.22"), deadline)) else {
            panic!("the launch must fail");
        };
        assert!(matches!(
            error.current_context(),
            SandboxError::SubstrateApi { status: 500, .. }
        ));
        assert!(
            api.calls()
                .iter()
                .any(|call| matches!(call, Call::Remove(id, true) if id.len() == 64)),
            "a failed launch removes the container (force)"
        );
        assert!(!api.has_container("ewe-sbx-000000000001"));
        assert!(!fs.has_dir(&run_dir()));
    }

    // Start: the daemon refuses the start.
    {
        let (ctx, fs, config) = simulated("fault_start");
        let api = FakeDockerApi::new();
        api.add_image("alpine:3.22");
        api.fail(Op::Start, Failure::Status(409, "start failed".to_string()));
        let driver = driver(&api, config);
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let Err(error) = result_of(run_launch(&driver, &ctx, spec("alpine:3.22"), deadline)) else {
            panic!("the launch must fail");
        };
        assert!(matches!(
            error.current_context(),
            SandboxError::SubstrateApi { status: 409, .. }
        ));
        assert!(api
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Remove(id, true) if id.len() == 64)));
        assert!(!fs.has_dir(&run_dir()));
    }

    // Sidecar: the slave exec fails.
    {
        let (ctx, fs, config) = simulated("fault_sidecar");
        let api = FakeDockerApi::new();
        api.add_image("alpine:3.22");
        api.fail(
            Op::ExecCreate,
            Failure::Status(500, "exec failed".to_string()),
        );
        let driver = driver(&api, config.with_slave_mode(SlaveMode::Sidecar));
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let Err(error) = result_of(run_launch(&driver, &ctx, spec("alpine:3.22"), deadline)) else {
            panic!("the launch must fail");
        };
        assert!(matches!(
            error.current_context(),
            SandboxError::SubstrateApi { status: 500, .. }
        ));
        assert!(api
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Remove(id, true) if id.len() == 64)));
        assert!(!fs.has_dir(&run_dir()));
    }
}

#[test]
fn cleanup_idempotent() {
    let (ctx, fs, _config) = simulated("cleanup_idempotent");
    fs.create_dir_all(&run_dir(), 0o700).expect("the run dir");
    let api = FakeDockerApi::new();
    api.add_container(
        "ewe-sbx-000000000001",
        FakeContainer {
            running: true,
            ..FakeContainer::default()
        },
    );
    let handle = DockerHandle::new(
        api.api(),
        ctx.clone(),
        "ewe-sbx-000000000001".to_string(),
        Some(run_dir()),
        "alpine:3.22".to_string(),
    );
    crate::pool::block_on(handle.cleanup()).expect("the first cleanup succeeds");
    crate::pool::block_on(handle.cleanup()).expect("the second cleanup succeeds");
    assert!(!api.has_container("ewe-sbx-000000000001"));
    assert!(!fs.has_dir(&run_dir()));
    assert_eq!(
        api.count(|call| matches!(call, Call::Remove(_, true))),
        2,
        "a second cleanup removes again and accepts the 404"
    );
}

#[test]
fn attach_cases() {
    let (ctx, fs, config) = simulated("attach_cases");
    fs.create_dir_all(&run_dir(), 0o700).expect("the run dir");
    let api = FakeDockerApi::new();
    let record = StoredRecord {
        id: sandbox_id(),
        driver: DriverId::Docker,
        spec: spec("alpine:3.22"),
        state: foundation_sandbox::shared::control::SandboxState::Ready,
        created_unix_ms: 0,
        endpoint: None,
        manifest: None,
    };

    // Running with the label: a substrate.
    api.add_container(
        "ewe-sbx-000000000001",
        FakeContainer {
            running: true,
            image: "alpine:3.22".to_string(),
            labels: [(LABEL_SANDBOX_ID.to_string(), "000000000001".to_string())]
                .into_iter()
                .collect(),
            ..FakeContainer::default()
        },
    );
    let docker_driver = driver(&api, config.clone());
    let attached =
        crate::pool::block_on(docker_driver.attach(ctx.clone(), &record)).expect("attach succeeds");
    let substrate = attached.expect("a running container with the label is found");
    assert_eq!(
        substrate.endpoint,
        SlaveEndpoint::Unix {
            path: run_dir().join("slave.sock")
        }
    );

    // Running without the label: not found, and the container is removed.
    api.add_container(
        "ewe-sbx-000000000001",
        FakeContainer {
            running: true,
            ..FakeContainer::default()
        },
    );
    let attached =
        crate::pool::block_on(docker_driver.attach(ctx.clone(), &record)).expect("attach succeeds");
    assert!(attached.is_none());
    assert!(!api.has_container("ewe-sbx-000000000001"));
    assert!(!fs.has_dir(&run_dir()), "attach removes the run directory");

    // Exited: not found.
    api.add_container(
        "ewe-sbx-000000000001",
        FakeContainer {
            running: false,
            labels: [(LABEL_SANDBOX_ID.to_string(), "000000000001".to_string())]
                .into_iter()
                .collect(),
            ..FakeContainer::default()
        },
    );
    let attached =
        crate::pool::block_on(docker_driver.attach(ctx.clone(), &record)).expect("attach succeeds");
    assert!(attached.is_none());

    // Missing: not found.
    let api = FakeDockerApi::new();
    let docker_driver = driver(&api, config);
    let attached =
        crate::pool::block_on(docker_driver.attach(ctx.clone(), &record)).expect("attach succeeds");
    assert!(attached.is_none());
}

#[test]
fn probe_no_daemon() {
    let (ctx, _fs, config) = simulated("probe_no_daemon");
    let api = FakeDockerApi::new();
    api.fail(
        Op::SystemInfo,
        Failure::Unreachable("cannot connect to the daemon".to_string()),
    );
    let driver = driver(&api, config);
    let report = crate::pool::block_on(driver.probe(ctx.clone()));
    assert!(!report.usable);
    let daemon = report
        .checks
        .iter()
        .find(|check| check.name == "daemon")
        .expect("the daemon check");
    assert!(!daemon.ok);
    assert!(daemon.message.contains("cannot connect to the daemon"));
}

#[test]
fn probe_capabilities_follow_the_link() {
    let (ctx, _fs, config) = simulated("probe_capabilities");
    let api = FakeDockerApi::new();
    let report = crate::pool::block_on(driver(&api, config.clone()).probe(ctx.clone()));
    assert!(report.usable);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.name == "kvm_for_nested"),
        "the probe reports whether the daemon's host has /dev/kvm"
    );
    assert!(
        report.capabilities.network_none,
        "the Unix link has network_none"
    );
    assert!(report.capabilities.rootfs_container_image);
    assert_eq!(report.capabilities.max_vcpus, 8);

    let tcp = config.with_link(DockerLink::Tcp);
    let report = crate::pool::block_on(driver(&api, tcp).probe(ctx));
    assert!(
        !report.capabilities.network_none,
        "the TCP link needs a container network"
    );
}

/// A container control over the fake Engine.
fn control(api: &FakeDockerApi) -> DockerContainerControl {
    DockerContainerControl::new(api.api())
}

fn container_spec() -> ContainerSpec {
    ContainerSpec {
        image: "alpine:3.22".to_string(),
        entrypoint: vec!["sleep".to_string(), "60".to_string()],
        env: vec![("A".to_string(), "B".to_string())],
        network: NetworkMode::Host,
        resources: ContainerResources {
            vcpus: Some(2),
            memory_mib: Some(128),
            instance_type: None,
        },
        labels: vec![("app".to_string(), "test".to_string())],
    }
}

#[test]
fn docker_container_control_sim() {
    let (ctx, _fs, _config) = simulated("container_control");
    let api = FakeDockerApi::new();
    api.set_exec_frames(vec![(1, b"hello\n".to_vec())]);
    let control = control(&api);
    let handle = crate::pool::block_on(control.start(ctx.clone(), container_spec()))
        .expect("the container starts");
    assert_eq!(handle.info().driver, DriverId::Docker);
    assert_eq!(handle.info().status, ContainerStatus::Running);
    let capabilities = handle.capabilities();
    assert!(capabilities.exec);
    assert!(!capabilities.port_fetch);
    assert!(capabilities.signal);
    assert!(capabilities.logs);

    let created = api
        .calls()
        .into_iter()
        .find_map(|call| match call {
            Call::Create { body, .. } => Some(body),
            _ => None,
        })
        .expect("the control creates the container");
    assert_eq!(created["Image"], "alpine:3.22");
    assert_eq!(created["Entrypoint"], serde_json::json!(["sleep", "60"]));
    assert_eq!(created["Env"], serde_json::json!(["A=B"]));
    assert_eq!(created["Labels"][LABEL_CONTAINER_CONTROL], "true");
    assert_eq!(created["Labels"]["app"], "test");
    assert_eq!(created["HostConfig"]["NetworkMode"], "bridge");
    assert_eq!(created["HostConfig"]["Memory"], 128_i64 * 1_048_576);
    assert_eq!(created["HostConfig"]["NanoCpus"], 2_i64 * 1_000_000_000);

    let mut exec = crate::pool::block_on(handle.exec(Command::new(["echo", "hello"])))
        .expect("the exec starts");
    let mut stdout = Vec::new();
    let mut exit = None;
    let mut events = exec.events();
    while let Some(event) = crate::pool::block_on(futures::StreamExt::next(&mut events)) {
        match event.expect("the event") {
            foundation_sandbox::shared::command::ExecEvent::Stdout(bytes) => {
                stdout.extend_from_slice(&bytes);
            }
            foundation_sandbox::shared::command::ExecEvent::Stderr(_) => {}
            foundation_sandbox::shared::command::ExecEvent::Exit(status) => {
                exit = status.exit_code();
            }
        }
    }
    assert_eq!(stdout, b"hello\n");
    assert_eq!(exit, Some(0));

    crate::pool::block_on(handle.signal(9)).expect("the signal");
    assert!(api
        .calls()
        .iter()
        .any(|call| matches!(call, Call::Kill(_, Some(signal)) if signal == "9")));
    let exit = crate::pool::block_on(handle.wait()).expect("the wait");
    assert_eq!(exit.code, Some(0));
    let exit = crate::pool::block_on(handle.stop(Duration::from_secs(5))).expect("the stop");
    assert!(!exit.forced);
    crate::pool::block_on(handle.destroy()).expect("the destroy");

    // `get` finds the running container again; `list` returns it in the fake
    // only while it runs, so start a second one.
    let second = crate::pool::block_on(control.start(ctx.clone(), container_spec()))
        .expect("the second container starts");
    let found = crate::pool::block_on(control.get(ctx.clone(), second.id())).expect("get succeeds");
    assert!(found.is_some());
    let listed = crate::pool::block_on(control.list(ctx.clone())).expect("list succeeds");
    assert_eq!(listed.len(), 1);
}

#[test]
fn docker_container_capabilities() {
    let (ctx, _fs, _config) = simulated("container_capabilities");
    let api = FakeDockerApi::new();
    let control = control(&api);
    let handle = crate::pool::block_on(control.start(ctx.clone(), container_spec()))
        .expect("the container starts");
    let request = foundation_sandbox::shared::container::PortRequest {
        method: "GET".to_string(),
        path: "/".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let Err(error) = crate::pool::block_on(handle.fetch(80, request)) else {
        panic!("the Engine cannot fetch a port");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::UnsupportedCapability { capability } if capability == "port_fetch"
    ));

    let mut spec = container_spec();
    spec.resources.instance_type = Some("lite".to_string());
    let Err(error) = crate::pool::block_on(control.start(ctx, spec)) else {
        panic!("the Docker control rejects an instance type");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::UnsupportedCapability { capability } if capability == "instance_type"
    ));
}

#[test]
fn docker_container_destroy_idempotent() {
    let (ctx, _fs, _config) = simulated("container_destroy");
    let api = FakeDockerApi::new();
    let control = control(&api);
    let handle =
        crate::pool::block_on(control.start(ctx, container_spec())).expect("the container starts");
    crate::pool::block_on(handle.destroy()).expect("the first destroy succeeds");
    crate::pool::block_on(handle.destroy()).expect("the second destroy succeeds");
    assert_eq!(api.count(|call| matches!(call, Call::Remove(_, true))), 2);
}

#[test]
fn docker_container_labels() {
    let (ctx, _fs, _config) = simulated("container_labels");
    let api = FakeDockerApi::new();
    // A container without the control label must never appear in the list.
    api.add_container(
        "not-a-control",
        FakeContainer {
            running: true,
            ..FakeContainer::default()
        },
    );
    let control = control(&api);
    let _first = crate::pool::block_on(control.start(ctx.clone(), container_spec()))
        .expect("the container starts");
    let listed = crate::pool::block_on(control.list(ctx)).expect("list succeeds");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].driver, DriverId::Docker);
    let call = api
        .calls()
        .into_iter()
        .find_map(|call| match call {
            Call::List(all, filters) => Some((all, filters)),
            _ => None,
        })
        .expect("the control lists the containers");
    assert!(!call.0, "the control lists the running containers only");
    assert!(call
        .1
        .unwrap_or_default()
        .contains("ewe.container.control=true"));
}
