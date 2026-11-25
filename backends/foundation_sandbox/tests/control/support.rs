//! The support of the control-plane tests.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::env::{Env, Timer, TimerFuture};
use foundation_context::Clock;
use foundation_context::Ctx;
use foundation_core::env;
use foundation_core::valtron::SimulatedClock;

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
    crate::pool::pool();
}

/// Drives a future to its result on the test thread.
///
/// The valtron tasks that the future starts run on the shared pool, and their
/// wakes reach this executor.
pub fn block_on<F: Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// The longest wait that the recording timer answers at once.
const SHORT_WAIT: Duration = Duration::from_secs(1);

/// A timer that records each wait. A short wait answers at once; a long wait
/// (a create deadline) never answers, so the deadline does not fire early.
pub struct RecordingTimer {
    clock: Arc<SimulatedClock>,
    waits: Mutex<Vec<Duration>>,
    resolve_long: AtomicBool,
}

impl RecordingTimer {
    /// A timer over the clock of the run.
    #[must_use]
    pub fn new(clock: Arc<SimulatedClock>) -> Self {
        Self {
            clock,
            waits: Mutex::new(Vec::new()),
            resolve_long: AtomicBool::new(false),
        }
    }

    /// A timer that also answers a long wait (a create deadline) at once.
    #[must_use]
    pub fn resolving(clock: Arc<SimulatedClock>) -> Self {
        Self {
            clock,
            waits: Mutex::new(Vec::new()),
            resolve_long: AtomicBool::new(true),
        }
    }

    /// Answers every later long wait at once.
    pub fn resolve_long_waits(&self) {
        self.resolve_long.store(true, Ordering::SeqCst);
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
        if wait <= SHORT_WAIT || self.resolve_long.load(Ordering::SeqCst) {
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

/// A record store that starts to fail when the test flips its switch.
#[derive(Debug, Default)]
pub struct SwitchStore {
    inner: Mutex<
        std::collections::HashMap<
            foundation_sandbox::shared::ids::SandboxId,
            foundation_sandbox::shared::driver::StoredRecord,
        >,
    >,
    failing: std::sync::atomic::AtomicBool,
}

impl SwitchStore {
    /// An empty store that accepts writes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every later `put` and `remove` fail.
    pub fn fail(&self) {
        self.failing
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Makes every later `put` and `remove` succeed again.
    pub fn recover(&self) {
        self.failing
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// The records that the store holds.
    #[must_use]
    pub fn records(&self) -> Vec<foundation_sandbox::shared::driver::StoredRecord> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    fn error() -> foundation_errstacks::ErrorTrace<foundation_sandbox::shared::error::SandboxError>
    {
        foundation_sandbox::shared::error::SandboxError::Io {
            op: "store".to_string(),
            message: "the store is full".to_string(),
        }
        .into()
    }
}

impl RecordStore for SwitchStore {
    fn put(
        &self,
        record: foundation_sandbox::shared::driver::StoredRecord,
    ) -> foundation_sandbox::shared::BoxFuture<
        '_,
        foundation_sandbox::shared::error::SandboxResult<()>,
    > {
        if self.failing.load(std::sync::atomic::Ordering::SeqCst) {
            return Box::pin(async { Err(Self::error()) });
        }
        let id = record.id.clone();
        let inner = &self.inner;
        Box::pin(async move {
            inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, record);
            Ok(())
        })
    }

    fn remove(
        &self,
        id: &foundation_sandbox::shared::ids::SandboxId,
    ) -> foundation_sandbox::shared::BoxFuture<
        '_,
        foundation_sandbox::shared::error::SandboxResult<()>,
    > {
        if self.failing.load(std::sync::atomic::Ordering::SeqCst) {
            return Box::pin(async { Err(Self::error()) });
        }
        let id = id.clone();
        let inner = &self.inner;
        Box::pin(async move {
            inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&id);
            Ok(())
        })
    }

    fn list(
        &self,
    ) -> foundation_sandbox::shared::BoxFuture<
        '_,
        foundation_sandbox::shared::error::SandboxResult<
            Vec<foundation_sandbox::shared::driver::StoredRecord>,
        >,
    > {
        let inner = &self.inner;
        Box::pin(async move {
            Ok(inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
                .cloned()
                .collect())
        })
    }
}
