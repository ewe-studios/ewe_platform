//! Graph metadata. The runtime releases the graph lock before it runs user code.

use super::ThreadId;
use crate::height_queue::HeightQueue;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Id = usize;
pub(super) type Cleanup = Box<dyn FnOnce() + Send>;
pub(super) type Evaluate = Box<dyn FnMut() + Send>;
type Work = (Evaluate, Vec<Cleanup>, Option<Id>);

#[derive(Default)]
pub(super) struct Node {
    pub callback: Option<Evaluate>,
    pub cleanup: Vec<Cleanup>,
    dependencies: BTreeSet<Id>,
    observers: BTreeSet<Id>,
    height: usize,
    queued: bool,
}

#[derive(Default)]
pub(super) struct Owned {
    pub parent: Option<Id>,
    pub children: BTreeSet<Id>,
    pub nodes: Vec<Id>,
    pub cleanups: Vec<Cleanup>,
}

#[derive(Default)]
pub(super) struct Graph {
    serial: Id,
    pub nodes: BTreeMap<Id, Node>,
    pub scopes: BTreeMap<Id, Owned>,
    pub dirty: BTreeSet<Id>,
    queue: HeightQueue<Id>,
    next_pass: Vec<Id>,
    pub owner: Option<ThreadId>,
    pub stabilizing: bool,
    pub active: Option<Id>,
    pub evaluating: usize,
    pub deferred: Vec<Cleanup>,
}

impl Graph {
    pub fn next_id(&mut self) -> Id {
        self.serial = self
            .serial
            .checked_add(1)
            .expect("Signal identifiers exhausted");
        self.serial
    }

    pub fn insert(&mut self, scope: Id) -> Id {
        let id = self.next_id();
        self.scopes
            .get_mut(&scope)
            .expect("Signal scope disposed")
            .nodes
            .push(id);
        self.nodes.insert(id, Node::default());
        id
    }

    pub fn next(&mut self, visited: &BTreeSet<Id>) -> Option<Id> {
        while let Some((height, id)) = self.queue.pop_first() {
            let Some(node) = self.nodes.get_mut(&id) else {
                continue;
            };
            if !self.dirty.contains(&id) {
                node.queued = false;
            } else if visited.contains(&id) {
                self.next_pass.push(id);
            } else if node.height == height {
                node.queued = false;
                return Some(id);
            } else {
                self.queue.push(node.height, id);
            }
        }
        None
    }

    pub fn finish_pass(&mut self) {
        for id in self.next_pass.drain(..) {
            if let Some(node) = self.nodes.get(&id) {
                self.queue.push(node.height, id);
            }
        }
        self.stabilizing = false;
    }

    pub fn mark(&mut self, id: Id) {
        if let Some(node) = self.nodes.get_mut(&id) {
            self.dirty.insert(id);
            if !node.queued {
                node.queued = true;
                self.queue.push(node.height, id);
            }
        }
    }

    // A new dependency can point above the current scheduler height. Prepare its known inputs first.
    pub fn refresh_order(&self, source: Id) -> Vec<Id> {
        let mut order = Vec::new();
        let mut visited = BTreeSet::new();
        let mut stack = vec![(source, false)];
        while let Some((id, expanded)) = stack.pop() {
            if expanded {
                order.push(id);
            } else if visited.insert(id) {
                if let Some(node) = self.nodes.get(&id) {
                    if node.callback.is_some() {
                        stack.push((id, true));
                        stack.extend(node.dependencies.iter().map(|dep| (*dep, false)));
                    }
                }
            }
        }
        order
    }

    pub fn changed(&mut self, id: Id) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        let observers: Vec<_> = node.observers.iter().copied().collect();
        for id in observers {
            self.mark(id);
        }
    }

    pub fn track(&mut self, source: Id) {
        let Some(active) = self.active else { return };
        if source == active || !self.nodes.contains_key(&active) {
            return;
        }
        let Some(node) = self.nodes.get_mut(&source) else {
            return;
        };
        node.observers.insert(active);
        let height = node.height + 1;
        let target = self.nodes.get_mut(&active).expect("Active node exists");
        target.dependencies.insert(source);
        target.height = target.height.max(height);
    }

    pub fn start(&mut self, id: Id) -> Option<Work> {
        let node = self.nodes.get_mut(&id)?;
        let callback = node.callback.take()?;
        let cleanup = std::mem::take(&mut node.cleanup);
        node.height = 1;
        for dep in std::mem::take(&mut node.dependencies) {
            if let Some(source) = self.nodes.get_mut(&dep) {
                source.observers.remove(&id);
            }
        }
        self.dirty.remove(&id);
        self.evaluating += 1;
        Some((callback, cleanup, self.active.replace(id)))
    }

    pub fn remove(&mut self, id: Id) -> Option<Node> {
        let node = self.nodes.remove(&id)?;
        self.dirty.remove(&id);
        for dep in &node.dependencies {
            if let Some(source) = self.nodes.get_mut(dep) {
                source.observers.remove(&id);
            }
        }
        for observer in &node.observers {
            if let Some(target) = self.nodes.get_mut(observer) {
                target.dependencies.remove(&id);
            }
        }
        Some(node)
    }
}
