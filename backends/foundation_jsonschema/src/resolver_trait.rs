//! Pluggable external schema resolution.
//!
//! WHY: JSON Schema allows `$ref` to point to external URIs. Rather than
//! building in HTTP/file resolution, we let the caller provide their own
//! strategy. This keeps the crate dependency-free and works in `no_std`.
//!
//! WHAT: `JsonResolver` trait with `NoopResolver` (default, always fails)
//! and `MapResolver` (pre-loaded URI-to-schema map).
//!
//! HOW: Resolver is passed to `ValidationOptions::with_resolver()`. The
//! registry calls `resolve()` during schema compilation for URIs not
//! already indexed. Each URI is resolved at most once — results are cached.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;

use foundation_errstacks::{ErrorTrace, IntoErrorTrace};

use derive_more::{Display, Error};
use serde_json::Value;

/// Context type for resolution failures.
///
/// WHY: Using `foundation_errstacks::ErrorTrace` lets callers attach
/// additional context (e.g., which registry triggered the resolution,
/// what the base URI was) as the error bubbles up through the compile stack.
///
/// WHAT: A minimal error type carrying only the URI that failed.
///
/// HOW: Wrapped in `ErrorTrace`; attachments added by the registry.
#[derive(Debug, Display, Error)]
#[display("failed to resolve external reference: {uri}")]
pub struct ResolveError {
    /// The URI that could not be resolved.
    pub uri: String,
}

impl ResolveError {
    /// Create a new `ResolveError` for the given URI.
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

/// Pluggable external schema resolution.
///
/// WHY: JSON Schema allows `$ref` to point to external URIs. Rather than
/// building in HTTP/file resolution, we let the caller provide their own
/// strategy — a local cache, a bundled map, an HTTP client, or a no-op
/// that rejects all external references.
///
/// WHAT: Given a URI string, return the resolved JSON document or an error.
///
/// HOW: Implement this trait and pass it to `ValidationOptions::with_resolver()`.
/// The registry calls `resolve()` during schema compilation for any URI not
/// already present in the registry. Resolution happens exactly once per URI —
/// results are cached in the registry.
///
/// # Examples
///
/// ```ignore
/// use foundation_jsonschema::{JsonResolver, ResolveError};
/// use foundation_errstacks::{ErrorTrace, IntoErrorTrace};
/// use serde_json::json;
///
/// struct MyResolver;
///
/// impl JsonResolver for MyResolver {
///     fn resolve(&self, uri: &str) -> Result<Value, ErrorTrace<ResolveError>> {
///         if uri == "https://example.com/person.json" {
///             Ok(json!({"type": "object"}))
///         } else {
///             Err(ResolveError::new(uri).into_error_trace())
///         }
///     }
/// }
/// ```
pub trait JsonResolver {
    /// Resolve a URI to a JSON Schema document.
    ///
    /// # Errors
    ///
    /// Returns `ErrorTrace<ResolveError>` if the URI cannot be resolved.
    fn resolve(&self, uri: &str) -> Result<Value, ErrorTrace<ResolveError>>;
}

/// A resolver that always fails — for schemas with no external references.
///
/// WHY: When no external references are needed, the default resolver should
/// clearly indicate that external resolution is not configured rather than
/// silently succeeding or requiring the user to provide a resolver.
///
/// WHAT: Always returns an error from `resolve()`.
///
/// HOW: Returns `ResolveError` wrapped in `ErrorTrace` with a message
/// indicating no resolver was configured.
#[derive(Debug, Clone, Copy)]
pub struct NoopResolver;

impl JsonResolver for NoopResolver {
    fn resolve(&self, uri: &str) -> Result<Value, ErrorTrace<ResolveError>> {
        Err(ResolveError::new(uri)
            .into_error_trace()
            .attach(format!("no resolver configured for: {uri}")))
    }
}

/// A resolver backed by a pre-loaded map of URI → schema document.
///
/// WHY: For cases where external schemas are known at compile time or loaded
/// from bundled resources, a map provides O(log n) lookup without any
/// network or I/O dependencies.
///
/// WHAT: A `BTreeMap<String, Value>` that maps URIs to JSON Schema documents.
///
/// HOW: `insert()` populates the map. `resolve()` does a lookup. Uses
/// `BTreeMap` instead of `HashMap` for `no_std` compatibility.
#[derive(Debug, Clone)]
pub struct MapResolver {
    schemas: BTreeMap<String, Value>,
}

impl MapResolver {
    /// Create an empty `MapResolver`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schemas: BTreeMap::new(),
        }
    }

    /// Insert a URI-to-schema mapping.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn insert(&mut self, uri: impl Into<String>, schema: Value) -> &mut Self {
        self.schemas.insert(uri.into(), schema);
        self
    }

    /// Build a `MapResolver` from an iterator of (URI, schema) pairs.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn from_pairs(iter: impl IntoIterator<Item = (String, Value)>) -> Self {
        Self {
            schemas: BTreeMap::from_iter(iter),
        }
    }
}

impl Default for MapResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonResolver for MapResolver {
    fn resolve(&self, uri: &str) -> Result<Value, ErrorTrace<ResolveError>> {
        self.schemas
            .get(uri)
            .cloned()
            .ok_or_else(|| {
                ResolveError::new(uri)
                    .into_error_trace()
                    .attach(format!("not found in map resolver: {uri}"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_noop_resolver_always_fails() {
        let resolver = NoopResolver;
        let result = resolver.resolve("https://example.com/schema.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_map_resolver_found() {
        let mut resolver = MapResolver::new();
        resolver.insert("https://example.com/a.json", json!({"type": "string"}));
        let result = resolver.resolve("https://example.com/a.json");
        assert_eq!(result.unwrap(), json!({"type": "string"}));
    }

    #[test]
    fn test_map_resolver_not_found() {
        let resolver = MapResolver::new();
        let result = resolver.resolve("https://example.com/missing.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_map_resolver_from_pairs() {
        let resolver = MapResolver::from_pairs(vec![
            ("https://a.json".into(), json!({"type": "string"})),
            ("https://b.json".into(), json!({"type": "number"})),
        ]);
        assert_eq!(
            resolver.resolve("https://a.json").unwrap(),
            json!({"type": "string"})
        );
        assert_eq!(
            resolver.resolve("https://b.json").unwrap(),
            json!({"type": "number"})
        );
        assert!(resolver.resolve("https://c.json").is_err());
    }

    #[test]
    fn test_resolve_error_display() {
        let err = ResolveError::new("https://example.com/test.json");
        let msg = format!("{err}");
        assert!(msg.contains("https://example.com/test.json"));
    }
}
