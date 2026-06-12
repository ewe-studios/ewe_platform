//! WHY: Ownership and teardown (decision 003): the graph is unified, but
//! components need a scope whose drop disposes everything it created —
//! cascading through child scopes — without any global magic.
//!
//! WHAT: [`Context`] — scoped creation (`signal` / `computed` / `effect`),
//! `child()` sub-scopes, `on_cleanup`, and recursive disposal on drop.
//!
//! HOW: A context records the `NodeId`s it created plus its child contexts'
//! inner records. Drop disposes owned nodes through the runtime (which defers
//! if a stabilize is in flight) and recurses into children. Double-dispose is
//! a no-op at every level (flags + arena generation checks).

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;

use crate::callback::EventData;
use crate::computed::{ComputedGetter, ComputedStorage};
use crate::node::{ComputedNode, EffectNode, Node, SignalNode, ThreeState};
use crate::runtime::Runtime;
use crate::signal::{SignalGetter, SignalSetter, SignalStorage};

/// A logical ownership scope within the unified graph.
pub struct Context {
    runtime: Rc<Runtime>,
    inner: Rc<RefCell<ContextInner>>,
}

struct ContextInner {
    disposed: bool,
    owned: Vec<crate::NodeId>,
    children: Vec<Rc<RefCell<ContextInner>>>,
    /// Context-level cleanups (`on_cleanup` called OUTSIDE any effect); run
    /// once at disposal, before owned nodes are removed.
    cleanups: Vec<Box<dyn FnOnce()>>,
    /// Live `Context` HANDLES over this scope (clones). Disposal happens
    /// when the LAST handle drops — `Rc::strong_count` can't be used because
    /// a parent's `children` list also holds the inner `Rc` without being a
    /// handle.
    handles: usize,
}

impl Context {
    /// A root scope on `runtime`.
    #[must_use]
    pub fn new(runtime: Rc<Runtime>) -> Self {
        Self {
            runtime,
            inner: Rc::new(RefCell::new(ContextInner {
                disposed: false,
                owned: Vec::new(),
                children: Vec::new(),
                cleanups: Vec::new(),
                handles: 1,
            })),
        }
    }

    /// A child scope: disposed when IT drops, and also (recursively) when this
    /// parent drops first.
    #[must_use]
    pub fn child(&self) -> Context {
        let child = Context::new(Rc::clone(&self.runtime));
        self.inner
            .borrow_mut()
            .children
            .push(Rc::clone(&child.inner));
        child
    }

    /// The shared runtime (for `stabilize()`, callback registration, ...).
    #[must_use]
    pub fn runtime(&self) -> &Rc<Runtime> {
        &self.runtime
    }

    /// Allocate a contiguous block of `count` instance ids from the runtime's
    /// monotonic counter (G20 — `html!` gives every mounted template instance
    /// a disjoint id range; loops/conditionals allocate per iteration).
    ///
    /// # Panics
    /// Panics if the 32-bit id space is exhausted (4 billion mounted nodes).
    #[must_use]
    pub fn allocate_id_block(&self, count: u32) -> u32 {
        self.runtime.allocate_id_block(count)
    }

    // ─── Creation ──────────────────────────────────────────────────────────────

    /// Create a signal; returns the `(getter, setter)` pair (decision 029).
    ///
    /// The setter's interop `callback_id` is assigned NOW (G17) and — for
    /// event-friendly `T` (`String`, `bool`, numerics) — a default callback
    /// that converts [`EventData`] into `T` is registered immediately. Other
    /// types get their callback from `Runtime::register_callback` (the `html!`
    /// macro's two-way-binding codegen does this).
    pub fn signal<T: Any + Clone + PartialEq + 'static>(
        &self,
        initial: T,
    ) -> (SignalGetter<T>, SignalSetter<T>) {
        let callback_id = self.runtime.next_callback_id();
        let node_id = self.runtime.graph().nodes.insert(Node::Signal(SignalNode {
            observers: Vec::new(),
            version: 0,
            callback_id,
        }));
        self.own(node_id);

        let storage = Rc::new(SignalStorage {
            runtime: Rc::clone(&self.runtime),
            value: RefCell::new(initial),
        });
        let getter = SignalGetter {
            storage: Rc::clone(&storage),
            node_id,
        };
        let setter = SignalSetter {
            storage,
            node_id,
            callback_id,
        };

        // G17: register the default event dispatch now, when `T` supports it.
        if event_convertible::<T>() {
            let target = setter.clone();
            self.runtime.register_callback(callback_id, move |data| {
                if let Some(value) = event_value::<T>(&data) {
                    target.set(value);
                } else {
                    tracing::warn!(
                        callback_id = target.callback_id(),
                        "callback event did not carry a convertible value; set skipped"
                    );
                }
            });
        }

        (getter, setter)
    }

    /// Create a computed. Evaluates immediately (tracked) to seed the cache and
    /// discover dependencies; re-evaluates during `stabilize()` only when a
    /// dependency actually changed.
    pub fn computed<T: Any + Clone + PartialEq + 'static>(
        &self,
        mut f: impl FnMut() -> T + 'static,
    ) -> ComputedGetter<T> {
        let storage = Rc::new(ComputedStorage::<T> {
            runtime: Rc::clone(&self.runtime),
            cached: RefCell::new(None),
        });

        // The node's eval closure: run the user fn, compare against the typed
        // cache, store, report "changed?" to the graph.
        let eval_storage = Rc::clone(&storage);
        let eval: Rc<RefCell<dyn FnMut() -> bool>> = Rc::new(RefCell::new(move || -> bool {
            let new_value = f();
            let mut cached = eval_storage.cached.borrow_mut();
            let changed = cached.as_ref() != Some(&new_value);
            if changed {
                *cached = Some(new_value);
            }
            changed
        }));

        let node_id = self
            .runtime
            .graph()
            .nodes
            .insert(Node::Computed(ComputedNode {
                eval: eval.clone(),
                height: 1,
                deps: Vec::new(),
                dep_versions: Vec::new(),
                state: ThreeState::Clean,
                observers: Vec::new(),
                version: 0,
            }));
        self.own(node_id);

        // Seed: tracked first evaluation (populates cache, links deps, height).
        self.runtime.tracked(node_id, || {
            (eval.borrow_mut())();
        });

        ComputedGetter { storage, node_id }
    }

    /// Create an effect. Runs IMMEDIATELY (decision 008) to track dependencies
    /// and queue its initial side-effect, then re-runs on `stabilize()` when a
    /// dependency changed.
    pub fn effect(&self, f: impl FnMut() + 'static) {
        let run: Rc<RefCell<dyn FnMut()>> = Rc::new(RefCell::new(f));
        let node_id = self.runtime.graph().nodes.insert(Node::Effect(EffectNode {
            run: run.clone(),
            height: 1,
            deps: Vec::new(),
            state: ThreeState::Clean,
            cleanup: None,
        }));
        self.own(node_id);

        self.runtime.tracked(node_id, || {
            (run.borrow_mut())();
        });
    }

    /// Register a cleanup. Called during an effect's evaluation it attaches to
    /// THAT effect (runs before each re-run and at disposal; registering again
    /// replaces the previous one). Called outside, it attaches to this context
    /// and runs once at disposal.
    ///
    /// Effect closures are `'static` and can't borrow the context — inside an
    /// effect, capture a runtime clone and call [`Runtime::on_cleanup_active`].
    pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        if let Some(f) = self.runtime.on_cleanup_active(Box::new(f)) {
            // Not inside an effect evaluation — context-level disposal hook.
            self.inner.borrow_mut().cleanups.push(f);
        }
    }

    fn own(&self, id: crate::NodeId) {
        self.inner.borrow_mut().owned.push(id);
    }
}

/// Handle semantics (spec-42 feature 03): a clone is ANOTHER HANDLE to the
/// SAME scope — signals/effects created through either belong to the one
/// scope, which is disposed when the last handle drops.
impl Clone for Context {
    fn clone(&self) -> Self {
        self.inner.borrow_mut().handles += 1;
        Self {
            runtime: Rc::clone(&self.runtime),
            inner: Rc::clone(&self.inner),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let last_handle = {
            let mut inner = self.inner.borrow_mut();
            inner.handles = inner.handles.saturating_sub(1);
            inner.handles == 0
        };
        if last_handle {
            dispose_inner(&self.runtime, &self.inner);
        }
    }
}

fn dispose_inner(runtime: &Rc<Runtime>, inner: &Rc<RefCell<ContextInner>>) {
    let (owned, children, cleanups) = {
        let mut inner = inner.borrow_mut();
        if inner.disposed {
            return; // double-dispose: no-op
        }
        inner.disposed = true;
        (
            std::mem::take(&mut inner.owned),
            std::mem::take(&mut inner.children),
            std::mem::take(&mut inner.cleanups),
        )
    };
    for cleanup in cleanups {
        cleanup();
    }
    for id in owned {
        runtime.dispose_node(id); // runs effect cleanups; defers mid-stabilize
    }
    for child in children {
        dispose_inner(runtime, &child);
    }
}

// ─── Default event -> value conversion (G17) ───────────────────────────────────

macro_rules! for_each_numeric {
    ($macro:ident) => {
        $macro!(i8);
        $macro!(i16);
        $macro!(i32);
        $macro!(i64);
        $macro!(isize);
        $macro!(u8);
        $macro!(u16);
        $macro!(u32);
        $macro!(u64);
        $macro!(usize);
        $macro!(f32);
        $macro!(f64);
    };
}

/// Is `T` one of the types the default callback can extract from [`EventData`]?
fn event_convertible<T: Any>() -> bool {
    let id = TypeId::of::<T>();
    if id == TypeId::of::<String>() || id == TypeId::of::<bool>() {
        return true;
    }
    macro_rules! check {
        ($ty:ty) => {
            if id == TypeId::of::<$ty>() {
                return true;
            }
        };
    }
    for_each_numeric!(check);
    false
}

/// Extract a `T` from event data: `String` takes `value`, `bool` prefers
/// `checked` then parses `value`, numerics parse `value`.
fn event_value<T: Any>(data: &EventData) -> Option<T> {
    let id = TypeId::of::<T>();
    let unbox = |boxed: Box<dyn Any>| boxed.downcast::<T>().ok().map(|b| *b);

    if id == TypeId::of::<String>() {
        return unbox(Box::new(data.value.clone()?));
    }
    if id == TypeId::of::<bool>() {
        let value = match data.checked {
            Some(checked) => checked,
            None => data.value.as_deref()?.parse::<bool>().ok()?,
        };
        return unbox(Box::new(value));
    }
    macro_rules! parse {
        ($ty:ty) => {
            if id == TypeId::of::<$ty>() {
                return unbox(Box::new(data.value.as_deref()?.parse::<$ty>().ok()?));
            }
        };
    }
    for_each_numeric!(parse);
    None
}
