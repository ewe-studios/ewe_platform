//! The sandbox record, its state machine and the record store.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use foundation_context::Ctx;

use crate::shared::control::{ExitReport, SandboxInfo, SandboxState};
use crate::shared::driver::{SlaveEndpoint, StoredRecord, SubstrateHandle};
use crate::shared::error::SandboxResult;
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::rpc::messages::SlaveInfo;
use crate::shared::spec::SandboxSpec;
use crate::shared::BoxFuture;

/// Where sandbox records are kept outside the manager's memory.
///
/// The in-memory store keeps nothing after the process ends. Feature 07 gives
/// a store on Durable Object (DO) storage.
pub trait RecordStore: Send + Sync + 'static {
    /// Stores a record, or replaces the record with the same identifier.
    fn put(&self, record: StoredRecord) -> BoxFuture<'_, SandboxResult<()>>;

    /// Removes the record with this identifier. A missing record is not an
    /// error.
    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>>;

    /// Every stored record.
    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<StoredRecord>>>;
}

/// A record store in process memory.
#[derive(Debug, Default)]
pub struct MemoryRecordStore {
    records: Mutex<HashMap<SandboxId, StoredRecord>>,
}

impl MemoryRecordStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RecordStore for MemoryRecordStore {
    fn put(&self, record: StoredRecord) -> BoxFuture<'_, SandboxResult<()>> {
        let id = record.id.clone();
        let records = &self.records;
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, record);
            Ok(())
        })
    }

    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        let id = id.clone();
        let records = &self.records;
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&id);
            Ok(())
        })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<StoredRecord>>> {
        let records = &self.records;
        Box::pin(async move {
            Ok(records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
                .cloned()
                .collect())
        })
    }
}

/// The lifetime deadline of one sandbox, as Unix milliseconds.
///
/// The create calls this one time and stores the result in the record. A
/// restart reads the stored value, so it never moves the deadline.
#[must_use]
pub fn lifetime_deadline(created_unix_ms: u64, ttl: Option<std::time::Duration>) -> Option<u64> {
    ttl.map(|ttl| {
        let millis = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
        created_unix_ms.saturating_add(millis)
    })
}

/// The changing part of one sandbox record.
pub struct RecordState {
    /// The state of the sandbox.
    pub state: SandboxState,
    /// How to reach the slave, once the driver answered.
    pub endpoint: Option<SlaveEndpoint>,
    /// The handle of the substrate.
    pub substrate: Option<Arc<dyn SubstrateHandle>>,
    /// The typed client of the slave.
    pub client: Option<Arc<crate::shared::control::SlaveClient>>,
    /// What the slave reported in `Info`.
    pub slave: Option<SlaveInfo>,
    /// How the sandbox ended, frozen at the first stop, kill or exit.
    pub exit: Option<ExitReport>,
}

/// One sandbox, as the manager and the handles share it.
pub struct SandboxRecord {
    /// The sandbox identifier.
    pub id: SandboxId,
    /// The driver that runs the sandbox.
    pub driver: DriverId,
    /// The spec that the caller gave.
    pub spec: SandboxSpec,
    /// The creation time as Unix milliseconds.
    pub created_unix_ms: u64,
    /// The lifetime deadline as Unix milliseconds, or `None` for no limit.
    pub expires_at_unix_ms: Option<u64>,
    /// The changing part.
    pub state: Mutex<RecordState>,
    /// The signal that a stop or kill finished.
    pub stop: StopGate,
}

impl SandboxRecord {
    /// A record in the `Creating` state.
    #[must_use]
    pub fn new(
        id: SandboxId,
        driver: DriverId,
        spec: SandboxSpec,
        created_unix_ms: u64,
        expires_at_unix_ms: Option<u64>,
    ) -> Self {
        Self {
            id,
            driver,
            spec,
            created_unix_ms,
            expires_at_unix_ms,
            state: Mutex::new(RecordState {
                state: SandboxState::Creating,
                endpoint: None,
                substrate: None,
                client: None,
                slave: None,
                exit: None,
            }),
            stop: StopGate::new(),
        }
    }

    /// What the control plane knows about this sandbox.
    #[must_use]
    pub fn info(&self) -> SandboxInfo {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        SandboxInfo {
            id: self.id.to_string(),
            driver: self.driver,
            state: state.state,
            created_unix_ms: self.created_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
            slave: state.slave.clone(),
        }
    }

    /// The record as it is stored outside the manager's memory.
    #[must_use]
    pub fn stored(&self) -> StoredRecord {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        StoredRecord {
            id: self.id.clone(),
            driver: self.driver,
            spec: self.spec.clone(),
            state: state.state,
            created_unix_ms: self.created_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
            endpoint: state.endpoint.clone(),
        }
    }

    /// Moves the state along an allowed edge.
    ///
    /// It returns `false` and changes nothing when the edge is not allowed.
    /// The caller holds the state lock.
    pub fn transition(state: &mut RecordState, next: SandboxState) -> bool {
        if !allowed_edge(state.state, next) {
            return false;
        }
        state.state = next;
        true
    }
}

/// Whether the state machine allows this edge.
#[must_use]
#[allow(
    clippy::unnested_or_patterns,
    reason = "the pairs read clearer grouped by the state they leave"
)]
pub fn allowed_edge(from: SandboxState, to: SandboxState) -> bool {
    matches!(
        (from, to),
        (
            SandboxState::Creating,
            SandboxState::Ready | SandboxState::Failed | SandboxState::Stopped
        ) | (
            SandboxState::Ready,
            SandboxState::Stopping | SandboxState::Failed
        ) | (SandboxState::Stopping, SandboxState::Stopped)
            | (SandboxState::Failed, SandboxState::Stopped)
    )
}

/// The one-time signal that carries the first `ExitReport` to every waiter.
#[derive(Debug, Default)]
pub struct StopGate {
    inner: Mutex<StopGateState>,
}

#[derive(Debug, Default)]
struct StopGateState {
    report: Option<ExitReport>,
    wakers: Vec<Waker>,
}

impl StopGate {
    /// A gate with no report yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores the report and wakes every waiter. The first report wins.
    pub fn complete(&self, report: ExitReport) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.report.is_some() {
            return;
        }
        state.report = Some(report);
        for waker in state.wakers.drain(..) {
            waker.wake();
        }
    }

    /// The stored report, when a stop or kill already finished.
    #[must_use]
    pub fn report(&self) -> Option<ExitReport> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .report
    }

    /// Waits for the first `ExitReport`.
    pub async fn wait(&self) -> ExitReport {
        futures::future::poll_fn(|context| self.poll_report(context)).await
    }

    fn poll_report(&self, context: &mut Context<'_>) -> Poll<ExitReport> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(report) = state.report {
            return Poll::Ready(report);
        }
        // Register the waker while the lock is held, so a completion that
        // happens between the check and the park cannot lose the wake.
        state.wakers.push(context.waker().clone());
        Poll::Pending
    }
}

/// The creation time as Unix milliseconds from the injected clock.
#[must_use]
pub fn created_unix_ms(ctx: &Ctx) -> u64 {
    let wall = ctx.clock().wall_now();
    wall.duration_since(foundation_compact::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
