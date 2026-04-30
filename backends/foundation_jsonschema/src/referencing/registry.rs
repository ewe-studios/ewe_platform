//! Registry of JSON Schema resources indexed by URI.
//!
//! WHY: During schema compilation, `$ref` keywords need to look up other
//! schemas by URI. The registry pre-indexes all known schemas (user-provided
//! and externally resolved) so lookups are O(log n).
//!
//! WHAT: `Registry` is an immutable collection of `Resource` values keyed
//! by URI. `RegistryBuilder` builds it via BFS crawling.
//!
//! HOW: Built via `RegistryBuilder`. The build phase crawls all schemas,
//! discovers sub-resources via `$id`, resolves external refs via `JsonResolver`,
//! and creates a URI → Resource index plus a (URI, `anchor_name`) → Anchor index.
//! After build, the registry is immutable.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::draft::Draft;
use crate::resolver_trait::{JsonResolver, NoopResolver};

use super::anchor::Anchor;
use super::resolver::Resolver;
use super::resource::{Resource, ResourceRef};
use super::spec;
use super::uri;

/// A prepared registry of JSON Schema resources indexed by URI.
///
/// WHY: During schema compilation, `$ref` keywords need O(log n) lookup of
/// other schemas by URI.
///
/// WHAT: Immutable map of URI → `Resource` plus (URI, `anchor_name`) → `AnchorEntry`.
///
/// HOW: Built via `RegistryBuilder::build()`. All mutations happen during
/// the build phase.
pub struct Registry {
    resources: BTreeMap<String, Resource>,
    anchors: BTreeMap<(String, String), AnchorEntry>,
}

/// Stored anchor metadata — enough to reconstruct an `Anchor` on demand.
#[derive(Debug, Clone)]
pub(crate) struct AnchorEntry {
    pub(crate) is_dynamic: bool,
}

/// Builder for constructing a `Registry`.
///
/// WHY: Registry construction is a multi-phase process: collect schemas,
/// crawl for sub-resources and external refs, resolve externals, index
/// everything. The builder pattern separates configuration from building.
pub struct RegistryBuilder {
    pending: Vec<(String, Value)>,
    resolver: Box<dyn JsonResolver>,
    default_draft: Draft,
}

impl RegistryBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            resolver: Box::new(NoopResolver),
            default_draft: Draft::DEFAULT,
        }
    }

    /// Set the external resolver.
    #[must_use]
    pub fn with_resolver(mut self, resolver: impl JsonResolver + 'static) -> Self {
        self.resolver = Box::new(resolver);
        self
    }

    /// Set the default draft for schemas that don't declare `$schema`.
    #[must_use]
    pub fn with_draft(mut self, draft: Draft) -> Self {
        self.default_draft = draft;
        self
    }

    /// Add a schema resource at the given URI.
    #[must_use]
    pub fn add_resource(mut self, uri: impl Into<String>, schema: Value) -> Self {
        let mut u: String = uri.into();
        if u.ends_with('#') {
            u.pop();
        }
        self.pending.push((u, schema));
        self
    }

    /// Build the registry: crawl schemas, resolve external refs, index everything.
    ///
    /// WHY: After all resources are added, the registry must be built to index
    /// them and resolve any external references.
    ///
    /// WHAT: Returns an immutable `Registry` ready for use by the compiler.
    ///
    /// HOW: BFS crawl over all schemas. For each schema:
    /// 1. Detect draft from `$schema` or use default
    /// 2. Extract `$id` to discover sub-resources
    /// 3. Extract anchors (`$anchor`, `$dynamicAnchor`, legacy)
    /// 4. Collect `$ref` targets; resolve external ones via `JsonResolver`
    /// 5. Index everything by URI and (URI, `anchor_name`)
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if an external reference cannot be resolved.
    pub fn build(self) -> Result<Registry, crate::error::ValidationError> {
        let mut resources: BTreeMap<String, Resource> = BTreeMap::new();
        let mut anchors: BTreeMap<(String, String), AnchorEntry> = BTreeMap::new();
        let mut external_refs: Vec<String> = Vec::new();
        let mut known_uris: alloc::collections::BTreeSet<String> =
            alloc::collections::BTreeSet::new();

        // Phase 1: Register all user-provided resources
        for (uri, schema) in &self.pending {
            let draft = Draft::detect(schema).unwrap_or(self.default_draft);
            known_uris.insert(uri.clone());
            resources.insert(uri.clone(), Resource::with_draft(schema.clone(), draft));
        }

        // Phase 2: BFS crawl — discover sub-resources, anchors, and external refs
        let mut queue: alloc::collections::VecDeque<(String, String)> =
            alloc::collections::VecDeque::new();

        for (uri, _schema) in &self.pending {
            queue.push_back((uri.clone(), uri.clone()));
        }

        while let Some((base_uri, resource_uri)) = queue.pop_front() {
            let Some(resource) = resources.get(&resource_uri) else {
                continue;
            };
            let draft = resource.draft();
            let contents = resource.contents().clone();

            crawl_schema(
                &base_uri,
                &contents,
                draft,
                &mut resources,
                &mut anchors,
                &mut external_refs,
                &mut known_uris,
            );
        }

        // Phase 3: Resolve external references
        let mut iteration = 0;
        while !external_refs.is_empty() && iteration < 100 {
            iteration += 1;
            let refs: Vec<String> = std::mem::take(&mut external_refs);
            for ext_uri in refs {
                let fragmentless = ext_uri.split('#').next().unwrap_or(&ext_uri).to_string();
                if known_uris.contains(&fragmentless) {
                    continue;
                }

                let resolved_value = self.resolver.resolve(&fragmentless).map_err(|e| {
                    use foundation_errstacks::IntoErrorTrace;
                    crate::error::ValidationErrorKind::Schema {
                        reason: alloc::format!(
                            "failed to resolve external reference: {fragmentless}"
                        ),
                    }
                    .into_error_trace()
                })?;

                let draft = Draft::detect(&resolved_value).unwrap_or(self.default_draft);
                known_uris.insert(fragmentless.clone());
                resources.insert(
                    fragmentless.clone(),
                    Resource::with_draft(resolved_value.clone(), draft),
                );

                // Crawl the newly resolved resource
                let mut sub_queue = alloc::collections::VecDeque::new();
                sub_queue.push_back((fragmentless.clone(), fragmentless.clone()));
                while let Some((base, res_uri)) = sub_queue.pop_front() {
                    let Some(resource) = resources.get(&res_uri) else {
                        continue;
                    };
                    let d = resource.draft();
                    let c = resource.contents().clone();
                    crawl_schema(
                        &base,
                        &c,
                        d,
                        &mut resources,
                        &mut anchors,
                        &mut external_refs,
                        &mut known_uris,
                    );
                }
            }
        }

        Ok(Registry { resources, anchors })
    }
}

impl Default for RegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// BFS crawl a schema, extracting sub-resources, anchors, and `$ref` targets.
#[allow(clippy::too_many_arguments)]
fn crawl_schema(
    base_uri: &str,
    contents: &Value,
    draft: Draft,
    resources: &mut BTreeMap<String, Resource>,
    anchors: &mut BTreeMap<(String, String), AnchorEntry>,
    external_refs: &mut Vec<String>,
    known_uris: &mut alloc::collections::BTreeSet<String>,
) {
    let Some(object) = contents.as_object() else {
        return;
    };

    let analysis = spec::analyze_object(draft, object);
    let mut effective_base = base_uri.to_string();

    // Handle $id — creates a new sub-resource
    if let Some(id) = analysis.id {
        let resolved_id = resolve_id(base_uri, id);
        if !known_uris.contains(&resolved_id) {
            known_uris.insert(resolved_id.clone());
            resources.insert(
                resolved_id.clone(),
                Resource::with_draft(contents.clone(), draft),
            );
        }
        effective_base = resolved_id;
    }

    // Handle anchors
    for anchor in spec::anchors_of(draft, contents) {
        let entry = AnchorEntry {
            is_dynamic: matches!(anchor, Anchor::Dynamic { .. }),
        };
        anchors.insert((effective_base.clone(), anchor.name().to_string()), entry);
    }

    // Collect $ref targets
    if let Some(dollar_ref) = analysis.dollar_ref {
        if !dollar_ref.starts_with('#') {
            let resolved = resolve_id(&effective_base, dollar_ref);
            let fragmentless = resolved.split('#').next().unwrap_or(&resolved).to_string();
            if !known_uris.contains(&fragmentless) {
                external_refs.push(resolved);
            }
        }
    }

    // Recurse into sub-schemas
    for child in spec::subresources_of(draft, contents) {
        let child_draft = Draft::detect(child).unwrap_or(draft);
        crawl_schema(
            &effective_base,
            child,
            child_draft,
            resources,
            anchors,
            external_refs,
            known_uris,
        );
    }
}

/// Resolve an ID against a base URI, producing an absolute URI.
fn resolve_id(base: &str, id: &str) -> String {
    if id.starts_with('#') {
        return base.to_string();
    }
    let normalized = id.strip_suffix('#').unwrap_or(id);

    match uri::resolve_against(base, normalized) {
        Ok(resolved) => {
            let s = resolved.as_str().to_string();
            s.split('#').next().unwrap_or(&s).to_string()
        }
        Err(_) => normalized.to_string(),
    }
}

impl Registry {
    /// Create a new `RegistryBuilder`.
    #[must_use]
    pub fn builder() -> RegistryBuilder {
        RegistryBuilder::new()
    }

    /// Create a `Resolver` rooted at the given base URI.
    #[must_use]
    pub fn resolver(&self, base_uri: &str) -> Resolver<'_> {
        Resolver::new(self, base_uri.to_string())
    }

    /// Look up a resource by URI.
    #[must_use]
    pub fn get_resource(&self, uri: &str) -> Option<ResourceRef<'_>> {
        self.resources
            .get(uri)
            .map(|r| ResourceRef::new(r.contents(), r.draft()))
    }

    /// Look up an anchor by base URI and anchor name.
    #[must_use]
    pub fn get_anchor<'a>(&'a self, base_uri: &str, name: &str) -> Option<Anchor<'a>> {
        let entry = self
            .anchors
            .get(&(base_uri.to_string(), name.to_string()))?;
        let resource = self.get_resource(base_uri)?;

        // Find the actual sub-schema with this anchor
        let contents = self.find_anchor_contents(base_uri, name)?;
        let resource_ref = ResourceRef::new(contents, resource.draft());

        if entry.is_dynamic {
            Some(Anchor::Dynamic {
                name: self.get_anchor_name(base_uri, name)?,
                resource: resource_ref,
            })
        } else {
            Some(Anchor::Default {
                name: self.get_anchor_name(base_uri, name)?,
                resource: resource_ref,
            })
        }
    }

    /// Find the contents of a schema that declares a given anchor.
    fn find_anchor_contents(&self, base_uri: &str, anchor_name: &str) -> Option<&Value> {
        let resource = self.resources.get(base_uri)?;
        find_anchor_in_value(resource.contents(), resource.draft(), anchor_name)
    }

    /// Get a stable reference to an anchor name stored in the anchors map.
    fn get_anchor_name(&self, base_uri: &str, name: &str) -> Option<&str> {
        let key = (base_uri.to_string(), name.to_string());
        self.anchors
            .get_key_value(&key)
            .map(|((_, n), _)| n.as_str())
    }

    /// Check if the registry contains a resource at the given URI.
    #[must_use]
    pub fn contains_resource(&self, uri: &str) -> bool {
        self.resources.contains_key(uri)
    }

    /// Resolve a URI reference against a base, using the registry's knowledge.
    #[allow(clippy::unused_self)]
    pub(crate) fn resolve_uri(
        &self,
        base: &str,
        reference: &str,
    ) -> Result<String, super::uri::UriError> {
        let resolved = uri::resolve_against(base, reference)?;
        Ok(resolved.without_fragment().to_string())
    }
}

/// Recursively search for a sub-schema declaring the given anchor.
fn find_anchor_in_value<'a>(
    value: &'a Value,
    draft: Draft,
    anchor_name: &str,
) -> Option<&'a Value> {
    for anchor in spec::anchors_of(draft, value) {
        if anchor.name() == anchor_name {
            return Some(value);
        }
    }

    for child in spec::subresources_of(draft, value) {
        let child_draft = Draft::detect(child).unwrap_or(draft);
        if let Some(found) = find_anchor_in_value(child, child_draft, anchor_name) {
            return Some(found);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver_trait::MapResolver;
    use serde_json::json;

    #[test]
    fn build_empty_registry() {
        let registry = Registry::builder().build().unwrap();
        assert!(!registry.contains_resource("anything"));
    }

    #[test]
    fn build_single_resource() {
        let registry = Registry::builder()
            .add_resource("http://example.com", json!({"type": "string"}))
            .build()
            .unwrap();
        assert!(registry.contains_resource("http://example.com"));
        let r = registry.get_resource("http://example.com").unwrap();
        assert_eq!(r.contents(), &json!({"type": "string"}));
    }

    #[test]
    fn build_discovers_subresource() {
        let registry = Registry::builder()
            .add_resource(
                "http://example.com/root",
                json!({
                    "$defs": {
                        "inner": {
                            "$id": "http://example.com/inner",
                            "type": "integer"
                        }
                    }
                }),
            )
            .build()
            .unwrap();
        assert!(registry.contains_resource("http://example.com/root"));
        assert!(registry.contains_resource("http://example.com/inner"));
    }

    #[test]
    fn build_discovers_anchors() {
        let registry = Registry::builder()
            .add_resource(
                "http://example.com",
                json!({
                    "$defs": {
                        "foo": {
                            "$anchor": "myAnchor",
                            "type": "string"
                        }
                    }
                }),
            )
            .build()
            .unwrap();

        assert!(registry
            .anchors
            .contains_key(&("http://example.com".into(), "myAnchor".into())));
    }

    #[test]
    fn build_resolves_external_refs() {
        let mut resolver = MapResolver::new();
        resolver.insert("http://example.com/external", json!({"type": "number"}));

        let registry = Registry::builder()
            .with_resolver(resolver)
            .add_resource(
                "http://example.com/root",
                json!({"$ref": "http://example.com/external"}),
            )
            .build()
            .unwrap();

        assert!(registry.contains_resource("http://example.com/external"));
    }

    #[test]
    fn build_with_explicit_draft() {
        let registry = Registry::builder()
            .with_draft(Draft::Draft4)
            .add_resource("urn:test", json!({}))
            .build()
            .unwrap();
        let r = registry.get_resource("urn:test").unwrap();
        assert_eq!(r.draft(), Draft::Draft4);
    }

    #[test]
    fn build_strips_trailing_hash() {
        let registry = Registry::builder()
            .add_resource("http://example.com/schema#", json!({"type": "string"}))
            .build()
            .unwrap();
        assert!(registry.contains_resource("http://example.com/schema"));
    }

    #[test]
    fn resolve_id_absolute() {
        assert_eq!(
            resolve_id("http://example.com/base", "http://other.com/schema"),
            "http://other.com/schema"
        );
    }

    #[test]
    fn resolve_id_relative() {
        assert_eq!(
            resolve_id("http://example.com/schemas/base.json", "inner"),
            "http://example.com/schemas/inner"
        );
    }

    #[test]
    fn resolve_id_fragment_returns_base() {
        assert_eq!(
            resolve_id("http://example.com/base", "#anchor"),
            "http://example.com/base"
        );
    }
}
