//! The support of the libkrun driver tests (feature 09).
//!
//! The support gives a simulated clock, a seeded random number generator, a
//! timer with a fixed threshold, and an in-memory host with the files that a
//! launch needs. No test waits on real time.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::env::{Env, Timer, TimerFuture};
use foundation_context::{Clock, Ctx};
use foundation_core::env;
use foundation_core::valtron::{SimulatedClock, Stream};
use foundation_sandbox::shared::control::testing::{FakeHostFs, FakeProcessSpawner};
use foundation_sandbox::shared::driver::{CreateProgress, SandboxDriver, Substrate};
use foundation_sandbox::shared::error::SandboxResult;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost, SlaveConnector};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::ArtifactLimits;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};

/// One guard that runs the driver tests one at a time.
///
/// The tests share one valtron pool, and a launch drives a blocking iterator.
/// One test at a time keeps a pool worker free.
static SERIAL: Mutex<()> = Mutex::new(());

/// The one-at-a-time guard.
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A timer that answers a wait at or below `below` at once and parks a longer
/// wait.
pub struct ThresholdTimer {
    clock: Arc<SimulatedClock>,
    below: Duration,
}

impl ThresholdTimer {
    fn new(clock: Arc<SimulatedClock>, below: Duration) -> Arc<Self> {
        Arc::new(Self { clock, below })
    }
}

impl Timer for ThresholdTimer {
    fn sleep_until(&self, _ctx: Ctx, deadline: Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(Clock::now(self.clock.as_ref()));
        if wait <= self.below {
            Box::pin(async {})
        } else {
            Box::pin(std::future::pending())
        }
    }
}

/// A context with a simulated clock, a seeded random number generator and a
/// timer with the given threshold.
pub fn sim_ctx(seed: u64, below: Duration) -> (Ctx, Arc<SimulatedClock>) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let timer = ThresholdTimer::new(Arc::clone(&clock), below);
    let env: Env = env::simulated(&ctx, seed)
        .expect("the simulated environment builds")
        .with_timer(timer as Arc<dyn Timer>);
    ctx.store(env);
    (ctx, clock)
}

/// A host file system with the programs and files that a launch needs.
pub fn seeded_fs() -> FakeHostFs {
    let fs = FakeHostFs::new();
    fs.insert_file("/bin/sandbox-krun", b"runner");
    fs.insert_file("/bin/agent-slave", b"slave-bytes");
    fs.insert_dir("/root");
    fs
}

/// The host configuration of the driver tests.
pub fn host_config() -> HostConfig {
    HostConfig {
        state_dir: PathBuf::from("/state"),
        cache_dir: PathBuf::from("/cache"),
        slave_path: Some(PathBuf::from("/bin/agent-slave")),
        firecracker_path: None,
        krun_runner_path: Some(PathBuf::from("/bin/sandbox-krun")),
        libkrun_path: None,
        driver_order: vec![DriverId::Libkrun],
        artifact_limits: ArtifactLimits::default(),
    }
}

/// Stores a `SandboxHost` of the fake parts in `ctx`.
pub fn install_host(
    ctx: &Ctx,
    fs: &FakeHostFs,
    spawner: &FakeProcessSpawner,
    connector: Arc<dyn SlaveConnector>,
) {
    install_host_with(ctx, fs, spawner, connector, host_config());
}

/// Stores a `SandboxHost` with an exact configuration in `ctx`.
pub fn install_host_with(
    ctx: &Ctx,
    fs: &FakeHostFs,
    spawner: &FakeProcessSpawner,
    connector: Arc<dyn SlaveConnector>,
    config: HostConfig,
) {
    let host = SandboxHost {
        processes: Arc::new(spawner.clone()),
        fs: Arc::new(fs.clone()),
        connector,
        config: Arc::new(config),
    };
    ctx.store(host);
}

/// A spec with a directory root.
pub fn sim_spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::Directory(PathBuf::from("/root")))
}

/// The fixed sandbox identifier of the driver tests.
pub fn sandbox_id() -> SandboxId {
    "000000000001".parse().expect("the identifier parses")
}

/// The run directory of [`sandbox_id`].
pub fn run_dir() -> PathBuf {
    PathBuf::from("/state/000000000001")
}

/// The boot root of [`sandbox_id`].
pub fn boot_root() -> PathBuf {
    run_dir().join("root")
}

/// One item of a launch stream.
pub type LaunchItem = Stream<SandboxResult<Substrate>, CreateProgress>;

/// Runs one launch to its one result and returns every item.
pub fn run_launch<D: SandboxDriver>(
    driver: &D,
    ctx: &Ctx,
    spec: SandboxSpec,
    deadline: Instant,
) -> Vec<LaunchItem> {
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
pub fn result_of(items: Vec<LaunchItem>) -> SandboxResult<Substrate> {
    items
        .into_iter()
        .find_map(|item| match item {
            Stream::Next(result) => Some(result),
            _ => None,
        })
        .expect("the launch gives one result")
}

/// The progress values of a launch, in order.
pub fn progress_of(items: &[LaunchItem]) -> Vec<CreateProgress> {
    items
        .iter()
        .filter_map(|item| match item {
            Stream::Pending(progress) => Some(*progress),
            _ => None,
        })
        .collect()
}
