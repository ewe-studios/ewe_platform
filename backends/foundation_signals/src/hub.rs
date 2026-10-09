//! WHY: Native consumers running under valtron's multi pool need to write,
//! read, and react to signals from worker threads WITHOUT locking the graph
//! (feature 18, born from the F02 RefCell-vs-Mutex review). The actor seam
//! keeps the graph single-threaded and extends GLITCH-FREEDOM across threads:
//! remote readers only ever observe post-stabilize snapshots.
//!
//! WHAT: [`SignalHub`] (signal-thread owner: command queue + exposure
//! registry + publishers), [`HubHandle`] / [`RemoteSetter`] /
//! [`RemoteGetter`] / [`SignalStream`] (the `Send + Sync + Clone` worker
//! surface), and [`HubDriver`] (a valtron `TaskIterator` that pumps).
//!
//! HOW: Commands flow IN through one MPSC queue; `pump()` applies the batch,
//! runs ONE `stabilize()`, and publisher EFFECTS (ordinary graph effects the
//! hub installs per exposure) copy each post-stabilize value into an
//! `Arc<RwLock<…>>` watch cell and fan out to subscriber channels. The
//! `Rc`-based setters never cross threads — only `RemoteId`s and `Send`
//! values do; the hub-side registry holds the typed apply functions.
//!
//! Review defaults adopted (features/18 §6, recorded in status.md): streams
//! are unbounded for v1 (state reads go through `snapshot()`; a latest-wins
//! ring is a follow-up), the command queue is unbounded (decision 009),
//! names as designed, in-crate `valtron` feature.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, RwLock};

use crate::{Context, Runtime, SignalGetter, SignalSetter};

/// The hub no longer exists — its runtime/thread shut down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubGone;

impl core::fmt::Display for HubGone {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("signal hub is gone (runtime thread shut down)")
    }
}

impl std::error::Error for HubGone {}

/// What one `pump()` did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PumpReport {
    pub commands_applied: usize,
    pub stabilized: bool,
}

/// A command crossing into the signal thread.
type HubCommand = Box<dyn FnOnce(&HubScope<'_>) + Send>;

/// A type-erased setter operation (values cross, setters never do).
/// A boxed, thread-crossing in-place mutation.
type ErasedUpdate = Box<dyn FnOnce(&mut dyn Any) + Send>;

enum SetterOp {
    Set(Box<dyn Any + Send>),
    Update(ErasedUpdate),
}

type SetterApply = Box<dyn Fn(SetterOp)>;

/// Owner side — lives on the thread that owns the [`Runtime`]. NOT `Send`.
pub struct SignalHub {
    runtime: Rc<Runtime>,
    ctx: Context,
    rx: Receiver<HubCommand>,
    tx: Sender<HubCommand>,
    registry: Rc<RefCell<HashMap<u64, SetterApply>>>,
    next_remote_id: Rc<RefCell<u64>>,
}

impl SignalHub {
    #[must_use]
    pub fn new(runtime: &Rc<Runtime>) -> Self {
        let (tx, rx) = channel();
        Self {
            runtime: Rc::clone(runtime),
            ctx: Context::new(Rc::clone(runtime)),
            rx,
            tx,
            registry: Rc::new(RefCell::new(HashMap::new())),
            next_remote_id: Rc::new(RefCell::new(0)),
        }
    }

    /// The `Send + Sync + Clone` handle workers use.
    #[must_use]
    pub fn handle(&self) -> HubHandle {
        HubHandle {
            tx: self.tx.clone(),
        }
    }

    /// Drain queued commands, apply them, stabilize ONCE, publish. The only
    /// place remote traffic touches the graph (feature 18 §3.1 FIFO).
    #[must_use]
    pub fn pump(&self) -> PumpReport {
        let mut applied = 0;
        while let Ok(command) = self.rx.try_recv() {
            let scope = HubScope { hub: self };
            command(&scope);
            applied += 1;
        }
        if applied > 0 {
            // Publisher effects run INSIDE this stabilize — remote snapshots
            // move from one stable state to the next, never in between.
            self.runtime.stabilize();
        }
        PumpReport {
            commands_applied: applied,
            stabilized: applied > 0,
        }
    }

    /// Expose a signal for remote READING (§2.4): installs a publisher effect
    /// copying each post-stabilize value into a watch cell + streams.
    #[must_use]
    pub fn expose<T>(&self, getter: &SignalGetter<T>) -> RemoteGetter<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let cell = Arc::new(RwLock::new((0u64, None::<T>)));
        let subscribers: Subscribers<T> = Arc::new(Mutex::new(Vec::new()));

        let publish_cell = Arc::clone(&cell);
        let publish_subs = Arc::clone(&subscribers);
        let read = getter.clone();
        self.ctx.effect(move || {
            let value = read.get(); // tracked — re-runs on change
            {
                let mut slot = publish_cell.write().unwrap_or_else(std::sync::PoisonError::into_inner);
                slot.0 += 1;
                slot.1 = Some(value.clone());
            }
            let mut subs = publish_subs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            subs.retain(|sender: &Sender<T>| sender.send(value.clone()).is_ok());
        });

        // Stream END semantics: when the hub's scope disposes (hub drop), the
        // senders drop and every `SignalStream` reports `HubGone` after its
        // queue drains — without this, the shared subscriber vec would keep
        // the senders alive as long as any RemoteGetter clone exists.
        let cleanup_subs = Arc::clone(&subscribers);
        self.ctx.on_cleanup(move || {
            cleanup_subs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        });

        RemoteGetter { cell, subscribers }
    }

    /// Expose a setter for remote WRITING (§2.3): registers the typed apply
    /// fn under a `Send` id — only values cross threads.
    #[must_use]
    pub fn expose_setter<T>(&self, setter: &SignalSetter<T>) -> RemoteSetter<T>
    where
        T: Clone + PartialEq + Send + 'static,
    {
        let id = {
            let mut next = self.next_remote_id.borrow_mut();
            *next += 1;
            *next
        };
        let target = setter.clone();
        self.registry.borrow_mut().insert(
            id,
            Box::new(move |op| match op {
                SetterOp::Set(any) => {
                    if let Ok(value) = any.downcast::<T>() {
                        target.set(*value);
                    }
                }
                SetterOp::Update(f) => target.update(|v| f(v as &mut dyn Any)),
            }),
        );
        RemoteSetter {
            tx: self.tx.clone(),
            registry_id: id,
            _value: std::marker::PhantomData,
        }
    }

    /// Both directions at once.
    #[must_use]
    pub fn expose_pair<T>(
        &self,
        getter: &SignalGetter<T>,
        setter: &SignalSetter<T>,
    ) -> (RemoteGetter<T>, RemoteSetter<T>)
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        (self.expose(getter), self.expose_setter(setter))
    }
}

/// What `run_on_hub` closures receive: scoped access on the signal thread.
pub struct HubScope<'hub> {
    hub: &'hub SignalHub,
}

impl HubScope<'_> {
    /// The hub's own context (create signals/effects/computeds here).
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.hub.ctx
    }

    /// The runtime (callback registration etc.).
    #[must_use]
    pub fn runtime(&self) -> &Rc<Runtime> {
        &self.hub.runtime
    }

    /// Mint remote handles from inside a `run_on_hub` closure (§3.4).
    #[must_use]
    pub fn expose<T: Clone + Send + Sync + 'static>(
        &self,
        getter: &SignalGetter<T>,
    ) -> RemoteGetter<T> {
        self.hub.expose(getter)
    }

    #[must_use]
    pub fn expose_setter<T: Clone + PartialEq + Send + 'static>(
        &self,
        setter: &SignalSetter<T>,
    ) -> RemoteSetter<T> {
        self.hub.expose_setter(setter)
    }

    fn apply_setter(&self, id: u64, op: SetterOp) {
        // Stale ids (disposed exposures) drop silently — the same contract as
        // stale callback ids (§3.5).
        if let Some(apply) = self.hub.registry.borrow().get(&id) {
            apply(op);
        }
    }
}

// ─── Worker-side handles ───────────────────────────────────────────────────────

/// Run arbitrary code on the signal thread — THE escape hatch (§2.2).
#[derive(Clone)]
pub struct HubHandle {
    tx: Sender<HubCommand>,
}

impl HubHandle {
    /// Queue `f` for the next pump. The closure crosses threads ONCE, then
    /// runs single-threaded with full hub access.
    ///
    /// # Errors
    /// [`HubGone`] when the hub (and its runtime thread) no longer exists.
    pub fn run_on_hub(&self, f: impl FnOnce(&HubScope<'_>) + Send + 'static) -> Result<(), HubGone> {
        self.tx.send(Box::new(f)).map_err(|_| HubGone)
    }

    /// Dispatch an interop callback remotely (worker-side event sources).
    ///
    /// # Errors
    /// [`HubGone`] when the hub no longer exists.
    pub fn invoke_callback(&self, id: u64, data: crate::EventData) -> Result<(), HubGone> {
        self.run_on_hub(move |scope| {
            let _ = scope.runtime().invoke_callback(id, data);
        })
    }
}

/// Remote write handle (§2.3) — `Send + Sync + Clone`.
pub struct RemoteSetter<T> {
    tx: Sender<HubCommand>,
    registry_id: u64,
    _value: std::marker::PhantomData<fn(T)>,
}

impl<T> Clone for RemoteSetter<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            registry_id: self.registry_id,
            _value: std::marker::PhantomData,
        }
    }
}

impl<T: Clone + PartialEq + Send + 'static> RemoteSetter<T> {
    /// Enqueue a write; lands at the next pump. `PartialEq` dedup happens on
    /// the signal thread, same as a local set.
    ///
    /// # Errors
    /// [`HubGone`] when the hub no longer exists.
    pub fn set(&self, value: T) -> Result<(), HubGone> {
        let id = self.registry_id;
        self.tx
            .send(Box::new(move |scope: &HubScope<'_>| {
                scope.apply_setter(id, SetterOp::Set(Box::new(value)));
            }))
            .map_err(|_| HubGone)
    }

    /// Enqueue an in-place update (the closure crosses once).
    ///
    /// # Errors
    /// [`HubGone`] when the hub no longer exists.
    pub fn update(&self, f: impl FnOnce(&mut T) + Send + 'static) -> Result<(), HubGone> {
        let id = self.registry_id;
        let erased: ErasedUpdate = Box::new(move |any| {
            if let Some(value) = any.downcast_mut::<T>() {
                f(value);
            }
        });
        self.tx
            .send(Box::new(move |scope: &HubScope<'_>| {
                scope.apply_setter(id, SetterOp::Update(erased));
            }))
            .map_err(|_| HubGone)
    }
}

type Subscribers<T> = Arc<Mutex<Vec<Sender<T>>>>;

/// Remote read handle (§2.4) — `Send + Sync + Clone`. Reads NEVER touch the
/// graph: `snapshot()` returns the last POST-STABILIZE value (glitch-freedom
/// across threads); `changes()` yields each published value.
pub struct RemoteGetter<T> {
    cell: Arc<RwLock<(u64, Option<T>)>>,
    subscribers: Subscribers<T>,
}

impl<T> Clone for RemoteGetter<T> {
    fn clone(&self) -> Self {
        Self {
            cell: Arc::clone(&self.cell),
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> RemoteGetter<T> {
    /// The latest post-stabilize value (`None` only before the exposure's
    /// first publish — which happens at expose time, decision 008).
    #[must_use]
    pub fn snapshot(&self) -> Option<T> {
        self.cell
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1
            .clone()
    }

    /// Monotonic publish counter — equal means "nothing changed since".
    #[must_use]
    pub fn version(&self) -> u64 {
        self.cell
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
    }

    /// A change stream: every post-stabilize value from subscription onward.
    #[must_use]
    pub fn changes(&self) -> SignalStream<T> {
        let (tx, rx) = channel();
        self.subscribers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(tx);
        SignalStream { rx }
    }
}

/// Post-stabilize value stream. Drive it on any executor; under the
/// `valtron` feature it is a `TaskIterator` (never blocks in `next_status`).
pub struct SignalStream<T> {
    rx: Receiver<T>,
}

impl<T> SignalStream<T> {
    /// Non-blocking poll: `Ok(Some(v))` value, `Ok(None)` empty,
    /// `Err(HubGone)` publisher dropped.
    ///
    /// # Errors
    /// [`HubGone`] once the publishing hub is gone AND the queue is drained.
    pub fn try_next(&self) -> Result<Option<T>, HubGone> {
        match self.rx.try_recv() {
            Ok(value) => Ok(Some(value)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(HubGone),
        }
    }
}

#[cfg(feature = "valtron")]
mod valtron_impls {
    use foundation_core::valtron::{NoSpawner, TaskIterator, TaskStatus};

    use super::{PumpReport, SignalHub, SignalStream, TryRecvError};

    impl<T: Send + 'static> TaskIterator for SignalStream<T> {
        type Ready = T;
        type Pending = ();
        type Spawner = NoSpawner;

        fn next_status(&mut self) -> Option<TaskStatus<T, (), NoSpawner>> {
            // Never blocks (project rule): empty queue is Pending.
            match self.rx.try_recv() {
                Ok(value) => Some(TaskStatus::Ready(value)),
                Err(TryRecvError::Empty) => Some(TaskStatus::Pending(())),
                Err(TryRecvError::Disconnected) => None,
            }
        }
    }

    /// Drives a hub as a valtron task: each non-empty pump yields a report;
    /// idle ticks are Pending (the executor re-polls / backs off).
    pub struct HubDriver {
        hub: SignalHub,
    }

    impl HubDriver {
        #[must_use]
        pub fn new(hub: SignalHub) -> Self {
            Self { hub }
        }
    }

    impl TaskIterator for HubDriver {
        type Ready = PumpReport;
        type Pending = ();
        type Spawner = NoSpawner;

        fn next_status(&mut self) -> Option<TaskStatus<PumpReport, (), NoSpawner>> {
            let report = self.hub.pump();
            if report.commands_applied > 0 {
                Some(TaskStatus::Ready(report))
            } else {
                Some(TaskStatus::Pending(()))
            }
        }
    }
}

#[cfg(feature = "valtron")]
pub use valtron_impls::HubDriver;
