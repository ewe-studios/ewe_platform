//! Scope ownership and reactive node registration.

use super::graph::Owned;
use super::value::Storage;
use super::{lock, Cleanup, Id, MultiThreadedRuntime, SignalGetter, SignalSetter};
use std::sync::{Arc, PoisonError, RwLock};
use std::thread;

struct Owner {
    runtime: Arc<MultiThreadedRuntime>,
    id: Id,
}

impl Owner {
    fn dispose(&self) {
        let mut retired = Vec::new();
        let mut cleanups = Vec::new();
        {
            let mut graph = lock(&self.runtime.graph);
            let mut scopes = vec![self.id];
            while let Some(id) = scopes.pop() {
                let Some(mut scope) = graph.scopes.remove(&id) else {
                    continue;
                };
                if let Some(parent) = scope
                    .parent
                    .and_then(|parent| graph.scopes.get_mut(&parent))
                {
                    parent.children.remove(&id);
                }
                scopes.extend(scope.children);
                cleanups.append(&mut scope.cleanups);
                for node_id in scope.nodes {
                    if let Some(mut node) = graph.remove(node_id) {
                        cleanups.append(&mut node.cleanup);
                        retired.push(node);
                    }
                }
            }
            if graph.evaluating > 0 {
                graph.deferred.append(&mut cleanups);
            }
        }
        drop(retired);
        for cleanup in cleanups {
            cleanup();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Owns shared reactive nodes. Clones retain the same scope.
#[derive(Clone)]
pub struct Scope(Arc<Owner>);

impl Scope {
    pub(super) fn new(runtime: &Arc<MultiThreadedRuntime>, parent: Option<Id>) -> Self {
        let mut graph = lock(&runtime.graph);
        let id = graph.next_id();
        if let Some(parent_id) = parent {
            graph
                .scopes
                .get_mut(&parent_id)
                .expect("Parent scope disposed")
                .children
                .insert(id);
        }
        graph.scopes.insert(
            id,
            Owned {
                parent,
                ..Owned::default()
            },
        );
        Self(Arc::new(Owner {
            runtime: runtime.clone(),
            id,
        }))
    }

    /// Creates a scope whose lifetime also depends on this scope.
    #[must_use]
    pub fn child(&self) -> Self {
        Self::new(&self.0.runtime, Some(self.0.id))
    }

    /// Disposes this scope and all its children. Repeated calls are harmless.
    pub fn dispose(&self) {
        self.0.dispose();
    }

    /// Registers a cleanup for this scope or the currently evaluating node.
    pub fn on_cleanup(&self, cleanup: impl FnOnce() + Send + 'static) {
        let cleanup: Cleanup = Box::new(cleanup);
        let mut graph = lock(&self.0.runtime.graph);
        if let Some(id) = graph
            .active
            .filter(|_| graph.owner == Some(thread::current().id()))
        {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.cleanup.push(cleanup);
            } else {
                graph.deferred.push(cleanup);
            }
            return;
        }
        if let Some(scope) = graph.scopes.get_mut(&self.0.id) {
            scope.cleanups.push(cleanup);
            return;
        }
        drop(graph);
        cleanup();
    }

    /// Creates a shared signal with an initial value.
    pub fn signal<T: Clone + PartialEq + Send + Sync + 'static>(
        &self,
        value: T,
    ) -> (SignalGetter<T>, SignalSetter<T>) {
        let id = lock(&self.0.runtime.graph).insert(self.0.id);
        let storage = Arc::new(Storage {
            runtime: self.0.runtime.clone(),
            id,
            computed: false,
            value: RwLock::new(Some(value)),
        });
        (SignalGetter(storage.clone()), SignalSetter(storage))
    }

    /// Creates and evaluates a computation. Later passes update the cached result.
    ///
    /// # Panics
    /// Panics if the caller disposes this scope during registration.
    pub fn computed<T: Clone + PartialEq + Send + Sync + 'static>(
        &self,
        mut compute: impl FnMut() -> T + Send + 'static,
    ) -> SignalGetter<T> {
        let runtime = &self.0.runtime;
        let _permit = runtime.enter();
        let mut graph = lock(&runtime.graph);
        let id = graph.insert(self.0.id);
        let storage = Arc::new(Storage {
            runtime: runtime.clone(),
            id,
            computed: true,
            value: RwLock::new(None),
        });
        let setter = SignalSetter(storage.clone());
        graph
            .nodes
            .get_mut(&id)
            .expect("Registered computation")
            .callback = Some(Box::new(move || setter.set(compute())));
        drop(graph);
        runtime.evaluate(id);
        assert!(
            storage
                .value
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some(),
            "Scope disposed before computation initialization"
        );
        SignalGetter(storage)
    }

    /// Runs an effect now and again after its observed values change.
    ///
    /// # Panics
    /// Panics if the scope was disposed before registration.
    pub fn effect(&self, effect: impl FnMut() + Send + 'static) {
        let runtime = &self.0.runtime;
        let _permit = runtime.enter();
        let id = {
            let mut graph = lock(&runtime.graph);
            let id = graph.insert(self.0.id);
            graph
                .nodes
                .get_mut(&id)
                .expect("Registered effect")
                .callback = Some(Box::new(effect));
            id
        };
        runtime.evaluate(id);
    }
}
