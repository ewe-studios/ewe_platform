//! Reference resolver with base URI context and dynamic scope tracking.
//!
//! WHY: JSON Schema reference resolution depends on context: the current
//! base URI (set by `$id`), the dynamic scope (for `$dynamicRef`), and the
//! registry of known resources. `Resolver` carries all this context.
//!
//! WHAT: `Resolver` resolves `$ref` strings to concrete JSON values.
//! `Resolved` pairs the resolved value with a new resolver positioned at
//! the resolved location.
//!
//! HOW: `lookup()` parses the reference, splits it into URI + fragment,
//! looks up the resource in the registry, and traverses any JSON Pointer
//! fragment.

use alloc::string::String;
use alloc::vec::Vec;

use foundation_errstacks::IntoErrorTrace;
use serde_json::Value;

use crate::draft::Draft;
use crate::error::{ValidationError, ValidationErrorKind};

use super::anchor::Anchor;
use super::pointer;
use super::registry::Registry;
use super::resource::ResourceRef;

/// The result of resolving a reference: the JSON value plus a new resolver
/// positioned at the resolved location.
///
/// WHY: After resolving a `$ref`, the compiler needs both the target schema
/// and a resolver that can handle further references relative to that target.
#[derive(Debug)]
pub struct Resolved<'r> {
    contents: &'r Value,
    resolver: Resolver<'r>,
    draft: Draft,
}

impl<'r> Resolved<'r> {
    pub(crate) fn new(contents: &'r Value, resolver: Resolver<'r>, draft: Draft) -> Self {
        Self {
            contents,
            resolver,
            draft,
        }
    }

    /// The resolved schema contents.
    #[must_use]
    pub fn contents(&self) -> &'r Value {
        self.contents
    }

    /// The resolver positioned at the resolved location.
    #[must_use]
    pub fn resolver(&self) -> &Resolver<'r> {
        &self.resolver
    }

    /// The draft of the resolved schema.
    #[must_use]
    pub fn draft(&self) -> Draft {
        self.draft
    }

    /// Decompose into parts.
    #[must_use]
    pub fn into_inner(self) -> (&'r Value, Resolver<'r>, Draft) {
        (self.contents, self.resolver, self.draft)
    }
}

/// Reference resolver with base URI context and dynamic scope tracking.
///
/// WHY: `$ref` resolution is context-dependent — relative URIs resolve
/// against the current base URI, and `$dynamicRef` walks the dynamic scope.
///
/// WHAT: Holds a reference to the registry, the current base URI, and the
/// dynamic scope stack.
///
/// HOW: `lookup()` resolves a reference string by:
/// 1. Parsing the reference into URI + fragment
/// 2. Resolving the URI against the current base
/// 3. Looking up the resource in the registry
/// 4. Traversing any JSON Pointer or resolving any anchor in the fragment
#[derive(Clone)]
pub struct Resolver<'r> {
    registry: &'r Registry,
    base_uri: String,
    dynamic_scope: Vec<String>,
}

impl<'r> Resolver<'r> {
    /// Create a new resolver rooted at the given base URI.
    pub(crate) fn new(registry: &'r Registry, base_uri: String) -> Self {
        Self {
            registry,
            base_uri,
            dynamic_scope: Vec::new(),
        }
    }

    /// The current base URI.
    #[must_use]
    pub fn base_uri(&self) -> &str {
        &self.base_uri
    }

    /// The current dynamic scope (stack of base URIs).
    #[must_use]
    pub fn dynamic_scope(&self) -> &[String] {
        &self.dynamic_scope
    }

    /// Resolve a `$ref` string to a JSON value.
    ///
    /// WHY: This is the core resolution method used by the compiler for all
    /// `$ref` keywords.
    ///
    /// WHAT: Returns the resolved schema plus a new resolver positioned at
    /// the resolved location.
    ///
    /// HOW: Handles fragment-only references (`#/defs/Foo`), absolute URIs,
    /// relative URIs, anchor references (`#my-anchor`), and JSON Pointer
    /// fragments (`#/properties/name`).
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the reference cannot be resolved.
    pub fn lookup(&self, reference: &str) -> Result<Resolved<'r>, ValidationError> {
        let (target_uri, fragment) = if let Some(frag) = reference.strip_prefix('#') {
            (self.base_uri.clone(), frag.to_string())
        } else if let Some((uri_part, frag)) = reference.rsplit_once('#') {
            let resolved = self
                .registry
                .resolve_uri(&self.base_uri, uri_part)
                .map_err(|e| {
                    ValidationErrorKind::Schema {
                        reason: alloc::format!("failed to resolve URI '{}': {}", reference, e),
                    }
                    .into_error_trace()
                })?;
            (resolved, frag.to_string())
        } else {
            let resolved = self
                .registry
                .resolve_uri(&self.base_uri, reference)
                .map_err(|e| {
                    ValidationErrorKind::Schema {
                        reason: alloc::format!("failed to resolve URI '{}': {}", reference, e),
                    }
                    .into_error_trace()
                })?;
            (resolved, String::new())
        };

        let resource = self.registry.get_resource(&target_uri).ok_or_else(|| {
            ValidationErrorKind::Schema {
                reason: alloc::format!(
                    "resource '{}' not found in registry",
                    target_uri
                ),
            }
            .into_error_trace()
        })?;

        // JSON Pointer fragment
        if fragment.starts_with('/') {
            let target = pointer::resolve_pointer(resource.contents(), &fragment).map_err(|e| {
                ValidationErrorKind::Schema {
                    reason: alloc::format!("pointer resolution failed: {}", e),
                }
                .into_error_trace()
            })?;
            let resolver = self.evolve(&target_uri);
            return Ok(Resolved::new(target, resolver, resource.draft()));
        }

        // Anchor fragment
        if !fragment.is_empty() {
            if fragment.contains('/') {
                return Err(ValidationErrorKind::Schema {
                    reason: alloc::format!("anchor '{}' is invalid", fragment),
                }
                .into_error_trace());
            }

            let anchor = self.registry.get_anchor(&target_uri, &fragment).ok_or_else(|| {
                ValidationErrorKind::Schema {
                    reason: alloc::format!("anchor '{}' does not exist", fragment),
                }
                .into_error_trace()
            })?;

            let resolver = self.evolve(&target_uri);
            return match anchor {
                Anchor::Default { resource: r, .. } => {
                    Ok(Resolved::new(r.contents(), resolver, r.draft()))
                }
                Anchor::Dynamic { resource: r, name } => {
                    // Walk dynamic scope for outermost matching dynamic anchor
                    let mut last_resource = r;
                    for scope_uri in &self.dynamic_scope {
                        if let Some(Anchor::Dynamic { resource: scoped_r, .. }) =
                            self.registry.get_anchor(scope_uri, name)
                        {
                            last_resource = scoped_r;
                        }
                    }
                    Ok(Resolved::new(
                        last_resource.contents(),
                        resolver,
                        last_resource.draft(),
                    ))
                }
            };
        }

        // No fragment — return the resource itself
        let resolver = self.evolve(&target_uri);
        Ok(Resolved::new(
            resource.contents(),
            resolver,
            resource.draft(),
        ))
    }

    /// Resolve `$recursiveRef` (Draft 2019-09).
    ///
    /// WHY: `$recursiveRef` walks the dynamic scope looking for schemas with
    /// `$recursiveAnchor: true`, stopping at the outermost one.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if resolution fails.
    pub fn lookup_recursive_ref(&self) -> Result<Resolved<'r>, ValidationError> {
        let mut resolved = self.lookup("#")?;

        if let Value::Object(obj) = resolved.contents {
            if obj
                .get("$recursiveAnchor")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                for scope_uri in &self.dynamic_scope {
                    let next = self.lookup(scope_uri)?;
                    match next.contents {
                        Value::Object(next_obj) => {
                            if !next_obj
                                .get("$recursiveAnchor")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            {
                                break;
                            }
                        }
                        _ => break,
                    }
                    resolved = next;
                }
            }
        }

        Ok(resolved)
    }

    /// Create a new resolver with an updated base URI.
    ///
    /// WHY: When entering a sub-resource (via `$id`), the base URI changes.
    /// The previous base is pushed onto the dynamic scope stack.
    #[must_use]
    pub fn evolve(&self, new_base_uri: &str) -> Resolver<'r> {
        let mut dynamic_scope = self.dynamic_scope.clone();
        if !self.base_uri.is_empty()
            && (dynamic_scope.is_empty() || new_base_uri != self.base_uri)
        {
            dynamic_scope.push(self.base_uri.clone());
        }
        Resolver {
            registry: self.registry,
            base_uri: new_base_uri.to_string(),
            dynamic_scope,
        }
    }

    /// Create a resolver for a sub-resource.
    ///
    /// WHY: When the compiler encounters a `$id`, it needs a new resolver
    /// with the updated base URI for that sub-resource.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the resource ID cannot be resolved.
    pub fn in_subresource(
        &self,
        subresource: ResourceRef<'_>,
    ) -> Result<Resolver<'r>, ValidationError> {
        if let Some(id) = subresource.id() {
            let base_uri = self
                .registry
                .resolve_uri(&self.base_uri, id)
                .map_err(|e| {
                    ValidationErrorKind::Schema {
                        reason: alloc::format!("failed to resolve subresource ID '{}': {}", id, e),
                    }
                    .into_error_trace()
                })?;
            Ok(Resolver {
                registry: self.registry,
                base_uri,
                dynamic_scope: self.dynamic_scope.clone(),
            })
        } else {
            Ok(self.clone())
        }
    }
}

impl core::fmt::Debug for Resolver<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Resolver")
            .field("base_uri", &self.base_uri)
            .field("dynamic_scope", &self.dynamic_scope)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::referencing::registry::Registry;
    use serde_json::json;

    fn build_registry(pairs: &[(&str, Value)]) -> Registry {
        let mut builder = Registry::builder();
        for (uri, schema) in pairs {
            builder = builder.add_resource(*uri, schema.clone());
        }
        builder.build().unwrap()
    }

    #[test]
    fn lookup_self_ref() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"type": "object"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        let resolved = resolver.lookup("#").unwrap();
        assert_eq!(resolved.contents(), &json!({"type": "object"}));
    }

    #[test]
    fn lookup_empty_ref() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"type": "object"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        let resolved = resolver.lookup("").unwrap();
        assert_eq!(resolved.contents(), &json!({"type": "object"}));
    }

    #[test]
    fn lookup_pointer_ref() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({
                "$defs": {
                    "foo": {"type": "string"}
                }
            }),
        )]);
        let resolver = registry.resolver("http://example.com");
        let resolved = resolver.lookup("#/$defs/foo").unwrap();
        assert_eq!(resolved.contents(), &json!({"type": "string"}));
    }

    #[test]
    fn lookup_absolute_uri() {
        let registry = build_registry(&[
            ("http://example.com/a", json!({"type": "string"})),
            ("http://example.com/b", json!({"type": "integer"})),
        ]);
        let resolver = registry.resolver("http://example.com/a");
        let resolved = resolver.lookup("http://example.com/b").unwrap();
        assert_eq!(resolved.contents(), &json!({"type": "integer"}));
    }

    #[test]
    fn lookup_missing_resource() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"type": "string"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        let err = resolver.lookup("http://missing.com").unwrap_err();
        let msg = alloc::format!("{}", err);
        assert!(msg.contains("not found"));
    }

    #[test]
    fn lookup_missing_anchor() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"type": "string"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        let err = resolver.lookup("#noSuchAnchor").unwrap_err();
        let msg = alloc::format!("{}", err);
        assert!(msg.contains("does not exist"));
    }

    #[test]
    fn evolve_pushes_scope() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"type": "string"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        assert!(resolver.dynamic_scope().is_empty());

        let evolved = resolver.evolve("http://example.com/sub");
        assert_eq!(evolved.base_uri(), "http://example.com/sub");
        assert_eq!(evolved.dynamic_scope(), &["http://example.com"]);
    }

    #[test]
    fn recursive_ref_trivial() {
        let registry = build_registry(&[(
            "http://example.com",
            json!({"$recursiveAnchor": true, "type": "object"}),
        )]);
        let resolver = registry.resolver("http://example.com");
        let first = resolver.lookup("").unwrap();
        let resolved = first.resolver().lookup_recursive_ref().unwrap();
        assert_eq!(
            resolved.contents(),
            &json!({"$recursiveAnchor": true, "type": "object"})
        );
    }
}
