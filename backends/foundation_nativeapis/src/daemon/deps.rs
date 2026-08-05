//! Dependency graph: startup ordering, shutdown ordering, and id resolution.
//!
//! WHY: Daemons declare `depends = [...]`; a dependent must not start until its
//! dependencies are ready, and shutdown must run in reverse so nothing loses a
//! dependency it is still using.
//!
//! WHAT: [`startup_order`] (topologically-sorted concurrency levels),
//! [`shutdown_order`] (those levels reversed), and [`resolve_id`] (turning a
//! dependency reference into a fully-qualified [`DaemonId`]).
//!
//! HOW: Kahn's algorithm produces levels of daemons whose dependencies are all
//! already satisfied; each level may start concurrently. A leftover non-zero
//! in-degree means a cycle, reported with its members.

use std::collections::{BTreeMap, VecDeque};

use super::config::DaemonDef;
use super::error::CycleError;
use super::id::{DaemonId, DEFAULT_NAMESPACE};

/// Compute startup order as concurrency levels via topological sort.
///
/// WHY: Daemons in the same level share no ordering constraint and can start in
/// parallel; later levels wait for earlier ones to be ready.
///
/// WHAT: A `Vec` of levels; each level is the set of daemons whose dependencies
/// are all in earlier levels.
///
/// HOW: Kahn's algorithm over the dependency edges. In-degree counts each
/// daemon's satisfied-dependency prerequisites; the queue drains one full level
/// at a time. If any daemon never reaches in-degree 0, the remaining members
/// form a cycle.
///
/// Dependencies that reference an unknown name are treated as external and
/// ignored for ordering (the supervisor validates existence separately) — a
/// name with no defining daemon contributes no edge, so it cannot deadlock the
/// sort.
///
/// # Errors
/// Returns [`CycleError`] listing the daemons involved in a dependency cycle.
///
/// # Panics
/// Never panics.
pub fn startup_order(daemons: &[DaemonDef]) -> Result<Vec<Vec<DaemonId>>, CycleError> {
    // Only edges between daemons we actually define constrain ordering.
    let defined: BTreeMap<&str, ()> = daemons.iter().map(|d| (d.name.as_str(), ())).collect();

    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for def in daemons {
        in_degree.entry(def.name.clone()).or_insert(0);
        for dep in &def.depends {
            // Strip any namespace qualifier for local ordering; cross-namespace
            // refs are resolved elsewhere and do not participate in this sort.
            let dep_name = dep.rsplit('/').next().unwrap_or(dep.as_str());
            if !defined.contains_key(dep_name) {
                continue;
            }
            dependents
                .entry(dep_name.to_string())
                .or_default()
                .push(def.name.clone());
            *in_degree.entry(def.name.clone()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut levels: Vec<Vec<DaemonId>> = Vec::new();
    let mut processed = 0usize;

    while !queue.is_empty() {
        let level: Vec<String> = queue.drain(..).collect();
        processed += level.len();
        for name in &level {
            if let Some(deps) = dependents.remove(name) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(&dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }
        levels.push(
            level
                .into_iter()
                .map(|name| DaemonId::new(DEFAULT_NAMESPACE, name))
                .collect(),
        );
    }

    if processed < daemons.len() {
        let cycle: Vec<DaemonId> = in_degree
            .into_iter()
            .filter(|(_, deg)| *deg > 0)
            .map(|(name, _)| DaemonId::new(DEFAULT_NAMESPACE, name))
            .collect();
        Err(CycleError { cycle })
    } else {
        Ok(levels)
    }
}

/// Reverse startup levels to get shutdown order (dependents stop first).
///
/// WHY: A daemon must not be torn down while a dependent still relies on it.
///
/// WHAT: The startup levels reversed.
///
/// HOW: `levels.iter().rev().cloned()`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn shutdown_order(levels: &[Vec<DaemonId>]) -> Vec<Vec<DaemonId>> {
    levels.iter().rev().cloned().collect()
}

/// Resolve a dependency reference into a fully-qualified [`DaemonId`].
///
/// WHY: A `depends` entry may be a bare name (same namespace) or a qualified
/// `namespace/name` cross-group reference.
///
/// WHAT: The referenced daemon's id, inheriting `self_id`'s namespace when the
/// reference is unqualified.
///
/// HOW: Splits on the first `/`; if present, the left side is the namespace.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn resolve_id(self_id: &DaemonId, ref_name: &str) -> DaemonId {
    if let Some((ns, name)) = ref_name.split_once('/') {
        DaemonId::new(ns, name)
    } else {
        DaemonId::new(self_id.namespace.clone(), ref_name)
    }
}
