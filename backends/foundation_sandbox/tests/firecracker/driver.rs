//! The Firecracker driver simulation tests (feature 05).
//!
//! The driver runs against a fake API, a fake process spawner and an in-memory
//! host file system. No virtual machine starts, and no test waits on real time.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::env::{Env, Timer, TimerFuture};
use foundation_context::{Clock, Ctx};
use foundation_core::env;
use foundation_core::valtron::{SimulatedClock, Stream};
use foundation_sandbox::native::firecracker::driver::{
    boot_args, machine_config, sweep_stale_run_dirs, ConfigStep, FcHandle, FirecrackerDriver,
};
use foundation_sandbox::native::firecracker::testing::{ApiCall, FakeFirecrackerApi};
use foundation_sandbox::native::host::NativeConnector;
use foundation_sandbox::shared::control::testing::{FakeHostFs, FakeProcessSpawner, ProcessRecord};
use foundation_sandbox::shared::driver::{
    CreateProgress, HostReport, SandboxDriver, SlaveEndpoint, StoredRecord, Substrate,
    SubstrateHandle,
};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::host::{ProcessSpawner, ProcessSpec, SandboxHost, SlaveConnector};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::{ArtifactLimits, SandboxLimits};
use foundation_sandbox::shared::spec::{HostMount, KernelSource, NetworkMode, RootFs, SandboxSpec};

/// One guard that runs the driver tests one at a time.
///
/// A launch blocks the calling thread while the pool drives its task. One test
/// at a time keeps a pool worker free and keeps the recorded waits unambiguous.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A timer that answers a wait at or below `below` at once and parks a longer
/// wait.
struct ThresholdTimer {
    clock: Arc<SimulatedClock>,
    below: Duration,
    waits: Mutex<Vec<Duration>>,
}

impl ThresholdTimer {
    fn new(clock: Arc<SimulatedClock>, below: Duration) -> Arc<Self> {
        Arc::new(Self {
            clock,
            below,
            waits: Mutex::new(Vec::new()),
        })
    }

    fn take_waits(&self) -> Vec<Duration> {
        std::mem::take(&mut *self.waits.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Timer for ThresholdTimer {
    fn sleep_until(&self, _ctx: Ctx, deadline: Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(Clock::now(self.clock.as_ref()));
        self.waits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(wait);
        if wait <= self.below {
            Box::pin(async {})
        } else {
            Box::pin(std::future::pending())
        }
    }
}

/// A context with a simulated clock, a seeded random number generator and a
/// timer with the given threshold.
fn sim_ctx(seed: u64, below: Duration) -> (Ctx, Arc<SimulatedClock>, Arc<ThresholdTimer>) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let timer = ThresholdTimer::new(Arc::clone(&clock), below);
    let env: Env = env::simulated(&ctx, seed)
        .expect("the simulated environment builds")
        .with_timer(Arc::clone(&timer) as Arc<dyn Timer>);
    ctx.store(env);
    (ctx, clock, timer)
}

/// A host file system with the programs and files that a launch needs.
fn seeded_fs() -> FakeHostFs {
    let fs = FakeHostFs::new();
    fs.insert_file("/bin/firecracker", b"firecracker");
    fs.insert_file("/bin/agent-slave", b"slave-bytes");
    fs.insert_file("/kernel", b"kernel");
    fs.insert_file("/rootfs.squashfs", b"rootfs");
    fs
}

fn host_config() -> foundation_sandbox::shared::host::HostConfig {
    foundation_sandbox::shared::host::HostConfig {
        state_dir: PathBuf::from("/state"),
        cache_dir: PathBuf::from("/cache"),
        slave_path: Some(PathBuf::from("/bin/agent-slave")),
        firecracker_path: Some(PathBuf::from("/bin/firecracker")),
        krun_runner_path: None,
        libkrun_path: None,
        mkfs_ext4_path: None,
        driver_order: vec![DriverId::Firecracker],
        artifact_limits: ArtifactLimits::default(),
        image_limits: foundation_sandbox::shared::limits::ImageLimits::default(),
    }
}

/// A connector that fails every call. The driver must not use it.
struct FailingConnector;

impl SlaveConnector for FailingConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        _endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>> {
        Err(SandboxError::Io {
            op: "connect".to_string(),
            message: "the driver must not call the slave".to_string(),
        }
        .into())
    }
}

/// Stores a `SandboxHost` of the fake parts in `ctx`.
fn install_host(
    ctx: &Ctx,
    fs: &FakeHostFs,
    spawner: &FakeProcessSpawner,
    connector: Arc<dyn SlaveConnector>,
) {
    let host = SandboxHost {
        processes: Arc::new(spawner.clone()),
        fs: Arc::new(fs.clone()),
        connector,
        config: Arc::new(host_config()),
    };
    ctx.store(host);
}

/// A spec with a squashfs root, an explicit kernel and no network.
fn sim_spec() -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::SquashfsImage(PathBuf::from("/rootfs.squashfs")));
    spec.kernel = Some(KernelSource::Path(PathBuf::from("/kernel")));
    spec
}

fn sandbox_id() -> SandboxId {
    "000000000001".parse().expect("the identifier parses")
}

fn run_dir() -> PathBuf {
    PathBuf::from("/state/000000000001")
}

/// Runs one launch to its one result and returns every item.
fn run_launch(
    driver: &FirecrackerDriver,
    ctx: &Ctx,
    spec: SandboxSpec,
    deadline: Instant,
) -> Vec<Stream<SandboxResult<Substrate>, CreateProgress>> {
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
    items: Vec<Stream<SandboxResult<Substrate>, CreateProgress>>,
) -> SandboxResult<Substrate> {
    items
        .into_iter()
        .find_map(|item| match item {
            Stream::Next(result) => Some(result),
            _ => None,
        })
        .expect("the launch gives one result")
}

/// A driver with the fake API.
fn driver(api: &Arc<FakeFirecrackerApi>) -> FirecrackerDriver {
    FirecrackerDriver::with_api_factory(api.factory())
}

#[test]
fn fc_boot_args_x86_64() {
    assert_eq!(
        boot_args(foundation_sandbox::shared::spec::GuestArch::X86_64),
        "console=ttyS0 reboot=k panic=1 quiet ewe.listen=vsock:1024 pci=off i8042.noaux \
         i8042.nomux i8042.dumbkbd"
    );
}

#[test]
fn fc_boot_args_aarch64() {
    assert_eq!(
        boot_args(foundation_sandbox::shared::spec::GuestArch::Aarch64),
        "console=ttyS0 reboot=k panic=1 quiet ewe.listen=vsock:1024"
    );
}

#[test]
fn launch_ok() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(7, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let substrate =
        result_of(run_launch(&driver, &ctx, sim_spec(), deadline)).expect("the launch succeeds");

    assert_eq!(
        substrate.endpoint,
        SlaveEndpoint::FirecrackerVsock {
            uds: run_dir().join("v.sock"),
            port: 1024,
        }
    );
    let calls = api.calls();
    assert_eq!(calls.len(), 6);
    assert!(matches!(calls[0], ApiCall::Describe));
    match &calls[1] {
        ApiCall::MachineConfig(body) => {
            assert_eq!(body.vcpu_count, 1);
            assert_eq!(body.mem_size_mib, 512);
            assert_eq!(body.smt, Some(false));
        }
        other => panic!("expected machine-config, got {other:?}"),
    }
    match &calls[2] {
        ApiCall::BootSource(body) => {
            let expected = boot_args(foundation_sandbox::shared::spec::GuestArch::host());
            assert_eq!(body.kernel_image_path, "/kernel");
            assert!(body.initrd_path.is_some());
            assert_eq!(body.boot_args.as_deref(), Some(expected.as_str()));
        }
        other => panic!("expected boot-source, got {other:?}"),
    }
    match &calls[3] {
        ApiCall::Drive(body) => {
            assert_eq!(body.drive_id, "rootfs");
            assert_eq!(body.path_on_host.as_deref(), Some("/rootfs.squashfs"));
            assert!(!body.is_root_device);
            assert_eq!(body.is_read_only, Some(true));
        }
        other => panic!("expected drive, got {other:?}"),
    }
    match &calls[4] {
        ApiCall::Vsock(body) => {
            assert!((3..=2_147_483_647).contains(&body.guest_cid));
            assert_eq!(
                body.uds_path,
                run_dir().join("v.sock").display().to_string()
            );
        }
        other => panic!("expected vsock, got {other:?}"),
    }
    assert!(matches!(calls[5], ApiCall::Start));
    // The run directory has the pid file and the empty logs.
    assert!(fs.has_file(&run_dir().join("pid")));
    assert!(fs.has_dir(&run_dir()));
}

#[test]
fn process_spec() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(8, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let _ = result_of(run_launch(&driver, &ctx, sim_spec(), deadline)).expect("the launch");

    let specs = spawner.specs();
    let record: &ProcessRecord = specs.first().expect("one spawn");
    assert_eq!(record.program, PathBuf::from("/bin/firecracker"));
    assert!(record.args.iter().any(|arg| arg == "--api-sock"));
    assert!(
        !record.args.iter().any(|arg| arg == "--no-seccomp"),
        "the default seccomp filter stays on"
    );
    assert!(
        record.env.is_empty(),
        "the process gets an empty environment"
    );
    assert!(record.kill_with_parent);
}

#[test]
fn api_error_stops() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(9, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::with_put_error(
        ConfigStep::Drive,
        SandboxError::SubstrateApi {
            status: 400,
            message: "bad drive".to_string(),
        },
    );
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
    match result {
        Err(error) => assert_eq!(
            error.current_context(),
            &SandboxError::SubstrateApi {
                status: 400,
                message: "bad drive".to_string(),
            }
        ),
        Ok(_) => panic!("the launch must fail"),
    }
    assert!(
        !api.calls()
            .iter()
            .any(|call| matches!(call, ApiCall::Start)),
        "instance_start is never called after a rejected setting"
    );
    let child = spawner.last_child().expect("one child");
    assert!(!child.is_live(), "the process got SIGKILL");
    assert!(child.signals().contains(&libc::SIGKILL));
    assert!(!fs.has_dir(&run_dir()), "no run directory is left");
}

#[test]
fn vmm_exit_early() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(10, Duration::from_millis(1));
    let fs = seeded_fs();
    fs.insert_file(
        run_dir().join("console.log"),
        b"agent-slave: no listen address\n",
    );
    let spawner = FakeProcessSpawner::new();
    spawner.exit_next_with(152);
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::with_describe_pending();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
    match result {
        Err(error) => match error.current_context() {
            SandboxError::SubstrateExited { code, log_tail } => {
                assert_eq!(*code, Some(152));
                assert!(String::from_utf8_lossy(log_tail).contains("agent-slave"));
            }
            other => panic!("expected SubstrateExited, got {other:?}"),
        },
        Ok(_) => panic!("the launch must fail"),
    }
    assert!(!fs.has_dir(&run_dir()), "no run directory is left");
}

#[test]
fn api_never_answers() {
    let _guard = serial();
    // The boot deadline is 10 s, so the timer answers it at once.
    let (ctx, _clock, _timer) = sim_ctx(11, Duration::from_secs(11));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::with_describe_pending();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
    match result {
        Err(error) => match error.current_context() {
            SandboxError::BootTimeout { stage, .. } => assert_eq!(stage, "api"),
            other => panic!("expected BootTimeout, got {other:?}"),
        },
        Ok(_) => panic!("the launch must fail"),
    }
    assert!(!fs.has_dir(&run_dir()), "no run directory is left");
}

#[test]
fn await_api_backoff() {
    let _guard = serial();
    let (ctx, _clock, timer) = sim_ctx(12, Duration::from_millis(150));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::with_describe_failures(4);
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let _ = result_of(run_launch(&driver, &ctx, sim_spec(), deadline)).expect("the launch");
    let waits = timer.take_waits();
    let short: Vec<u64> = waits
        .iter()
        .filter(|wait| **wait <= Duration::from_millis(150))
        .map(|wait| u64::try_from(wait.as_millis()).unwrap_or(u64::MAX))
        .collect();
    assert_eq!(short, vec![5, 10, 20, 40]);
}

#[test]
fn launch_uses_given_deadline() {
    let _guard = serial();
    // The deadline is 50 ms and the API answers after 100 ms. The timer answers
    // only the 50 ms wait.
    let (ctx, _clock, _timer) = sim_ctx(13, Duration::from_millis(60));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::with_describe_delay(Duration::from_millis(100));
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_millis(50);
    let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
    match result {
        Err(error) => match error.current_context() {
            SandboxError::BootTimeout { stage, .. } => assert_eq!(stage, "api"),
            other => panic!("expected BootTimeout, got {other:?}"),
        },
        Ok(_) => panic!("the launch must fail at the given deadline"),
    }
}

#[test]
fn rejects_directory_rootfs() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(14, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let mut spec = sim_spec();
    spec.rootfs = RootFs::Directory(PathBuf::from("/root"));
    let result = result_of(run_launch(&driver, &ctx, spec, deadline));
    assert!(matches!(
        result,
        Err(error) if matches!(error.current_context(), SandboxError::UnsupportedRootFs)
    ));
    assert!(spawner.specs().is_empty(), "no process was spawned");
}

#[test]
fn rejects_odd_vcpus() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(15, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let mut spec = sim_spec();
    spec.vcpus = 3;
    let result = result_of(run_launch(&driver, &ctx, spec, deadline));
    assert!(matches!(
        result,
        Err(error) if matches!(error.current_context(), SandboxError::InvalidSpec { field, .. } if field == "vcpus")
    ));
    assert!(spawner.specs().is_empty(), "no process was spawned");
}

#[test]
fn rejects_host_network() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(16, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let mut spec = sim_spec();
    spec.network = NetworkMode::Host;
    let result = result_of(run_launch(&driver, &ctx, spec, deadline));
    assert!(matches!(
        result,
        Err(error) if matches!(error.current_context(), SandboxError::UnsupportedNetwork)
    ));
    assert!(spawner.specs().is_empty(), "no process was spawned");
}

#[test]
fn rejects_host_mounts() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(17, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let mut spec = sim_spec();
    spec.mounts = vec![HostMount {
        host_path: PathBuf::from("/tmp/share"),
        guest_path: "/mnt/share".to_string(),
        read_only: true,
    }];
    let result = result_of(run_launch(&driver, &ctx, spec, deadline));
    assert!(matches!(
        result,
        Err(error) if matches!(error.current_context(), SandboxError::UnsupportedMount)
    ));
    assert!(spawner.specs().is_empty(), "no process was spawned");
}

#[test]
fn driver_does_not_call_slave() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(18, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(FailingConnector));
    let api = FakeFirecrackerApi::new();
    let driver = driver(&api);
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let substrate = result_of(run_launch(&driver, &ctx, sim_spec(), deadline))
        .expect("the driver does not need the slave");
    assert!(matches!(
        substrate.endpoint,
        SlaveEndpoint::FirecrackerVsock { .. }
    ));
}

#[test]
fn probe_missing_binary() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(19, Duration::from_millis(1));
    let fs = FakeHostFs::new();
    fs.insert_file("/bin/agent-slave", b"slave");
    let spawner = FakeProcessSpawner::new();
    let mut config = host_config();
    config.firecracker_path = Some(PathBuf::from("/bin/missing-firecracker"));
    let host = SandboxHost {
        processes: Arc::new(spawner.clone()),
        fs: Arc::new(fs.clone()),
        connector: Arc::new(NativeConnector),
        config: Arc::new(config),
    };
    ctx.store(host);
    let driver = FirecrackerDriver::new();
    let report: HostReport = crate::pool::block_on(driver.probe(ctx.clone()));
    assert!(!report.usable);
    let check = report
        .checks
        .iter()
        .find(|check| check.name == "firecracker")
        .expect("the firecracker check");
    assert!(!check.ok);
}

#[test]
fn probe_no_kvm() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(20, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    spawner.with_version_output(fs.clone(), "Firecracker v1.15.0\n");
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let driver = FirecrackerDriver::new();
    let report: HostReport = crate::pool::block_on(driver.probe(ctx.clone()));
    assert!(!report.usable);
    let check = report
        .checks
        .iter()
        .find(|check| check.name == "kvm")
        .expect("the kvm check");
    assert!(!check.ok);
}

#[test]
fn attach_returns_none_and_removes_run_dir() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(21, Duration::from_millis(1));
    let fs = seeded_fs();
    fs.insert_dir(run_dir());
    fs.insert_file(run_dir().join("pid"), b"123");
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    let driver = FirecrackerDriver::new();
    let record = StoredRecord {
        id: sandbox_id(),
        driver: DriverId::Firecracker,
        spec: sim_spec(),
        state: foundation_sandbox::shared::control::SandboxState::Ready,
        created_unix_ms: 0,
        endpoint: None,
        manifest: None,
    };
    let attached =
        crate::pool::block_on(driver.attach(ctx.clone(), &record)).expect("attach succeeds");
    assert!(attached.is_none());
    assert!(!fs.has_dir(&run_dir()), "the run directory is removed");
}

#[test]
fn sweep_removes_dead() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(22, Duration::from_millis(1));
    let fs = seeded_fs();
    fs.insert_dir("/state/dead");
    fs.insert_file("/state/dead/pid", b"999999");
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    sweep_stale_run_dirs(&ctx, &SandboxLimits::default()).expect("the sweep");
    assert!(!fs.has_dir(std::path::Path::new("/state/dead")));
}

#[test]
fn sweep_keeps_live() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(23, Duration::from_millis(1));
    let fs = seeded_fs();
    fs.insert_dir("/state/live");
    let spawner = FakeProcessSpawner::new();
    let child = spawner
        .spawn(ProcessSpec {
            program: PathBuf::from("/bin/sleep"),
            args: vec!["60".to_string()],
            env: Vec::new(),
            cwd: PathBuf::from("/state"),
            stdout: PathBuf::from("/state/live/out"),
            stderr: PathBuf::from("/state/live/err"),
            kill_with_parent: true,
            extra_fds: Vec::new(),
        })
        .expect("the child starts");
    fs.insert_file("/state/live/pid", child.pid().to_string().as_bytes());
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    sweep_stale_run_dirs(&ctx, &SandboxLimits::default()).expect("the sweep");
    assert!(
        fs.has_dir(std::path::Path::new("/state/live")),
        "a live process keeps its directory"
    );
}

#[test]
fn sweep_removes_old_without_pid() {
    let _guard = serial();
    let (ctx, _clock, _timer) = sim_ctx(24, Duration::from_millis(1));
    let fs = seeded_fs();
    fs.insert_dir("/state/old");
    fs.set_modified(
        "/state/old",
        foundation_context::Clock::wall_now(ctx.clock().as_ref()) - Duration::from_secs(60),
    );
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(NativeConnector));
    sweep_stale_run_dirs(&ctx, &SandboxLimits::default()).expect("the sweep");
    assert!(!fs.has_dir(std::path::Path::new("/state/old")));
}

#[test]
fn handle_model() {
    let _guard = serial();
    // A short grace answers at once, so terminate sends SIGTERM.
    let (ctx, _clock, _timer) = sim_ctx(25, Duration::from_millis(100));
    let fs = seeded_fs();
    fs.insert_dir(run_dir());
    let spawner = FakeProcessSpawner::new();
    let child = spawner
        .spawn(ProcessSpec {
            program: PathBuf::from("/bin/firecracker"),
            args: Vec::new(),
            env: Vec::new(),
            cwd: run_dir(),
            stdout: run_dir().join("console.log"),
            stderr: run_dir().join("console.log"),
            kill_with_parent: true,
            extra_fds: Vec::new(),
        })
        .expect("the child starts");
    let control = spawner.last_child().expect("the child control");
    let handle = FcHandle::new(
        ctx.clone(),
        Arc::from(child),
        Arc::new(fs.clone()),
        run_dir(),
    );

    // A terminate with a grace the timer answers at once forces the stop.
    let exit =
        crate::pool::block_on(handle.terminate(Duration::from_millis(10))).expect("terminate");
    assert!(exit.forced);
    assert_eq!(exit.signal, Some(libc::SIGTERM));
    let signals = control.signals();
    assert_eq!(signals, vec![libc::SIGTERM]);

    // A kill after the exit sends no signal and returns the stored exit.
    let exit = crate::pool::block_on(handle.kill()).expect("kill after exit");
    assert_eq!(control.signals(), signals, "no signal after the exit");
    assert!(exit.forced);

    // Cleanup twice is safe, and the directory is gone.
    crate::pool::block_on(handle.cleanup()).expect("cleanup");
    crate::pool::block_on(handle.cleanup()).expect("cleanup again");
    assert!(!fs.has_dir(&run_dir()));
}

#[test]
fn machine_config_body() {
    let plan = foundation_sandbox::native::firecracker::driver::FcPlan {
        id: sandbox_id(),
        run_dir: run_dir(),
        firecracker: PathBuf::from("/bin/firecracker"),
        kernel: PathBuf::from("/kernel"),
        initramfs: PathBuf::from("/initramfs"),
        rootfs: PathBuf::from("/rootfs.squashfs"),
        rootfs_type: foundation_sandbox::shared::spec::FsType::Squashfs,
        root_mode: foundation_sandbox::shared::spec::RootMode::Overlay,
        vcpus: 2,
        memory_mib: 256,
        guest_cid: 3,
        boot_args: String::new(),
        limits: SandboxLimits::default(),
    };
    let body = machine_config(&plan);
    assert_eq!(body.vcpu_count, 2);
    assert_eq!(body.mem_size_mib, 256);
    assert_eq!(body.smt, Some(false));
}

#[test]
fn no_os_calls_outside_host() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/native/firecracker");
    let forbidden = [
        "std::fs::",
        "std::process::Command",
        "Command::new",
        "std::net::Tcp",
        "TcpStream",
        "TcpListener",
        "UnixStream",
        "UnixListener",
        "std::env::",
    ];
    for path in source_files(&root) {
        let text = std::fs::read_to_string(&path).expect("read a source file");
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{} must not call {needle}",
                path.display()
            );
        }
    }
}

fn source_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // The generated API client is not control-side code.
            if path.file_name().is_some_and(|name| name == "generated") {
                continue;
            }
            files.extend(source_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}
