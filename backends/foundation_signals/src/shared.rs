//! Reactive values for a native, multithreaded host.

mod graph;
mod scope;
mod value;

pub use scope::Scope;
pub use value::{SignalGetter, SignalSetter};

use graph::{Cleanup, Graph, Id};
use std::collections::BTreeSet;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Owns a reactive graph. Workers can update independent signals concurrently.
#[derive(Default)]
pub struct MultiThreadedRuntime {
    graph: Mutex<Graph>,
    available: Condvar,
}

/// Only its owner evaluates callbacks. Nested registrations can enter on the same thread.
struct EvaluationPermit<'a> {
    runtime: &'a MultiThreadedRuntime,
    nested: bool,
}

impl Drop for EvaluationPermit<'_> {
    fn drop(&mut self) {
        if !self.nested {
            let mut graph = lock(&self.runtime.graph);
            graph.owner = None;
            graph.finish_pass();
            drop(graph);
            self.runtime.available.notify_one();
        }
    }
}

impl MultiThreadedRuntime {
    /// Creates an empty native runtime.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an independent root scope in this runtime.
    #[must_use]
    pub fn scope(self: &Arc<Self>) -> Scope {
        Scope::new(self, None)
    }

    fn enter(&self) -> EvaluationPermit<'_> {
        let current = thread::current().id();
        let graph = lock(&self.graph);
        let mut graph = self
            .available
            .wait_while(graph, |graph| {
                graph.owner.is_some_and(|owner| owner != current)
            })
            .unwrap_or_else(PoisonError::into_inner);
        let nested = graph.owner.is_some();
        graph.owner = Some(current);
        EvaluationPermit {
            runtime: self,
            nested,
        }
    }

    /// Runs dirty computations in dependency order. A nested call leaves work for the outer pass.
    pub fn stabilize(&self) {
        let permit = self.enter();
        if permit.nested {
            return;
        }
        lock(&self.graph).stabilizing = true;
        let mut visited = BTreeSet::new();
        loop {
            let next = lock(&self.graph).next(&visited);
            let Some(id) = next else { break };
            visited.insert(id);
            self.evaluate(id);
        }
    }

    /// Reads values without recording dependencies in the current evaluation.
    pub fn untracked<R>(&self, read: impl FnOnce() -> R) -> R {
        let active = {
            let mut graph = lock(&self.graph);
            if graph.owner != Some(thread::current().id()) {
                drop(graph);
                return read();
            }
            graph.active.take()
        };
        let result = catch_unwind(AssertUnwindSafe(read));
        lock(&self.graph).active = active;
        result.unwrap_or_else(|panic| resume_unwind(panic))
    }

    fn evaluate(&self, id: Id) {
        let (mut callback, before, parent) = {
            let mut graph = lock(&self.graph);
            let Some(work) = graph.start(id) else { return };
            work
        };

        let result = catch_unwind(AssertUnwindSafe(|| {
            for cleanup in before {
                self.untracked(cleanup);
            }
            // A cleanup can dispose its own scope.
            if lock(&self.graph).nodes.contains_key(&id) {
                callback();
            }
        }));

        let deferred = {
            let mut graph = lock(&self.graph);
            graph.active = parent;
            graph.evaluating -= 1;
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.callback = Some(callback);
                if result.is_err() {
                    graph.mark(id);
                }
            }
            if graph.evaluating == 0 {
                std::mem::take(&mut graph.deferred)
            } else {
                Vec::new()
            }
        };
        for cleanup in deferred {
            self.untracked(cleanup);
        }
        result.unwrap_or_else(|panic| resume_unwind(panic));
    }

    fn refresh_computed(&self, id: Id) {
        let order = {
            let graph = lock(&self.graph);
            if !graph.stabilizing
                || graph.dirty.is_empty()
                || graph.owner != Some(thread::current().id())
            {
                return;
            }
            graph.refresh_order(id)
        };
        for source in order {
            if lock(&self.graph).dirty.contains(&source) {
                self.evaluate(source);
            }
        }
    }

    fn changed(&self, id: Id) {
        lock(&self.graph).changed(id);
    }

    fn track(&self, id: Id) {
        let mut graph = lock(&self.graph);
        if graph.owner == Some(thread::current().id()) {
            graph.track(id);
        }
    }
}
