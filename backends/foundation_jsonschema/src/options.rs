//! `ValidationOptions` builder — configure schema compilation and validation.

use crate::draft::Draft;
use crate::error::ValidationError;
use crate::in_memory_fetcher::InMemoryFetcher;
use crate::referencing::Registry;
use crate::validator::Validator;
use serde_json::Value;

/// Builder for configuring schema compilation and validation behavior.
///
/// WHY: Different schemas may need different resolvers, draft settings, or
/// custom keywords. This builder provides an ergonomic configuration API.
pub struct ValidationOptions {
    default_draft: Draft,
    resolver: InMemoryFetcher,
}

impl ValidationOptions {
    /// Create a new builder with default settings.
    ///
    /// The default resolver is `InMemoryFetcher::builtin()` which resolves
    /// all well-known JSON Schema meta-schema URIs (draft-04 through draft-2020-12).
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_draft: Draft::DEFAULT,
            resolver: InMemoryFetcher::builtin(),
        }
    }

    /// Set the default draft for schemas that don't declare `$schema`.
    #[must_use]
    pub fn with_draft(mut self, draft: Draft) -> Self {
        self.default_draft = draft;
        self
    }

    /// Replace the resolver with a custom one.
    ///
    /// Use this to provide your own external reference resolution,
    /// or to add custom schemas alongside the built-in meta-schemas.
    #[must_use]
    pub fn with_resolver(mut self, resolver: InMemoryFetcher) -> Self {
        self.resolver = resolver;
        self
    }

    /// Compile the schema with these options.
    ///
    /// WHY: This is the main entry point for schema compilation. It builds
    /// a registry from the schema, detects the draft, and compiles the
    /// validator tree.
    ///
    /// WHAT: Returns a `Validator` ready for validating instances.
    ///
    /// HOW: Builds a `Registry` from the schema, creates a `CompilerContext`,
    /// and compiles the schema into a `SchemaNode`.
    ///
    /// # Errors
    ///
    /// Returns an error if schema compilation fails (invalid schema structure,
    /// unresolvable references, etc.).
    pub fn build(self, schema: &Value) -> Result<Validator, ValidationError> {
        let draft = Draft::detect(schema).unwrap_or(self.default_draft);

        let registry = Registry::builder()
            .with_resolver(self.resolver)
            .with_draft(draft)
            .add_resource("", schema.clone())
            .build()?;

        let base_uri = "";
        let root_node = crate::compiler::compile(schema, &registry, draft)?;

        let _ = base_uri;

        Ok(Validator::new(root_node, draft))
    }
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self::new()
    }
}
