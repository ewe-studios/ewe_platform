//! The support of the control-plane tests.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::env::{Env, Timer, TimerFuture};
use foundation_context::Clock;
use foundation_context::Ctx;
use foundation_core::env;
use foundation_core::valtron::{initialize_pool, SimulatedClock};

use foundation_sandbox::shared::control::testing::{
    in_memory_connector, CountingStore, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{ManagerConfig, RecordStore, SandboxManager};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::SlaveConnector;
use foundation_sandbox::shared::ids::DriverId;

/// One guard that runs the control tests one at a time.
///
/// The tests share one valtron pool, and the control service drives a blocking
/// iterator on a pool worker. One test at a time keeps a worker free.
static SERIAL: Mutex<()> = Mutex::new(());

/// The one-at-a-time guard of the control tests.
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Starts the shared valtron pool one time for the test binary.
pub fn pool() {
    static POOL: OnceLock<()> = OnceLock::new();
    POOL.get_or_init(|| {
        std::mem::forget(initialize_pool(7, Some(8)));
    });
}

/// Drives a future to its result on the test thread.
///
/// The valtron tasks that the future starts run on the shared pool, and their
/// wakes reach this executor.
pub fn block_on<F: Future>(future: F) -> F::Output {
    pool();
    futures::executor::block_on(future)
}

/// The longest wait that the recording timer answers at once.
const SHORT_WAIT: Duration = Duration::from_secs(1);

/// A timer that records each wait. A short wait answers at once; a long wait
/// (a create deadline) never answers, so the deadline does not fire early.
pub struct RecordingTimer {
    clock: Arc<SimulatedClock>,
    waits: Mutex<Vec<Duration>>,
    resolve_long: bool,
}

impl RecordingTimer {
    /// A timer over the clock of the run.
    #[must_use]
    pub fn new(clock: Arc<SimulatedClock>) -> Self {
        Self {
            clock,
            waits: Mutex::new(Vec::new()),
            resolve_long: false,
        }
    }

    /// A timer that also answers a long wait (a create deadline) at once.
    #[must_use]
    pub fn resolving(clock: Arc<SimulatedClock>) -> Self {
        Self {
            clock,
            waits: Mutex::new(Vec::new()),
            resolve_long: true,
        }
    }

    /// The waits that the code asked for, in order.
    #[must_use]
    pub fn waits(&self) -> Vec<Duration> {
        self.waits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Timer for RecordingTimer {
    fn sleep_until(&self, _ctx: Ctx, deadline: Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(Clock::now(self.clock.as_ref()));
        self.waits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(wait);
        if wait <= SHORT_WAIT || self.resolve_long {
            Box::pin(async {})
        } else {
            Box::pin(std::future::pending())
        }
    }
}

/// A context with a simulated clock, a seeded RNG and the real injected timer.
#[must_use]
pub fn ctx(seed: u64) -> (Ctx, Arc<SimulatedClock>) {
    pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env = env::simulated(&ctx, seed).expect("the simulated Env builds");
    ctx.store(env);
    (ctx, clock)
}

/// A context whose timer records its waits.
#[must_use]
pub fn recording_ctx(seed: u64) -> (Ctx, Arc<SimulatedClock>, Arc<RecordingTimer>) {
    recording_ctx_with(seed, false)
}

/// A context whose timer records its waits and answers a long wait at once.
#[must_use]
pub fn recording_ctx_resolve_long(seed: u64) -> (Ctx, Arc<SimulatedClock>, Arc<RecordingTimer>) {
    recording_ctx_with(seed, true)
}

fn recording_ctx_with(
    seed: u64,
    resolve_long: bool,
) -> (Ctx, Arc<SimulatedClock>, Arc<RecordingTimer>) {
    pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let timer = if resolve_long {
        Arc::new(RecordingTimer::resolving(Arc::clone(&clock)))
    } else {
        Arc::new(RecordingTimer::new(Arc::clone(&clock)))
    };
    let env = env::simulated(&ctx, seed).expect("the simulated Env builds");
    let env = env.with_timer(Arc::clone(&timer) as Arc<dyn Timer>);
    let _stored: Env = env.clone();
    ctx.store(env);
    (ctx, clock, timer)
}

/// A manager over one simulated driver and one scripted slave.
pub fn manager(
    ctx: Ctx,
    drivers: Vec<Arc<dyn SandboxDriver>>,
    slave: Arc<ScriptedSlave>,
    config: ManagerConfig,
    store: Arc<dyn RecordStore>,
) -> SandboxManager {
    let connector: Arc<dyn SlaveConnector> = in_memory_connector(slave);
    SandboxManager::new(ctx, drivers, connector, store, config)
}

/// A manager with the default store and configuration.
pub fn simple_manager(
    ctx: Ctx,
    driver: Arc<SimDriver>,
    slave: Arc<ScriptedSlave>,
) -> (SandboxManager, Arc<CountingStore>) {
    let store = Arc::new(CountingStore::new());
    let manager = manager(
        ctx,
        vec![driver],
        slave,
        ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    (manager, store)
}

/// One property configuration with a fixed seed.
#[must_use]
pub fn property_config(seed: u64) -> proptest::prelude::ProptestConfig {
    proptest::prelude::ProptestConfig {
        cases: 32,
        rng_seed: proptest::test_runner::RngSeed::Fixed(seed),
        ..proptest::prelude::ProptestConfig::default()
    }
}

/// The error of a call that must fail.
///
/// # Panics
/// When the call succeeds.
pub fn expect_error<T>(
    result: foundation_sandbox::shared::error::SandboxResult<T>,
    what: &str,
) -> foundation_sandbox::shared::error::SandboxError {
    match result {
        Ok(_) => panic!("{what}: the call succeeded"),
        Err(error) => error.current_context().clone(),
    }
}

/// The driver identifier of the simulated driver.
#[must_use]
pub fn driver_id() -> DriverId {
    DriverId::Firecracker
}
