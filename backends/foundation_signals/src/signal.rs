//! WHY: Components want function-style state (decision 029): `let (count,
//! set_count) = ctx.signal(0)` — a read handle that auto-subscribes inside
//! effects and a write handle that marks the graph dirty, both cheap clones.
//!
//! WHAT: [`SignalGetter`], [`SignalSetter`], and the shared typed
//! [`SignalStorage`] both handles point at.
//!
//! HOW: The VALUE lives here, typed, in `RefCell<T>` (G15 single-threaded) —
//! the graph node only carries observers/version/callback-id. `get()` reports
//! the read to the runtime (dependency tracking) and clones the value; `set()`
//! compares with `PartialEq`, writes, and asks the runtime to mark observers —
//! propagation always waits for `stabilize()`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::arena::NodeId;
use crate::runtime::Runtime;

pub(crate) struct SignalStorage<T> {
    pub(crate) runtime: Rc<Runtime>,
    pub(crate) value: RefCell<T>,
}

/// Read handle. Cloneable; reading inside an effect/computed subscribes the
/// reader as a dependency.
pub struct SignalGetter<T> {
    pub(crate) storage: Rc<SignalStorage<T>>,
    pub(crate) node_id: NodeId,
}

impl<T> Clone for SignalGetter<T> {
    fn clone(&self) -> Self {
        Self {
            storage: Rc::clone(&self.storage),
            node_id: self.node_id,
        }
    }
}

impl<T: Clone + 'static> SignalGetter<T> {
    /// Current value. Inside a tracked evaluation this records the dependency.
    #[must_use]
    pub fn get(&self) -> T {
        self.storage.runtime.track_read(self.node_id);
        self.storage.value.borrow().clone()
    }

    /// Read without subscribing — for event handlers and other untracked code
    /// that wants the value but not the dependency.
    #[must_use]
    pub fn get_untracked(&self) -> T {
        self.storage.value.borrow().clone()
    }

    /// The graph id (useful for assertions/debugging; identity is the handle).
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}

/// Write handle. Carries the interop `callback_id` (G17) the `html!` macro
/// reads for two-way binding.
pub struct SignalSetter<T> {
    pub(crate) storage: Rc<SignalStorage<T>>,
    pub(crate) node_id: NodeId,
    pub(crate) callback_id: u64,
}

impl<T> Clone for SignalSetter<T> {
    fn clone(&self) -> Self {
        Self {
            storage: Rc::clone(&self.storage),
            node_id: self.node_id,
            callback_id: self.callback_id,
        }
    }
}

impl<T: Clone + PartialEq + 'static> SignalSetter<T> {
    /// Write `value`. Observers are marked dirty only if it differs
    /// (`PartialEq`); nothing propagates until `stabilize()`.
    pub fn set(&self, value: T) {
        {
            let mut current = self.storage.value.borrow_mut();
            if *current == value {
                return;
            }
            *current = value;
        }
        self.storage.runtime.signal_changed(self.node_id);
    }

    /// In-place update; dirties observers only if the result differs.
    pub fn update<F: FnOnce(&mut T)>(&self, f: F) {
        let changed = {
            let mut current = self.storage.value.borrow_mut();
            let before = current.clone();
            f(&mut current);
            *current != before
        };
        if changed {
            self.storage.runtime.signal_changed(self.node_id);
        }
    }

    /// The interop callback id assigned at creation — monotonic per runtime,
    /// never reused. The `html!` macro reads this; it does not assign it.
    #[must_use]
    pub fn callback_id(&self) -> u64 {
        self.callback_id
    }

    /// The graph id (shared with the paired getter).
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}
