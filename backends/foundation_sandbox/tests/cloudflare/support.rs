//! Support for the Cloudflare tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::env::{Env, Timer, TimerFuture};
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;

use foundation_sandbox::shared::cloudflare::testing::{
    slave_handler, FailingIndex, FakeAlarm, FakeContainer, MemoryDoStorage, MemoryIndex,
};
use foundation_sandbox::shared::cloudflare::{CloudflareConfig, CloudflareHost};
use foundation_sandbox::shared::container::PortError;
use foundation_sandbox::shared::control::testing::ScriptedSlave;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};

/// Starts the shared pool and drives a future to its result.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// A timer whose waits resolve at once above or below a threshold.
///
/// `short` answers a wait of at most one second at once: a retry back-off. `long`
/// answers a later wait at once: a boot deadline or a call deadline. A wait that
/// neither flag answers parks.
pub struct TestTimer {
    clock: Arc<SimulatedClock>,
    short: bool,
    long: bool,
    waits: Mutex<Vec<Duration>>,
}

impl TestTimer {
    /// The waits that the code asked for, in order.
    #[must_use]
    pub fn waits(&self) -> Vec<Duration> {
        self.waits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Timer for TestTimer {
    fn sleep_until(&self, _ctx: Ctx, deadline: Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(Clock::now(self.clock.as_ref()));
        self.waits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(wait);
        let ready = (self.short && wait <= Duration::from_secs(1))
            || (self.long && wait > Duration::from_secs(1));
        if ready {
            Box::pin(async {})
        } else {
            Box::pin(std::future::pending())
        }
    }
}

/// A context with a simulated clock and the test timer.
#[must_use]
pub fn ctx(seed: u64, short: bool, long: bool) -> (Ctx, Arc<SimulatedClock>, Arc<TestTimer>) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let timer = Arc::new(TestTimer {
        clock: Arc::clone(&clock),
        short,
        long,
        waits: Mutex::new(Vec::new()),
    });
    let env: Env = foundation_core::env::simulated(&ctx, seed).expect("the simulated Env builds");
    let env = env.with_timer(timer.clone() as Arc<dyn Timer>);
    ctx.store(env);
    (ctx, clock, timer)
}

/// A context with a simulated clock and the timer of the simulated `Env`.
///
/// That timer wakes when the clock moves, so a test that steps the clock makes
/// a stream deadline pass without a real wait.
#[must_use]
pub fn simulated_ctx(seed: u64) -> (Ctx, Arc<SimulatedClock>) {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env: Env = foundation_core::env::simulated(&ctx, seed).expect("the simulated Env builds");
    ctx.store(env);
    (ctx, clock)
}

/// The configuration of a test host.
#[must_use]
pub fn config() -> CloudflareConfig {
    CloudflareConfig::new()
        .with_name("abc123abc123")
        .with_image_name("sandbox")
        .with_idle_timeout(Duration::from_secs(600))
        .with_boot_timeout(Duration::from_secs(60))
}

/// One ready-to-use host bundle.
pub struct Harness {
    pub ctx: Ctx,
    pub clock: Arc<SimulatedClock>,
    pub timer: Arc<TestTimer>,
    pub container: Arc<FakeContainer>,
    pub storage: Arc<MemoryDoStorage>,
    pub alarm: Arc<FakeAlarm>,
    pub index: Arc<MemoryIndex>,
    pub scripted: Arc<ScriptedSlave>,
    pub host: CloudflareHost,
}

impl Harness {
    /// A harness with no fetch failure.
    #[must_use]
    pub fn new(seed: u64, short: bool, long: bool) -> Self {
        let (ctx, clock, timer) = ctx(seed, short, long);
        Self::build(ctx, clock, timer)
    }

    /// A harness whose timer wakes when the simulated clock moves.
    ///
    /// Use it for a stream deadline test. It starts no real or fake wait.
    #[must_use]
    pub fn new_simulated(seed: u64) -> Self {
        let (ctx, clock) = simulated_ctx(seed);
        let timer = Arc::new(TestTimer {
            clock: Arc::clone(&clock),
            short: false,
            long: false,
            waits: Mutex::new(Vec::new()),
        });
        Self::build(ctx, clock, timer)
    }

    fn build(ctx: Ctx, clock: Arc<SimulatedClock>, timer: Arc<TestTimer>) -> Self {
        let scripted = Arc::new(ScriptedSlave::new());
        let handler = slave_handler(Arc::clone(&scripted));
        let container = Arc::new(FakeContainer::new().with_slave(handler));
        let storage = Arc::new(MemoryDoStorage::new());
        let alarm = Arc::new(FakeAlarm::new());
        let index = Arc::new(MemoryIndex::new());
        let index_trait: Arc<dyn foundation_sandbox::shared::cloudflare::SandboxIndex> =
            index.clone();
        let host = foundation_sandbox::shared::cloudflare::testing::host(
            Arc::clone(&container),
            Arc::clone(&storage),
            Arc::clone(&alarm),
            index_trait,
            config(),
            None,
        );
        Self {
            ctx,
            clock,
            timer,
            container,
            storage,
            alarm,
            index,
            scripted,
            host,
        }
    }

    /// The same harness with a failing index.
    #[must_use]
    pub fn with_failing_index(mut self) -> Self {
        self.host.index = Arc::new(FailingIndex);
        self
    }

    /// The sandbox identifier that the harness uses.
    #[must_use]
    pub fn id(&self) -> foundation_sandbox::shared::ids::SandboxId {
        self.host
            .config
            .name
            .parse()
            .expect("the sandbox identifier is valid")
    }
}

/// A sandbox spec for the test image.
#[must_use]
pub fn spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::ContainerImage("sandbox".to_string()))
}

/// The default failures of a port that is not listening.
#[must_use]
pub fn not_listening(count: usize) -> Vec<PortError> {
    (0..count).map(|_| PortError::NotListening).collect()
}

/// The limits of a test control plane.
///
/// The shutdown grace is one second, so the test timer answers it at once.
#[must_use]
pub fn limits() -> foundation_sandbox::shared::limits::SandboxLimits {
    foundation_sandbox::shared::limits::SandboxLimits::default()
        .with_shutdown_grace(Duration::from_secs(1))
}

/// Builds the control plane of one DO.
#[must_use]
pub fn planner(
    harness: &Harness,
    id: Option<foundation_sandbox::shared::ids::SandboxId>,
) -> foundation_sandbox::shared::cloudflare::DoControlPlane {
    block_on(
        foundation_sandbox::shared::cloudflare::DoControlPlane::open(
            harness.ctx.clone(),
            harness.host.clone(),
            id,
            limits(),
        ),
    )
    .expect("the DO control plane opens")
    .expect("the DO has a stored identifier")
}

/// The error of a result that must fail.
///
/// Some success values (`SandboxHandle`, `ExecHandle`, `Substrate`) do not
/// implement `Debug`, so `Result::expect_err` cannot be used on them.
///
/// # Panics
/// When the result is a success.
pub fn error_of<T>(
    result: foundation_sandbox::shared::error::SandboxResult<T>,
    context: &str,
) -> foundation_sandbox::shared::error::SandboxError {
    match result {
        Ok(_) => panic!("{context}: the call succeeded and should have failed"),
        Err(error) => error.current_context().clone(),
    }
}
