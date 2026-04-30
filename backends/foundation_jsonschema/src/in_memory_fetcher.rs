//! In-memory resolver pre-loaded with JSON Schema meta-schemas.
//!
//! WHY: Most `$ref` lookups target well-known meta-schema URLs
//! (e.g. `https://json-schema.org/draft/2020-12/schema`). Rather than
//! requiring users to supply these, we embed them at compile time
//! and resolve them from memory.
//!
//! WHAT: `InMemoryFetcher` implements `JsonResolver` with a bundled
//! set of meta-schemas for all 5 supported drafts.
//!
//! HOW: Meta-schema JSON files are embedded via `include_str!` and
//! parsed into a `BTreeMap` at construction time. No network or I/O.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};

use foundation_errstacks::{ErrorTrace, IntoErrorTrace};
use serde_json::Value;

use crate::resolver_trait::{JsonResolver, ResolveError};

/// Build the built-in meta-schema map.
fn build_builtin_map() -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    let pairs: &[(&str, &str)] = &[
        // Draft 04
        (
            "http://json-schema.org/draft-04/schema#",
            include_str!("../artefacts/draft-04-schema.json"),
        ),
        (
            "http://json-schema.org/draft-04/hyper-schema#",
            include_str!("../artefacts/draft-04-hyper-schema.json"),
        ),
        // Draft 06
        (
            "http://json-schema.org/draft-06/schema#",
            include_str!("../artefacts/draft-06-schema.json"),
        ),
        (
            "http://json-schema.org/draft-06/hyper-schema#",
            include_str!("../artefacts/draft-06-hyper-schema.json"),
        ),
        // Draft 07
        (
            "http://json-schema.org/draft-07/schema#",
            include_str!("../artefacts/draft-07-schema.json"),
        ),
        (
            "http://json-schema.org/draft-07/hyper-schema#",
            include_str!("../artefacts/draft-07-hyper-schema.json"),
        ),
        // Draft 2019-09
        (
            "https://json-schema.org/draft/2019-09/schema",
            include_str!("../artefacts/draft-2019-09-schema.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/core",
            include_str!("../artefacts/draft-2019-09-meta-core.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/applicator",
            include_str!("../artefacts/draft-2019-09-meta-applicator.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/validation",
            include_str!("../artefacts/draft-2019-09-meta-validation.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/meta-data",
            include_str!("../artefacts/draft-2019-09-meta-meta-data.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/format",
            include_str!("../artefacts/draft-2019-09-meta-format.json"),
        ),
        (
            "https://json-schema.org/draft/2019-09/meta/content",
            include_str!("../artefacts/draft-2019-09-meta-content.json"),
        ),
        // Draft 2020-12
        (
            "https://json-schema.org/draft/2020-12/schema",
            include_str!("../artefacts/draft-2020-12-schema.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/core",
            include_str!("../artefacts/draft-2020-12-meta-core.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/applicator",
            include_str!("../artefacts/draft-2020-12-meta-applicator.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/validation",
            include_str!("../artefacts/draft-2020-12-meta-validation.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/meta-data",
            include_str!("../artefacts/draft-2020-12-meta-meta-data.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/format-annotation",
            include_str!("../artefacts/draft-2020-12-meta-format-annotation.json"),
        ),
        (
            "https://json-schema.org/draft/2020-12/meta/content",
            include_str!("../artefacts/draft-2020-12-meta-content.json"),
        ),
    ];
    for (uri, json_str) in pairs {
        if let Ok(val) = serde_json::from_str::<Value>(json_str) {
            map.insert((*uri).to_string(), val);
        }
    }
    map
}

/// An in-memory resolver pre-loaded with JSON Schema meta-schemas.
///
/// WHY: Most `$ref` URIs reference well-known meta-schemas. This resolver
/// resolves them from memory without any network or file I/O.
///
/// WHAT: A `BTreeMap<String, Value>` containing 20 meta-schema documents
/// for all 5 supported drafts (4, 6, 7, 2019-09, 2020-12).
///
/// HOW: Embedded at compile time via `include_str!`, parsed once at
/// construction, then used for O(log n) lookups.
///
/// # Cloning
///
/// The internal map is `Clone`-able so users can extend it with custom
/// schemas:
///
/// ```ignore
/// let mut resolver = InMemoryFetcher::builtin();
/// resolver.insert("https://example.com/myschema", json!({"type": "string"}));
/// ```
#[derive(Clone)]
pub struct InMemoryFetcher {
    schemas: BTreeMap<String, Value>,
}

impl InMemoryFetcher {
    /// Create a resolver pre-loaded with all built-in meta-schemas.
    ///
    /// Includes schemas for Draft 4, 6, 7, 2019-09, and 2020-12
    /// plus their meta files (applicator, validation, core, etc.).
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            schemas: build_builtin_map(),
        }
    }

    /// Create an empty resolver with no embedded schemas.
    ///
    /// Use this if you want to build a custom map from scratch.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schemas: BTreeMap::new(),
        }
    }

    /// Insert a custom URI → schema mapping.
    pub fn insert(&mut self, uri: impl Into<String>, schema: Value) -> &mut Self {
        self.schemas.insert(uri.into(), schema);
        self
    }

    /// Get a cloned copy of the internal map.
    ///
    /// Use this to inspect or serialize the resolver's contents.
    #[must_use]
    pub fn schemas(&self) -> &BTreeMap<String, Value> {
        &self.schemas
    }

    /// Returns the number of schemas in the resolver.
    #[must_use]
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Returns `true` if the resolver has no schemas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

impl Default for InMemoryFetcher {
    fn default() -> Self {
        Self::builtin()
    }
}

impl JsonResolver for InMemoryFetcher {
    fn resolve(&self, uri: &str) -> Result<Value, ErrorTrace<ResolveError>> {
        self.schemas.get(uri).cloned().ok_or_else(|| {
            ResolveError::new(uri)
                .into_error_trace()
                .attach(format!("not found in in-memory fetcher: {uri}"))
        })
    }
}

impl core::fmt::Debug for InMemoryFetcher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InMemoryFetcher")
            .field("schema_count", &self.schemas.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_has_all_draft_schemas() {
        let fetcher = InMemoryFetcher::builtin();
        assert!(fetcher.len() >= 20);
    }

    #[test]
    fn test_resolve_draft4_schema() {
        let fetcher = InMemoryFetcher::builtin();
        let result = fetcher.resolve("http://json-schema.org/draft-04/schema#");
        assert!(result.is_ok());
        let schema = result.unwrap();
        assert!(schema.get("id").is_some() || schema.get("$id").is_some());
    }

    #[test]
    fn test_resolve_draft7_schema() {
        let fetcher = InMemoryFetcher::builtin();
        let result = fetcher.resolve("http://json-schema.org/draft-07/schema#");
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_draft202012_schema() {
        let fetcher = InMemoryFetcher::builtin();
        let result = fetcher.resolve("https://json-schema.org/draft/2020-12/schema");
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_draft201909_meta() {
        let fetcher = InMemoryFetcher::builtin();
        let result = fetcher.resolve("https://json-schema.org/draft/2019-09/meta/applicator");
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_missing_uri() {
        let fetcher = InMemoryFetcher::builtin();
        let result = fetcher.resolve("https://example.com/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_fetcher() {
        let fetcher = InMemoryFetcher::empty();
        assert!(fetcher.is_empty());
        assert_eq!(fetcher.len(), 0);
    }

    #[test]
    fn test_insert_and_resolve() {
        let mut fetcher = InMemoryFetcher::empty();
        fetcher.insert(
            "https://example.com/test",
            serde_json::json!({"type": "string"}),
        );
        let result = fetcher.resolve("https://example.com/test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!({"type": "string"}));
    }

    #[test]
    fn test_clone_resolver() {
        let fetcher = InMemoryFetcher::builtin();
        let cloned = fetcher.clone();
        assert_eq!(fetcher.len(), cloned.len());
    }
}
