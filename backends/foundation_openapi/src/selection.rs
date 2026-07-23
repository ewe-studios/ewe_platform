//! Selecting the slice of a spec we actually generate.
//!
//! **WHY:** Vendor specs are enormous and we use a sliver of them — Linode's is
//! 9.3 MB across 334 paths, `DigitalOcean`'s ~3 MB covering their whole platform,
//! and a provider crate needs about six endpoints. Generating the rest buries the
//! crate in types nothing calls (`foundation_deployment_cloudflare` carries
//! thousands with no consumer). Cargo features gate *compilation*, not
//! *generation*, so the dead code is still emitted, committed and reviewed.
//!
//! **WHAT:** A [`Selection`] — an allowlist of paths, tags and operation ids. An
//! empty selection means "everything", so existing providers generate unchanged.
//!
//! **HOW:** Matching is done against the spec's own operations (path, `tags`,
//! `operationId`), before endpoints are extracted or schemas resolved, so an
//! unselected operation costs nothing downstream.

use std::collections::BTreeSet;

use crate::spec::{OpenApiSpec, Operation};

/// The slice of a spec to generate.
///
/// Empty means **everything** — the whole spec, exactly as before this existed.
/// Any non-empty list narrows it: an operation is selected when it matches at
/// least one entry of *any* populated list (the lists are `ORed`, not `ANDed` —
/// `paths` and `tags` are two ways of naming the same thing, not a filter chain).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    paths: Vec<String>,
    tags: Vec<String>,
    operations: Vec<String>,
}

impl Selection {
    /// Select everything (the default).
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// The path patterns this selection carries.
    ///
    /// Readers exist so a declaration can be *rendered back out* — a generated
    /// `build.rs` has to reproduce the selection it was generated with, or the
    /// build script and the CLI would disagree about what the crate contains.
    #[must_use]
    pub fn selected_path_patterns(&self) -> &[String] {
        &self.paths
    }

    /// The tags this selection carries.
    #[must_use]
    pub fn selected_tags(&self) -> &[String] {
        &self.tags
    }

    /// The `operationId`s this selection carries.
    #[must_use]
    pub fn selected_operations(&self) -> &[String] {
        &self.operations
    }

    /// Include operations whose path matches any of `patterns`.
    ///
    /// Patterns are exact, or globs with `*` (one path segment) and `**` (any
    /// number). `/servers/{id}` matches literally; `/servers/*` matches
    /// `/servers/{id}` but not `/servers/{id}/actions`; `/servers/**` matches both.
    #[must_use]
    pub fn paths<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.paths.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Include operations carrying any of these `tags` (exact, case-sensitive —
    /// they are the vendor's own strings, e.g. Linode's `"Linode instances"`).
    #[must_use]
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Include operations with any of these `operationId`s (exact) — the name the
    /// vendor's own documentation uses, e.g. Linode's `post-linode-instance`.
    #[must_use]
    pub fn operations<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.operations.extend(ids.into_iter().map(Into::into));
        self
    }

    /// `true` when nothing was asked for — meaning "generate everything".
    #[must_use]
    pub fn is_all(&self) -> bool {
        self.paths.is_empty() && self.tags.is_empty() && self.operations.is_empty()
    }

    /// Does this selection include `operation`, found at `path`?
    #[must_use]
    pub fn includes(&self, path: &str, operation: &Operation) -> bool {
        self.matches(
            path,
            operation.tags.as_deref(),
            operation.operation_id.as_deref(),
        )
    }

    /// [`Self::includes`] against a raw JSON operation.
    ///
    /// The pruner works on `serde_json::Value` rather than the typed spec: refs
    /// point into `components/{schemas,responses,parameters,headers,…}` and only
    /// `schemas` is modelled, so walking the JSON is both simpler and complete.
    #[must_use]
    pub fn includes_value(&self, path: &str, operation: &serde_json::Value) -> bool {
        if self.is_all() {
            return true;
        }
        let tags: Option<Vec<String>> = operation.get("tags").and_then(|t| t.as_array()).map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        });
        let op_id = operation.get("operationId").and_then(|v| v.as_str());
        self.matches(path, tags.as_deref(), op_id)
    }

    /// The one place the rule lives: a path glob, a tag, or an operation id —
    /// `ORed`, because they are three ways of naming the same thing.
    fn matches(&self, path: &str, tags: Option<&[String]>, operation_id: Option<&str>) -> bool {
        if self.is_all() {
            return true;
        }

        if self.paths.iter().any(|p| glob_match(p, path)) {
            return true;
        }

        if let Some(op_tags) = tags {
            if op_tags.iter().any(|t| self.tags.contains(t)) {
                return true;
            }
        }

        if let Some(id) = operation_id {
            if self.operations.iter().any(|o| o == id) {
                return true;
            }
        }

        false
    }

    /// The paths of `spec` that contain at least one selected operation.
    ///
    /// A path with *some* operations selected stays — the unselected ones are
    /// dropped individually by [`Self::includes`] downstream. A path with none is
    /// gone entirely, which is where the bulk of the saving comes from: for a
    /// bundled spec (Linode, Hetzner) an operation's schemas are inlined in it, so
    /// dropping the path drops its schemas with it.
    #[must_use]
    pub fn selected_paths(&self, spec: &OpenApiSpec) -> BTreeSet<String> {
        let mut kept = BTreeSet::new();
        for (path, item) in &spec.paths {
            let any = operations_of(item)
                .into_iter()
                .any(|(_, op)| self.includes(path, op));
            if any {
                kept.insert(path.clone());
            }
        }
        kept
    }
}

/// Every `(method, operation)` a path item defines.
#[must_use]
pub fn operations_of(item: &crate::spec::PathItem) -> Vec<(&'static str, &Operation)> {
    [
        ("GET", &item.get),
        ("POST", &item.post),
        ("PUT", &item.put),
        ("PATCH", &item.patch),
        ("DELETE", &item.delete),
        ("OPTIONS", &item.options),
        ("HEAD", &item.head),
    ]
    .into_iter()
    .filter_map(|(method, op)| op.as_ref().map(|o| (method, o)))
    .collect()
}

/// Glob match for API paths, segment-aware.
///
/// - `*` matches within **one** segment (`/servers/*` → `/servers/{id}`, but not
///   `/servers/{id}/actions`)
/// - `**` matches **any number** of segments (`/servers/**` → both)
/// - anything else is literal, including `{param}` braces
///
/// Segment-aware rather than a plain substring match because API paths are a
/// hierarchy: `/servers/*` meaning "servers and everything under it" would make
/// it impossible to ask for just the collection.
#[must_use]
pub fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }

    let p: Vec<&str> = pattern.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    match_segments(&p, &s)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.first(), path.first()) {
        (None, None) => true,
        // `**` at the end swallows whatever is left, including nothing.
        (Some(&"**"), _) if pattern.len() == 1 => true,
        (Some(&"**"), _) => {
            // Try consuming 0..n segments with `**`, then match the rest.
            for skip in 0..=path.len() {
                if match_segments(&pattern[1..], &path[skip..]) {
                    return true;
                }
            }
            false
        }
        (Some(&"*"), Some(_)) => match_segments(&pattern[1..], &path[1..]),
        (Some(a), Some(b)) if a == b => match_segments(&pattern[1..], &path[1..]),
        _ => false,
    }
}
