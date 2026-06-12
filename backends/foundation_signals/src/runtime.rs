//! WHY: One unified reactive graph (decision 003): a single place that owns the
//! nodes, the dirty bookkeeping, and the flush. Everything else — getters,
//! setters, contexts — is a thin handle over this.
//!
//! WHAT: [`Runtime`] — the R3-style graph: bucket-queue `dirty_heap` indexed by
//! height, global version counter, `active` node for dynamic dependency
//! tracking, deferred removals, the JS-interop callback registry, and
//! [`stabilize`](Runtime::stabilize) — the ONLY entry point that flushes dirty
//! nodes (`set()` never propagates inline).
//!
//! HOW: The graph lives in one `RefCell` (`single_threaded`, G15). The borrow
//! discipline is the heart of the file: user closures (effects, computeds,
//! callbacks, notification managers) are ALWAYS run with the graph borrow
//! RELEASED — they re-enter through getters/setters, which take their own short
//! borrows. Stabilize pops a node, clones its `Rc` eval closure, drops the
//! borrow, runs the closure, re-borrows to commit tracking results.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use crate::arena::{Arena, NodeId};
use crate::callback::EventData;
use crate::node::{Node, ThreeState};
use crate::notification::NotificationManager;

#[cfg(not(feature = "single_threaded"))]
compile_error!(
    "foundation_signals: only the `single_threaded` feature path is implemented \
     (G15 — WASM is single-threaded; the Mutex-backed path awaits a real consumer)"
);

/// The unified reactive graph. Created once (wrap it in `Arc`/`Rc`), passed
/// explicitly — never a thread-local singleton.
pub struct Runtime {
    graph: RefCell<Graph>,
}

/// All mutable graph state, behind the one cell.
pub(crate) struct Graph {
    pub(crate) nodes: Arena,
    /// Bucket queue: `dirty[height]` holds nodes awaiting processing. Grows on
    /// demand; cheap level-by-level iteration without sorting. FIFO within a
    /// height: effects at the same height run in CREATION/marking order, so
    /// order-dependent op streams (DOM ops) stay deterministic.
    dirty: Vec<VecDeque<NodeId>>,
    /// Global change counter. Bumped by every value-changing write and at the
    /// start of every stabilize — any increase means "something changed".
    version: u64,
    /// The node currently evaluating (dependency reads attach to it).
    active: Option<NodeId>,
    /// Dependency reads recorded for `active` during the current evaluation.
    trail: Vec<NodeId>,
    /// Nodes disposed mid-stabilize; unlinked after the dirty loop drains.
    pending_removals: Vec<NodeId>,
    /// True while stabilize's dirty loop runs (routes disposals to deferral).
    stabilizing: bool,
    /// JS-interop callback registry (G17). Ids are NEVER reused.
    callbacks: BTreeMap<u64, Box<dyn FnMut(EventData)>>,
    next_callback_id: u64,
    /// Monotonic instance-id allocator for `html!` mounts (G20).
    next_instance_id: u32,
    managers: Vec<Box<dyn NotificationManager>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: RefCell::new(Graph {
                nodes: Arena::default(),
                dirty: Vec::new(),
                version: 0,
                active: None,
                trail: Vec::new(),
                pending_removals: Vec::new(),
                stabilizing: false,
                callbacks: BTreeMap::new(),
                next_callback_id: 0,
                next_instance_id: 0,
                managers: Vec::new(),
            }),
        }
    }

    pub(crate) fn graph(&self) -> std::cell::RefMut<'_, Graph> {
        self.graph.borrow_mut()
    }

    /// Register a post-stabilize listener (telemetry, JS interop). Managers
    /// fire after every stabilize, in registration order (G16).
    pub fn add_notification_manager(&self, manager: Box<dyn NotificationManager>) {
        self.graph().managers.push(manager);
    }

    // ─── Callback registry (G17 / two-way binding) ─────────────────────────────

    /// Allocate the next interop callback id (monotonic, never reused).
    pub(crate) fn next_callback_id(&self) -> u64 {
        let mut graph = self.graph();
        let id = graph.next_callback_id;
        graph.next_callback_id += 1;
        id
    }

    /// Register (or replace) the callback dispatched by [`invoke_callback`]
    /// for `id`. The `html!` macro's two-way-binding codegen calls this with
    /// the id it read from `SignalSetter::callback_id`.
    ///
    /// [`invoke_callback`]: Runtime::invoke_callback
    pub fn register_callback(&self, id: u64, callback: impl FnMut(EventData) + 'static) {
        self.graph().callbacks.insert(id, Box::new(callback));
    }

    /// Dispatch an event to callback `id`. Returns `false` for stale ids
    /// (disposed signals) — expected during teardown races, silently dropped.
    ///
    /// The callback typically calls a setter; call [`stabilize`] afterwards to
    /// flush the effects it dirtied.
    ///
    /// [`stabilize`]: Runtime::stabilize
    pub fn invoke_callback(&self, id: u64, data: EventData) -> bool {
        // Take the callback OUT while it runs — it will re-enter the graph
        // through setters, and a fresh registration for the same id mid-call
        // would be a bug anyway (ids are never reused).
        let callback = self.graph().callbacks.remove(&id);
        let Some(mut callback) = callback else {
            return false;
        };
        callback(data);
        // Put it back unless the callback disposed its own signal meanwhile.
        let mut graph = self.graph();
        if graph.next_callback_id > id && !graph.callbacks.contains_key(&id) {
            graph.callbacks.insert(id, callback);
        }
        true
    }

    /// JSON leg of [`invoke_callback`](Runtime::invoke_callback): deserialize
    /// `EventData` and dispatch. Undeserializable payloads are logged and
    /// dropped (the signal keeps its previous value).
    pub fn invoke_callback_json(&self, id: u64, json: &str) -> bool {
        match serde_json::from_str::<EventData>(json) {
            Ok(data) => self.invoke_callback(id, data),
            Err(error) => {
                tracing::warn!(id, %error, "invoke_callback_json: bad EventData payload");
                false
            }
        }
    }

    /// Allocate a contiguous block of `count` instance ids (G20). Blocks are
    /// never reused; see `Context::allocate_id_block`.
    ///
    /// # Panics
    /// Panics on 32-bit id-space exhaustion.
    #[must_use]
    pub fn allocate_id_block(&self, count: u32) -> u32 {
        let mut graph = self.graph();
        let base = graph.next_instance_id;
        graph.next_instance_id = base
            .checked_add(count)
            .expect("html! instance id space (u32) exhausted");
        base
    }

    /// True if `id` is currently registered (stale ids return false).
    #[must_use]
    pub fn has_callback(&self, id: u64) -> bool {
        self.graph.borrow().callbacks.contains_key(&id)
    }

    /// Attach `cleanup` to the effect currently evaluating (it runs before the
    /// next re-run and at disposal; attaching again replaces the previous one).
    /// Returns the closure back if no effect is evaluating, so callers
    /// (`Context::on_cleanup`) can fall back to scope-level cleanup.
    pub fn on_cleanup_active(&self, cleanup: Box<dyn FnOnce()>) -> Option<Box<dyn FnOnce()>> {
        let mut graph = self.graph();
        let Some(active) = graph.active else {
            return Some(cleanup);
        };
        match graph.nodes.get_mut(active) {
            Some(Node::Effect(effect)) => {
                effect.cleanup = Some(cleanup);
                None
            }
            _ => Some(cleanup),
        }
    }

    // ─── Dirty marking ─────────────────────────────────────────────────────────

    /// A signal value changed: bump the global version, stamp the node, mark
    /// observers Dirty. Called by `SignalSetter::set` — never propagates inline;
    /// the dirty nodes wait for [`stabilize`](Runtime::stabilize).
    pub(crate) fn signal_changed(&self, id: NodeId) {
        let mut graph = self.graph();
        graph.version += 1;
        let version = graph.version;
        let Some(Node::Signal(node)) = graph.nodes.get_mut(id) else {
            return; // disposed — stale setter, no-op
        };
        node.version = version;
        let observers = node.observers.clone();
        for observer in observers {
            graph.mark(observer, ThreeState::Dirty);
        }
    }

    // ─── Tracked evaluation ────────────────────────────────────────────────────

    /// Run `eval` with dependency tracking for `id`: reads via getters record
    /// themselves, then the dep diff is committed (un/relink + height).
    /// The graph borrow is RELEASED around `eval` — user code re-enters freely.
    pub(crate) fn tracked<R>(&self, id: NodeId, eval: impl FnOnce() -> R) -> R {
        let prev_deps = {
            let mut graph = self.graph();
            debug_assert!(graph.active.is_none(), "tracked evaluations never nest");
            graph.active = Some(id);
            let prev = match graph.nodes.get_mut(id).and_then(Node::deps_mut) {
                Some(deps) => std::mem::take(deps),
                None => Vec::new(),
            };
            graph.trail.clear();
            prev
        };

        let out = eval();

        let mut graph = self.graph();
        graph.active = None;
        let new_deps = std::mem::take(&mut graph.trail);
        graph.commit_tracking(id, &prev_deps, new_deps);
        out
    }

    /// Record a dependency read: the ACTIVE node (if any) depends on `read`.
    /// Called by every getter.
    pub(crate) fn track_read(&self, read: NodeId) {
        let mut graph = self.graph();
        if graph.active.is_some() && !graph.trail.contains(&read) {
            graph.trail.push(read);
        }
    }

    // ─── stabilize ─────────────────────────────────────────────────────────────

    /// Flush all dirty nodes in height order — the single propagation entry
    /// point. O(dirty nodes); diamonds resolve to one evaluation per node
    /// (height ordering). Re-entrant `set()`s during effects land in the heap:
    /// same pass if their height is still ahead, next stabilize otherwise.
    pub fn stabilize(&self) {
        {
            let mut graph = self.graph();
            if graph.stabilizing {
                // G16 / re-entrancy: stabilize never recurses.
                return;
            }
            graph.stabilizing = true;
            graph.version += 1;
        }

        let mut height = 0usize;
        loop {
            // Pop one node at this height (heap may GROW during the pass).
            let popped = {
                let mut graph = self.graph();
                if height >= graph.dirty.len() {
                    graph.stabilizing = false;
                    break;
                }
                graph.dirty[height].pop_front()
            };
            let Some(id) = popped else {
                height += 1;
                continue;
            };
            self.process(id);
        }

        // Deferred disposals (mid-stabilize context drops).
        let removals = std::mem::take(&mut self.graph().pending_removals);
        for id in removals {
            self.graph().unlink_and_remove(id);
        }

        // Notification managers: after the loop, registration order, borrow
        // released (managers may set signals — they dirty the NEXT stabilize).
        let mut managers = std::mem::take(&mut self.graph().managers);
        for manager in &mut managers {
            manager.on_stabilize_complete();
        }
        let mut graph = self.graph();
        managers.extend(std::mem::take(&mut graph.managers)); // keep any added during firing
        graph.managers = managers;
    }

    /// Process one popped node according to the [`ThreeState`] transition rules.
    // The snapshot/plan/act phases are one state machine; splitting them into
    // helpers would scatter the borrow discipline this function exists to keep.
    #[allow(clippy::too_many_lines)]
    fn process(&self, id: NodeId) {
        enum Plan {
            Skip,
            EvalComputed(Rc<RefCell<dyn FnMut() -> bool>>),
            RunEffect(Rc<RefCell<dyn FnMut()>>, Option<Box<dyn FnOnce()>>),
        }

        // Snapshot what the node IS under one short borrow, then decide.
        enum Kind {
            Other,
            Computed {
                state: ThreeState,
                eval: Rc<RefCell<dyn FnMut() -> bool>>,
                dep_info: Vec<(NodeId, u64)>,
            },
            Effect {
                state: ThreeState,
                run: Rc<RefCell<dyn FnMut()>>,
            },
        }

        let kind = {
            let graph = self.graph();
            match graph.nodes.get(id) {
                None | Some(Node::Signal(_)) => Kind::Other, // disposed while queued / source
                Some(Node::Computed(computed)) => Kind::Computed {
                    state: computed.state,
                    eval: computed.eval.clone(),
                    dep_info: computed
                        .deps
                        .iter()
                        .copied()
                        .zip(computed.dep_versions.iter().copied())
                        .collect(),
                },
                Some(Node::Effect(effect)) => Kind::Effect {
                    state: effect.state,
                    run: effect.run.clone(),
                },
            }
        };

        let plan = match kind {
            Kind::Other => Plan::Skip,
            Kind::Computed {
                state,
                eval,
                dep_info,
            } => match state {
                ThreeState::Clean => Plan::Skip,
                ThreeState::Dirty => Plan::EvalComputed(eval),
                ThreeState::Check => {
                    // Check resolution: did any DIRECT dep's version move past
                    // what we saw at our last evaluation?
                    let mut graph = self.graph();
                    let changed = dep_info.iter().any(|(dep, seen)| {
                        graph.nodes.get(*dep).is_none_or(|d| d.version() > *seen)
                    });
                    if changed {
                        Plan::EvalComputed(eval) // promote to Dirty handling
                    } else {
                        // Short-circuit: nothing really changed upstream.
                        if let Some(node) = graph.nodes.get_mut(id) {
                            node.set_state(ThreeState::Clean);
                        }
                        Plan::Skip
                    }
                }
            },
            Kind::Effect { state, run } => {
                if state == ThreeState::Dirty {
                    let cleanup = match self.graph().nodes.get_mut(id) {
                        Some(Node::Effect(e)) => e.cleanup.take(),
                        _ => None,
                    };
                    Plan::RunEffect(run, cleanup)
                } else {
                    Plan::Skip
                }
            }
        };

        match plan {
            Plan::Skip => {}
            Plan::EvalComputed(eval) => {
                // Tracked re-evaluation; the closure compares against its typed
                // cache and reports whether the value changed.
                let changed = self.tracked(id, || (eval.borrow_mut())());
                let mut graph = self.graph();
                let version = graph.version;
                if let Some(Node::Computed(computed)) = graph.nodes.get_mut(id) {
                    computed.state = ThreeState::Clean;
                    if changed {
                        computed.version = version;
                    }
                }
                if changed {
                    // Computed observers get Check (might not change for them);
                    // effect observers get Dirty.
                    let observers = graph
                        .nodes
                        .get(id)
                        .map(|n| n.observers().to_vec())
                        .unwrap_or_default();
                    for observer in observers {
                        let state = match graph.nodes.get(observer) {
                            Some(Node::Computed(_)) => ThreeState::Check,
                            Some(Node::Effect(_)) => ThreeState::Dirty,
                            _ => continue,
                        };
                        graph.mark(observer, state);
                    }
                }
            }
            Plan::RunEffect(run, cleanup) => {
                if let Some(cleanup) = cleanup {
                    cleanup(); // before re-evaluation, borrow released
                }
                self.tracked(id, || (run.borrow_mut())());
                if let Some(node) = self.graph().nodes.get_mut(id) {
                    node.set_state(ThreeState::Clean);
                }
            }
        }
    }

    // ─── Disposal ──────────────────────────────────────────────────────────────

    /// Dispose `id`: run an effect's cleanup, then unlink (immediately, or
    /// deferred to the end of the in-flight stabilize — mid-loop removal would
    /// corrupt the heap walk).
    pub(crate) fn dispose_node(&self, id: NodeId) {
        let cleanup = {
            let mut graph = self.graph();
            match graph.nodes.get_mut(id) {
                Some(Node::Effect(effect)) => effect.cleanup.take(),
                _ => None,
            }
        };
        if let Some(cleanup) = cleanup {
            cleanup(); // user code — borrow released
        }
        let mut graph = self.graph();
        if !graph.nodes.contains(id) {
            return; // double-dispose: no-op
        }
        if graph.stabilizing {
            graph.pending_removals.push(id);
        } else {
            graph.unlink_and_remove(id);
        }
    }
}

impl Graph {
    /// Mark a node and queue it at its height (idempotent per state upgrade:
    /// Dirty wins over Check; Clean nodes always (re)queue).
    fn mark(&mut self, id: NodeId, state: ThreeState) {
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        let current = node.state();
        let next = match (current, state) {
            (ThreeState::Dirty, _) | (_, ThreeState::Check) if current != ThreeState::Clean => {
                return; // already queued at >= requested strength
            }
            _ => state,
        };
        node.set_state(next);
        if current == ThreeState::Clean {
            let height = node.height() as usize;
            if self.dirty.len() <= height {
                self.dirty.resize_with(height + 1, VecDeque::new);
            }
            self.dirty[height].push_back(id);
        }
    }

    /// Commit a tracked evaluation's dependency diff: unlink stale deps, link
    /// new ones, snapshot dep versions, recompute height.
    fn commit_tracking(&mut self, id: NodeId, prev_deps: &[NodeId], new_deps: Vec<NodeId>) {
        // Unlink deps that were read before but not this time.
        for dep in prev_deps {
            if !new_deps.contains(dep) {
                if let Some(observers) = self.nodes.get_mut(*dep).and_then(Node::observers_mut) {
                    observers.retain(|o| *o != id);
                }
            }
        }
        // Link deps that are new this time.
        for dep in &new_deps {
            if !prev_deps.contains(dep) {
                if let Some(observers) = self.nodes.get_mut(*dep).and_then(Node::observers_mut) {
                    if !observers.contains(&id) {
                        observers.push(id);
                    }
                }
            }
        }
        // Height = max(dep heights) + 1; snapshot versions for Check resolution.
        let mut height = 0u32;
        let mut dep_versions = Vec::with_capacity(new_deps.len());
        for dep in &new_deps {
            if let Some(node) = self.nodes.get(*dep) {
                height = height.max(node.height() + 1);
                dep_versions.push(node.version());
            } else {
                dep_versions.push(0);
            }
        }
        height = height.max(1); // dependents are never sources

        match self.nodes.get_mut(id) {
            Some(Node::Computed(computed)) => {
                computed.deps = new_deps;
                computed.dep_versions = dep_versions;
                computed.height = height;
            }
            Some(Node::Effect(effect)) => {
                effect.deps = new_deps;
                effect.height = height;
            }
            _ => {}
        }
    }

    /// Remove `id` from the graph: detach from deps' observer lists and
    /// observers' dep lists, drop its interop callback, free the slot.
    pub(crate) fn unlink_and_remove(&mut self, id: NodeId) {
        let Some(node) = self.nodes.remove(id) else {
            return; // already gone (double dispose) — no-op
        };
        let (deps, observers, callback_id) = match node {
            Node::Signal(n) => (Vec::new(), n.observers, Some(n.callback_id)),
            Node::Computed(n) => (n.deps, n.observers, None),
            Node::Effect(n) => (n.deps, Vec::new(), None),
        };
        for dep in deps {
            if let Some(list) = self.nodes.get_mut(dep).and_then(Node::observers_mut) {
                list.retain(|o| *o != id);
            }
        }
        for observer in observers {
            if let Some(list) = self.nodes.get_mut(observer).and_then(Node::deps_mut) {
                list.retain(|d| *d != id);
            }
        }
        if let Some(callback_id) = callback_id {
            self.callbacks.remove(&callback_id);
        }
    }
}
