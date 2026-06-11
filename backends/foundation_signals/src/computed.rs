//! WHY: Derived values that cache, only recompute when their dependencies
//! actually changed (three-state + version short-circuit), and participate in
//! the height order so diamonds evaluate once.
//!
//! WHAT: [`ComputedGetter`] and the typed [`ComputedStorage`] its graph node's
//! eval closure writes into.
//!
//! HOW: Like signals, the VALUE lives here typed; the node's `eval` closure
//! (built in `Context::computed`) runs the user compute fn, compares with
//! `PartialEq` against this cache, stores, and reports "changed?" to the graph.
//! Between stabilize calls `get()` returns the cached value.

use std::cell::RefCell;
use std::rc::Rc;

use crate::arena::NodeId;
use crate::runtime::Runtime;

pub(crate) struct ComputedStorage<T> {
    pub(crate) runtime: Rc<Runtime>,
    /// `None` only between construction and the seeding evaluation inside
    /// `Context::computed` — every public handle observes it populated.
    pub(crate) cached: RefCell<Option<T>>,
}

/// Read handle for a computed. Same subscription behaviour as a signal getter.
pub struct ComputedGetter<T> {
    pub(crate) storage: Rc<ComputedStorage<T>>,
    pub(crate) node_id: NodeId,
}

impl<T> Clone for ComputedGetter<T> {
    fn clone(&self) -> Self {
        Self {
            storage: Rc::clone(&self.storage),
            node_id: self.node_id,
        }
    }
}

impl<T: Clone + 'static> ComputedGetter<T> {
    /// Last stabilized value. Inside a tracked evaluation this records the
    /// dependency (so computeds can depend on computeds).
    ///
    /// # Panics
    /// Never in practice — the cache is seeded by the creation-time evaluation.
    #[must_use]
    pub fn get(&self) -> T {
        self.storage.runtime.track_read(self.node_id);
        self.storage
            .cached
            .borrow()
            .clone()
            .expect("computed cache seeded at creation")
    }

    /// Read the cache without subscribing.
    ///
    /// # Panics
    /// Never in practice — the cache is seeded by the creation-time evaluation.
    #[must_use]
    pub fn get_untracked(&self) -> T {
        self.storage
            .cached
            .borrow()
            .clone()
            .expect("computed cache seeded at creation")
    }

    /// The graph id (useful for assertions/debugging).
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}
