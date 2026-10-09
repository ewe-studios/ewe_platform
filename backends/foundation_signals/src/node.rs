//! WHY: The reactive graph needs uniform storage for its three node kinds so the
//! stabilize loop, dependency tracking, and disposal treat them through one
//! arena — while values stay TYPED in per-handle storages (no `Box<dyn Any>`
//! downcasts; see the module note below).
//!
//! WHAT: [`Node`] and its three payloads — [`SignalNode`], [`ComputedNode`],
//! [`EffectNode`] — plus the [`ThreeState`] dirty-tracking enum (decision 002).
//!
//! HOW: Nodes hold GRAPH data only: observers, deps, heights, versions, state,
//! and type-erased *evaluation closures*. The closures capture the typed
//! `Arc<...Storage<T>>` handles, so re-evaluation reads/writes/compares values
//! with full type information and merely reports "did it change?" back to the
//! graph. The feature-02 spec sketches `Box<dyn Any>` value slots inside the
//! nodes; capturing typed storage in the eval closure keeps identical observable
//! semantics with no downcast failure path (spec G18 becomes unreachable).

use std::cell::RefCell;
use std::rc::Rc;

use crate::arena::NodeId;

/// Dirty-tracking state (decision 002 / R3 three-state flags).
///
/// - `Clean` — value is current.
/// - `Check` — a TRANSITIVE dependency changed; direct deps must be verified
///   before deciding to re-evaluate (enables the short-circuit).
/// - `Dirty` — a DIRECT dependency changed; must re-evaluate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreeState {
    Clean,
    Check,
    Dirty,
}

/// One graph node. All three kinds live in the same arena so ids are uniform.
pub(crate) enum Node {
    Signal(SignalNode),
    Computed(ComputedNode),
    Effect(EffectNode),
}

/// A source value. Height 0 by definition; never re-evaluated.
pub(crate) struct SignalNode {
    /// Nodes that read this signal during their last evaluation.
    pub observers: Vec<NodeId>,
    /// Global version at which the value last changed.
    pub version: u64,
    /// The interop callback id assigned at creation (G17); removed from the
    /// registry when this node is disposed.
    pub callback_id: u64,
}

/// A derived value. The eval closure recomputes the TYPED cache it captures and
/// returns whether the cached value changed (`PartialEq`).
pub(crate) struct ComputedNode {
    /// Re-evaluate into the captured `ComputedStorage<T>`; returns "changed?".
    /// `Rc<RefCell<..>>` so stabilize can run it without holding the graph borrow.
    pub eval: Rc<RefCell<dyn FnMut() -> bool>>,
    /// `max(dep heights) + 1`, recomputed by `endTracking`.
    pub height: u32,
    /// Nodes read during the last evaluation.
    pub deps: Vec<NodeId>,
    /// Versions of `deps` observed at the last evaluation — the Check
    /// short-circuit compares these against the deps' current versions.
    pub dep_versions: Vec<u64>,
    pub state: ThreeState,
    pub observers: Vec<NodeId>,
    /// Global version at which the cached value last changed.
    pub version: u64,
}

/// A side-effect. Re-runs whenever a dependency changes; owns an optional
/// cleanup that runs before each re-run and on disposal.
pub(crate) struct EffectNode {
    pub run: Rc<RefCell<dyn FnMut()>>,
    pub height: u32,
    pub deps: Vec<NodeId>,
    pub state: ThreeState,
    pub cleanup: Option<Box<dyn FnOnce()>>,
}

impl Node {
    pub(crate) fn observers(&self) -> &[NodeId] {
        match self {
            Node::Signal(n) => &n.observers,
            Node::Computed(n) => &n.observers,
            Node::Effect(_) => &[],
        }
    }

    pub(crate) fn observers_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        match self {
            Node::Signal(n) => Some(&mut n.observers),
            Node::Computed(n) => Some(&mut n.observers),
            Node::Effect(_) => None,
        }
    }

    pub(crate) fn deps_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        match self {
            Node::Signal(_) => None,
            Node::Computed(n) => Some(&mut n.deps),
            Node::Effect(n) => Some(&mut n.deps),
        }
    }

    /// Height in the topological order (signals are 0 — sources).
    pub(crate) fn height(&self) -> u32 {
        match self {
            Node::Signal(_) => 0,
            Node::Computed(n) => n.height,
            Node::Effect(n) => n.height,
        }
    }

    /// Version at which this node's VALUE last changed (effects have none).
    pub(crate) fn version(&self) -> u64 {
        match self {
            Node::Signal(n) => n.version,
            Node::Computed(n) => n.version,
            Node::Effect(_) => 0,
        }
    }

    pub(crate) fn state(&self) -> ThreeState {
        match self {
            Node::Signal(_) => ThreeState::Clean,
            Node::Computed(n) => n.state,
            Node::Effect(n) => n.state,
        }
    }

    pub(crate) fn set_state(&mut self, state: ThreeState) {
        match self {
            Node::Signal(_) => {}
            Node::Computed(n) => n.state = state,
            Node::Effect(n) => n.state = state,
        }
    }
}
