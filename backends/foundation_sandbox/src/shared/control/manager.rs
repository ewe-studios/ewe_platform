//! `SandboxManager`: the registry, the driver choice and the public control
//! plane.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use foundation_context::Ctx;

use crate::shared::drivers::nested::NestedDriver;

use crate::shared::container::ContainerControl;
use crate::shared::control::record::{created_unix_ms, RecordStore, SandboxRecord};
use crate::shared::control::send_box;
use crate::shared::control::stop::{kill_record, spawn_detached, start_watcher, stop_record};
use crate::shared::control::{ExitReport, SandboxInfo, SandboxProvider, SandboxState};
use crate::shared::driver::{HostReport, SandboxDriver, StoredRecord, Substrate, SubstrateHandle};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::CreateStream;
use crate::shared::host::SlaveConnector;
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::images::ImageResolver;
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::SandboxSpec;
use crate::shared::BoxFuture;

/// What the manager does with its sandboxes when it is dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropPolicy {
    /// A task kills every record. This is the default: a host program that
    /// ends must not leave a substrate behind.
    #[default]
    KillAll,
    /// Nothing. The substrates keep running, and a later manager finds them
    /// with [`SandboxManager::restore`].
    Detach,
}

/// How the manager chooses the identifier of a new sandbox.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SandboxIdPolicy {
    /// A random identifier from the context.
    #[default]
    Random,
    /// One fixed identifier. A second create while that sandbox exists fails
    /// with `TooManySessions`.
    Fixed(SandboxId),
}

/// The configuration of one manager.
#[derive(Clone, Debug)]
pub struct ManagerConfig {
    /// The limits of a spec that keeps the default limits.
    pub limits: SandboxLimits,
    /// The most sandboxes in `Creating` or `Ready`. Default 64.
    pub max_sandboxes: usize,
    /// How a new sandbox gets its identifier.
    pub id_policy: SandboxIdPolicy,
    /// What the manager does when it is dropped.
    pub on_drop: DropPolicy,
    /// The most stopped reports that the manager keeps for a later idempotent
    /// stop. Default 1024.
    pub max_stopped_reports: usize,
    /// The grace that `terminate` gives a substrate after a stop ran out of
    /// `shutdown_grace`. Default 1 s.
    pub terminate_grace: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            limits: SandboxLimits::default(),
            max_sandboxes: 64,
            id_policy: SandboxIdPolicy::Random,
            on_drop: DropPolicy::KillAll,
            max_stopped_reports: 1024,
            terminate_grace: Duration::from_secs(1),
        }
    }
}

impl ManagerConfig {
    /// A configuration with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::limits`].
    #[must_use]
    pub fn with_limits(mut self, value: SandboxLimits) -> Self {
        self.limits = value;
        self
    }

    /// Replaces [`Self::max_sandboxes`].
    #[must_use]
    pub fn with_max_sandboxes(mut self, value: usize) -> Self {
        self.max_sandboxes = value;
        self
    }

    /// Replaces [`Self::id_policy`].
    #[must_use]
    pub fn with_id_policy(mut self, value: SandboxIdPolicy) -> Self {
        self.id_policy = value;
        self
    }

    /// Replaces [`Self::on_drop`].
    #[must_use]
    pub fn with_on_drop(mut self, value: DropPolicy) -> Self {
        self.on_drop = value;
        self
    }

    /// Replaces [`Self::max_stopped_reports`].
    #[must_use]
    pub fn with_max_stopped_reports(mut self, value: usize) -> Self {
        self.max_stopped_reports = value;
        self
    }

    /// Replaces [`Self::terminate_grace`].
    #[must_use]
    pub fn with_terminate_grace(mut self, value: Duration) -> Self {
        self.terminate_grace = value;
        self
    }
}

/// The shared state of one manager.
pub struct Inner {
    /// The context: clock, entropy and the injected side effects.
    pub ctx: Ctx,
    /// The drivers, in preference order.
    pub drivers: Vec<Arc<dyn SandboxDriver>>,
    /// Turns a slave endpoint into a transport.
    pub connector: Arc<dyn SlaveConnector>,
    /// Where the records live outside memory.
    pub store: Arc<dyn RecordStore>,
    /// The configuration.
    pub config: ManagerConfig,
    /// The resolver of a named image. `None` refuses `RootFs::Image`.
    image_resolver: Mutex<Option<Arc<dyn ImageResolver>>>,
    entries: Mutex<HashMap<SandboxId, Arc<SandboxRecord>>>,
    stopped: Mutex<Vec<(SandboxId, ExitReport)>>,
    restored: AtomicBool,
}

impl Inner {
    /// Adds one record to the registry.
    pub fn insert(&self, record: Arc<SandboxRecord>) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(record.id.clone(), record);
    }

    /// The record of one sandbox.
    pub fn record(&self, id: &SandboxId) -> Option<Arc<SandboxRecord>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    /// Removes the record of one sandbox.
    pub fn remove(&self, id: &SandboxId) -> Option<Arc<SandboxRecord>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id)
    }

    /// Every live record.
    pub fn records(&self) -> Vec<Arc<SandboxRecord>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// The number of records in `Creating` or `Ready`.
    pub fn live_count(&self) -> usize {
        self.records()
            .iter()
            .filter(|record| {
                matches!(
                    record
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .state,
                    SandboxState::Creating | SandboxState::Ready
                )
            })
            .count()
    }

    /// Keeps the report of a stopped sandbox for a later idempotent stop.
    pub fn remember_exit(&self, id: SandboxId, report: ExitReport) {
        let mut stopped = self
            .stopped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stopped.retain(|(stopped_id, _)| stopped_id != &id);
        stopped.push((id, report));
        let max = self.config.max_stopped_reports;
        while stopped.len() > max {
            stopped.remove(0);
        }
    }

    /// The kept report of a stopped sandbox.
    pub fn exit_report(&self, id: &SandboxId) -> Option<ExitReport> {
        self.stopped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(stopped_id, _)| stopped_id == id)
            .map(|(_, report)| *report)
    }

    /// Writes the record to the store. A failure is logged and does not
    /// change the result of the caller.
    pub async fn persist(&self, record: &SandboxRecord) {
        if let Err(error) = self.store.put(record.stored()).await {
            tracing::warn!(error = %error, id = %record.id, "the record store did not keep the record");
        }
    }

    /// Sets the resolver of a named image.
    pub fn set_image_resolver(&self, resolver: Arc<dyn ImageResolver>) {
        *self
            .image_resolver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(resolver);
    }

    /// The resolver of a named image, when the manager has one.
    pub fn image_resolver(&self) -> Option<Arc<dyn ImageResolver>> {
        self.image_resolver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The driver with this identifier.
    pub fn driver(&self, id: DriverId) -> Option<Arc<dyn SandboxDriver>> {
        self.drivers
            .iter()
            .find(|driver| driver.id() == id)
            .cloned()
    }
}

/// The control plane, local to this process.
pub struct SandboxManager {
    inner: Arc<Inner>,
}

impl SandboxManager {
    /// A manager over the drivers and the store.
    ///
    /// `drivers` is in preference order. `connector` turns an endpoint into a
    /// transport.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        drivers: Vec<Arc<dyn SandboxDriver>>,
        connector: Arc<dyn SlaveConnector>,
        store: Arc<dyn RecordStore>,
        config: ManagerConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                ctx,
                drivers,
                connector,
                store,
                config,
                image_resolver: Mutex::new(None),
                entries: Mutex::new(HashMap::new()),
                stopped: Mutex::new(Vec::new()),
                restored: AtomicBool::new(false),
            }),
        }
    }

    /// A manager with the nested driver over its own registry (feature 11).
    ///
    /// The nested driver finds the parent of a child in this manager. The
    /// manager fills the driver's registry slot after it exists, because the
    /// manager holds the driver.
    #[must_use]
    pub fn with_nested_driver(
        ctx: Ctx,
        mut drivers: Vec<Arc<dyn SandboxDriver>>,
        connector: Arc<dyn SlaveConnector>,
        store: Arc<dyn RecordStore>,
        config: ManagerConfig,
    ) -> Self {
        let slot = Arc::new(OnceLock::new());
        drivers.push(Arc::new(NestedDriver::with_slot(Arc::clone(&slot))));
        let inner = Arc::new(Inner {
            ctx,
            drivers,
            connector,
            store,
            config,
            image_resolver: Mutex::new(None),
            entries: Mutex::new(HashMap::new()),
            stopped: Mutex::new(Vec::new()),
            restored: AtomicBool::new(false),
        });
        let _ = slot.set(Arc::clone(&inner));
        Self { inner }
    }

    /// The shared state, for the create task and the handles.
    #[must_use]
    pub fn inner(&self) -> Arc<Inner> {
        Arc::clone(&self.inner)
    }

    /// Sets the resolver that turns a named image into a root file and a
    /// manifest (feature 12).
    ///
    /// A manager with no resolver refuses `RootFs::Image` with
    /// `InvalidSpec { field: "rootfs" }`.
    #[must_use]
    pub fn with_image_resolver(self, resolver: Arc<dyn ImageResolver>) -> Self {
        self.inner.set_image_resolver(resolver);
        self
    }

    /// Loads the stored records and finds their substrates again.
    ///
    /// Call it one time, before the first request, in a control plane that can
    /// restart while its sandboxes live. A second call does nothing.
    ///
    /// # Errors
    /// The error of the record store.
    pub async fn restore(&self) -> SandboxResult<()> {
        if self.inner.restored.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let stored = self.inner.store.list().await?;
        for record in stored {
            self.restore_one(record).await;
        }
        Ok(())
    }

    async fn restore_one(&self, stored: StoredRecord) {
        if stored.state == SandboxState::Stopped {
            self.remove_stored(&stored.id).await;
            return;
        }
        let Some(driver) = self.inner.driver(stored.driver) else {
            self.remove_stored(&stored.id).await;
            return;
        };
        // The lifetime deadline is fixed at create. A restore that finds it in
        // the past destroys the sandbox, as a `Creating` record is today.
        let now = created_unix_ms(&self.inner.ctx);
        let expired = stored
            .expires_at_unix_ms
            .is_some_and(|expires| now >= expires);
        let attached = driver.attach(self.inner.ctx.clone(), &stored).await;
        let Ok(Some(substrate)) = attached else {
            self.remove_stored(&stored.id).await;
            return;
        };
        if expired {
            let handle: Arc<dyn SubstrateHandle> = Arc::from(substrate.handle);
            let _ = handle.kill().await;
            let _ = handle.cleanup().await;
            self.remove_stored(&stored.id).await;
            return;
        }
        if stored.state != SandboxState::Ready {
            let handle: Arc<dyn SubstrateHandle> = Arc::from(substrate.handle);
            let _ = handle.kill().await;
            let _ = handle.cleanup().await;
            self.remove_stored(&stored.id).await;
            return;
        }
        self.restore_ready(stored, substrate).await;
    }

    async fn restore_ready(&self, stored: StoredRecord, substrate: Substrate) {
        let endpoint = substrate.endpoint.clone();
        let handle: Arc<dyn SubstrateHandle> = Arc::from(substrate.handle);
        let limits = stored.spec.limits.clone();
        let Ok(transport) = self
            .inner
            .connector
            .connect(&self.inner.ctx, &endpoint, &limits)
        else {
            let _ = handle.cleanup().await;
            self.remove_stored(&stored.id).await;
            return;
        };
        let Ok(client) = crate::shared::control::SlaveClient::new(
            self.inner.ctx.clone(),
            transport,
            limits.clone(),
        ) else {
            let _ = handle.cleanup().await;
            self.remove_stored(&stored.id).await;
            return;
        };
        let client = Arc::new(client);
        let info = client.info(limits.rpc_timeout).await;
        match info {
            Ok(info) if info.prepared => {
                let mut rebuilt = SandboxRecord::new(
                    stored.id.clone(),
                    stored.driver,
                    stored.spec.clone(),
                    stored.created_unix_ms,
                    stored.expires_at_unix_ms,
                );
                rebuilt.manifest.clone_from(&stored.manifest);
                let record = Arc::new(rebuilt);
                {
                    let mut state = record
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state.state = SandboxState::Ready;
                    state.endpoint = Some(endpoint);
                    state.substrate = Some(handle);
                    state.client = Some(client);
                    state.slave = Some(info);
                }
                self.inner.insert(Arc::clone(&record));
                start_watcher(Arc::clone(&self.inner), record);
            }
            _ => {
                let _ = handle.cleanup().await;
                self.remove_stored(&stored.id).await;
            }
        }
    }

    async fn remove_stored(&self, id: &SandboxId) {
        self.inner.remove(id);
        let _ = self.inner.store.remove(id).await;
    }

    /// The container control of the first driver in `drivers` that has one.
    #[must_use]
    pub fn containers(&self) -> Option<Arc<dyn ContainerControl>> {
        self.inner
            .drivers
            .iter()
            .find_map(|driver| driver.containers())
    }

    async fn do_create(
        &self,
        spec: SandboxSpec,
    ) -> SandboxResult<crate::shared::control::SandboxHandle> {
        crate::shared::control::create::create_ready(Arc::clone(&self.inner), spec).await
    }
}

impl Drop for SandboxManager {
    fn drop(&mut self) {
        if self.inner.config.on_drop != DropPolicy::KillAll {
            return;
        }
        for record in self.inner.records() {
            let inner = Arc::clone(&self.inner);
            spawn_detached(async move {
                let _ = kill_record(inner, record).await;
            });
        }
    }
}

impl SandboxProvider for SandboxManager {
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<HostReport>>> {
        let inner = Arc::clone(&self.inner);
        send_box(async move {
            let mut reports = Vec::with_capacity(inner.drivers.len());
            for driver in &inner.drivers {
                reports.push(driver.probe(inner.ctx.clone()).await);
            }
            Ok(reports)
        })
    }

    fn create(
        &self,
        spec: SandboxSpec,
    ) -> BoxFuture<'_, SandboxResult<crate::shared::control::SandboxHandle>> {
        let manager = self;
        send_box(async move { manager.do_create(spec).await })
    }

    fn create_with_progress(&self, spec: SandboxSpec) -> CreateStream {
        crate::shared::control::create::create_stream(Arc::clone(&self.inner), spec)
    }

    fn get(
        &self,
        id: SandboxId,
    ) -> BoxFuture<'_, SandboxResult<crate::shared::control::SandboxHandle>> {
        let inner = Arc::clone(&self.inner);
        send_box(async move {
            let record = inner.record(&id).ok_or_else(|| SandboxError::NotFound {
                what: "sandbox".to_string(),
            })?;
            let ops: Arc<dyn crate::shared::control::SandboxOps> =
                Arc::new(crate::shared::control::LocalSandboxOps::new(
                    Arc::clone(&inner),
                    record,
                    inner.ctx.clone(),
                    inner.config.limits.clone(),
                ));
            Ok(crate::shared::control::SandboxHandle::new(id, ops))
        })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>> {
        let inner = Arc::clone(&self.inner);
        send_box(async move { Ok(inner.records().iter().map(|record| record.info()).collect()) })
    }

    fn stop(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let inner = Arc::clone(&self.inner);
        send_box(async move {
            if let Some(record) = inner.record(&id) {
                return stop_record(inner, record).await;
            }
            inner.exit_report(&id).ok_or_else(|| {
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                }
                .into()
            })
        })
    }

    fn kill(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let inner = Arc::clone(&self.inner);
        send_box(async move {
            if let Some(record) = inner.record(&id) {
                return kill_record(inner, record).await;
            }
            inner.exit_report(&id).ok_or_else(|| {
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                }
                .into()
            })
        })
    }
}

/// Chooses the driver of one spec, with the rules of section 6.2.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] for an unknown driver,
/// [`SandboxError::DriverUnavailable`] for a named driver that cannot run,
/// [`SandboxError::NoDriver`] when no driver is usable, or the validation
/// error of the first usable driver.
pub async fn choose_driver(
    inner: &Inner,
    spec: &SandboxSpec,
) -> SandboxResult<(Arc<dyn SandboxDriver>, HostReport)> {
    if let Some(id) = spec.driver {
        let driver = inner.driver(id).ok_or_else(|| SandboxError::InvalidSpec {
            field: "driver".to_string(),
            reason: format!("the driver {id} is not registered"),
        })?;
        let report = driver.probe(inner.ctx.clone()).await;
        if !report.usable {
            return Err(SandboxError::DriverUnavailable {
                driver: id,
                reason: first_failed_check(&report),
            }
            .into());
        }
        let driver = driver;
        let capabilities = report.capabilities.clone();
        spec.validate(&capabilities)?;
        return Ok((driver, report));
    }

    let mut reports = Vec::with_capacity(inner.drivers.len());
    for driver in &inner.drivers {
        reports.push(driver.probe(inner.ctx.clone()).await);
    }
    let mut first_error = None;
    let mut chosen = None;
    for (driver, report) in inner.drivers.iter().zip(reports.iter()) {
        // A nested child names its driver and its parent: the validate rule
        // requires `driver: Nested` exactly when `parent` is set. So the
        // automatic choice never picks the nested driver (feature 11).
        if driver.id() == DriverId::Nested {
            continue;
        }
        if !report.usable {
            continue;
        }
        match spec.validate(&report.capabilities) {
            Ok(()) => {
                chosen = Some((Arc::clone(driver), report.clone()));
                break;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(chosen) = chosen {
        return Ok(chosen);
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Err(SandboxError::NoDriver { reports }.into())
}

/// The reason of an unusable driver: the message of the first failed check.
fn first_failed_check(report: &HostReport) -> String {
    report.checks.iter().find(|check| !check.ok).map_or_else(
        || "the driver is not usable on this host".to_string(),
        |check| check.message.clone(),
    )
}

/// The spec with the manager defaults where the caller kept the defaults.
#[must_use]
pub fn apply_default_limits(spec: &SandboxSpec, limits: &SandboxLimits) -> SandboxSpec {
    let mut spec = spec.clone();
    if spec.limits == SandboxLimits::default() {
        spec.limits = limits.clone();
    }
    spec
}

/// The creation time of a new record.
#[must_use]
pub fn record_created(ctx: &Ctx) -> u64 {
    created_unix_ms(ctx)
}
