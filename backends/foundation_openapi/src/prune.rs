//! Cutting a spec down to a [`Selection`] and its reachable closure.
//!
//! **WHY:** selecting six endpoints out of Linode's 334 is only half the job —
//! the components they *don't* use have to go too, or the crate still carries
//! them. Linode alone ships 77 `components/schemas` that **nothing references**
//! (its spec has zero `$ref`s), and DigitalOcean's 7814 refs reach far beyond the
//! handful of paths a VPS crate calls.
//!
//! **WHAT:** [`prune_to_selection`] drops unselected operations, then removes
//! every component nothing kept can reach.
//!
//! **HOW:** on raw JSON, not the typed spec. DigitalOcean refs into
//! `components/responses` (3758), `headers` (1372) and `parameters` (833), and
//! only `schemas` is modelled — walking the JSON follows all of them for free.
//! Reachability is a fixed-point walk from the surviving paths, following `$ref`
//! **and** `discriminator.mapping` (Hetzner's only refs live there, and a
//! discriminator pointing at a dropped schema is a broken spec).
//!
//! Run it **after** canonicalisation, so hoisted schemas are already in place and
//! reachable from the operations that own them.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::selection::Selection;

/// What [`prune_to_selection`] removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneStats {
    /// Paths dropped entirely (no operation survived).
    pub paths_removed: usize,
    /// Operations dropped from paths that did survive.
    pub operations_removed: usize,
    /// Components dropped as unreachable.
    pub components_removed: usize,
    /// Components kept — the closure.
    pub components_kept: usize,
}

/// Reduce `spec` to `selection` and everything it can reach.
///
/// An empty selection selects everything, and then only unreachable components
/// are pruned — which is still worth doing (Linode's 77 orphans).
pub fn prune_to_selection(spec: &mut Value, selection: &Selection) -> PruneStats {
    let mut stats = PruneStats::default();

    prune_paths(spec, selection, &mut stats);
    let reachable = reachable_components(spec);
    prune_components(spec, &reachable, &mut stats);

    stats
}

/// Drop unselected operations, and any path left with none.
///
/// Paths with nothing selected go first and whole, so `operations_removed` counts
/// only what was pruned *out of a surviving path* — a path that vanished is
/// `paths_removed`, and counting its operations in both would double-report the
/// same loss.
fn prune_paths(spec: &mut Value, selection: &Selection, stats: &mut PruneStats) {
    if selection.is_all() {
        return;
    }
    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };

    // 1. Whole paths where nothing is selected.
    let doomed: Vec<String> = paths
        .iter()
        .filter(|(path, item)| {
            let Some(item) = item.as_object() else { return false };
            !item
                .iter()
                .any(|(key, op)| is_http_method(key) && selection.includes_value(path, op))
        })
        .map(|(path, _)| path.clone())
        .collect();

    for path in doomed {
        paths.remove(&path);
        stats.paths_removed += 1;
    }

    // 2. Unselected operations on the paths that survived.
    for (path, item) in paths.iter_mut() {
        let Some(item) = item.as_object_mut() else { continue };
        let drop: Vec<String> = item
            .iter()
            .filter(|(key, op)| is_http_method(key) && !selection.includes_value(path, op))
            .map(|(key, _)| key.clone())
            .collect();

        for key in drop {
            item.remove(&key);
            stats.operations_removed += 1;
        }
    }
}

/// Every `#/components/<kind>/<name>` reachable from the surviving paths.
///
/// A fixed point: a component's own body can reference others, so keep walking
/// until nothing new turns up. That is also what makes recursive schemas safe —
/// a type that references itself adds nothing on the second pass.
fn reachable_components(spec: &Value) -> BTreeSet<String> {
    let mut reachable = BTreeSet::new();

    let mut frontier: Vec<String> = Vec::new();
    if let Some(paths) = spec.get("paths") {
        collect_refs(paths, &mut frontier);
    }

    while let Some(pointer) = frontier.pop() {
        if !reachable.insert(pointer.clone()) {
            continue; // already walked — this is the recursion guard
        }
        if let Some(component) = resolve(spec, &pointer) {
            collect_refs(component, &mut frontier);
        }
    }

    reachable
}

/// Every component reference in `value`'s subtree.
///
/// Two ref sites, not one:
/// - `{"$ref": "#/components/…"}` — the obvious one;
/// - `discriminator.mapping` values — the *only* refs Hetzner's spec has (35 of
///   them). Miss these and a kept schema's discriminator points at a type that is
///   no longer there.
fn collect_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                if is_component_ref(r) {
                    out.push(r.clone());
                }
            }
            if let Some(mapping) = map
                .get("discriminator")
                .and_then(|d| d.get("mapping"))
                .and_then(Value::as_object)
            {
                for target in mapping.values() {
                    if let Some(r) = target.as_str() {
                        if is_component_ref(r) {
                            out.push(r.to_string());
                        }
                    }
                }
            }
            for v in map.values() {
                collect_refs(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_refs(v, out);
            }
        }
        _ => {}
    }
}

/// Remove every component not in `reachable`.
fn prune_components(spec: &mut Value, reachable: &BTreeSet<String>, stats: &mut PruneStats) {
    let Some(components) = spec.get_mut("components").and_then(Value::as_object_mut) else {
        return;
    };

    for (kind, entries) in components.iter_mut() {
        let Some(entries) = entries.as_object_mut() else { continue };
        let drop: Vec<String> = entries
            .keys()
            .filter(|name| !reachable.contains(&format!("#/components/{kind}/{name}")))
            .cloned()
            .collect();

        for name in drop {
            entries.remove(&name);
            stats.components_removed += 1;
        }
        stats.components_kept += entries.len();
    }
}

/// Resolve `#/a/b/c` against `spec`. Only local component pointers are followed —
/// an external `$ref` (another file, a URL) is not ours to resolve, and a spec
/// that has them is not self-contained.
fn resolve<'a>(spec: &'a Value, pointer: &str) -> Option<&'a Value> {
    let path = pointer.strip_prefix("#/")?;
    let mut current = spec;
    for segment in path.split('/') {
        // JSON-pointer escapes, in the order the spec mandates.
        let segment = segment.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(map) => map.get(&segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn is_component_ref(r: &str) -> bool {
    r.starts_with("#/components/")
}

fn is_http_method(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "options" | "head" | "trace"
    )
}

/// Every component `$ref` in `spec` that resolves to nothing.
///
/// The invariant a pruner must not break: keep too little and the output does not
/// compile. This is how the tests prove it — and it is worth running on real specs
/// even without pruning, since a vendor's spec can ship dangling refs of its own.
#[must_use]
pub fn dangling_refs(spec: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_refs(spec, &mut refs);
    refs.sort();
    refs.dedup();
    refs.retain(|r| resolve(spec, r).is_none());
    refs
}

/// Components present but reachable from nothing — dead weight.
#[must_use]
pub fn unreachable_components(spec: &Value) -> Vec<String> {
    let reachable = reachable_components(spec);
    let mut orphans = Vec::new();
    if let Some(components) = spec.get("components").and_then(Value::as_object) {
        for (kind, entries) in components {
            let Some(entries) = entries.as_object() else { continue };
            for name in entries.keys() {
                let pointer = format!("#/components/{kind}/{name}");
                if !reachable.contains(&pointer) {
                    orphans.push(pointer);
                }
            }
        }
    }
    orphans
}

/// A component map's size, for reporting.
#[must_use]
pub fn component_count(spec: &Value, kind: &str) -> usize {
    spec.pointer(&format!("/components/{kind}"))
        .and_then(Value::as_object)
        .map_or(0, Map::len)
}
