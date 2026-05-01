//! `ValidationOptions` builder — configure schema compilation and validation.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::draft::Draft;
use crate::error::ValidationError;
use crate::formats::FormatChecker;
use crate::in_memory_fetcher::InMemoryFetcher;
use crate::keywords::custom::KeywordFactory;
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
    assert_format: bool,
    custom_keywords: BTreeMap<String, Box<dyn KeywordFactory>>,
    custom_formats: BTreeMap<String, Box<dyn FormatChecker>>,
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
            assert_format: false,
            custom_keywords: BTreeMap::new(),
            custom_formats: BTreeMap::new(),
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

    /// Enable or disable format assertion mode.
    ///
    /// When true, the `format` keyword performs validation rather than
    /// just annotation collection (the default per JSON Schema spec).
    #[must_use]
    pub fn assert_format(mut self, assert: bool) -> Self {
        self.assert_format = assert;
        self
    }

    /// Register a custom keyword factory.
    ///
    /// The factory will be invoked when the compiler encounters a keyword
    /// with the given name in a schema.
    #[must_use]
    pub fn with_keyword(mut self, name: &str, factory: Box<dyn KeywordFactory>) -> Self {
        self.custom_keywords.insert(name.into(), factory);
        self
    }

    /// Register a custom format checker.
    ///
    /// If a format with the same name already exists, this replaces it,
    /// allowing custom checkers to override built-in ones.
    #[must_use]
    pub fn with_format(mut self, name: &str, checker: Box<dyn FormatChecker>) -> Self {
        self.custom_formats.insert(name.into(), checker);
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

        // Extract the schema's own id to use as base_uri for registry registration.
        // Draft 4 uses "id"; Draft 6+ uses "$id". Strip trailing '#' for consistency
        // with add_resource's internal normalization.
        let id_keyword = draft.id_keyword();
        let base_uri = schema
            .as_object()
            .and_then(|o| o.get(id_keyword))
            .and_then(|v| v.as_str())
            .map_or("", |s| s.strip_suffix('#').unwrap_or(s));

        let registry = Registry::builder()
            .with_resolver(self.resolver)
            .with_draft(draft)
            .add_resource(base_uri, schema.clone())
            .build()?;

        let root_node = crate::compiler::compile(
            schema,
            &registry,
            draft,
            self.assert_format,
            self.custom_formats,
            self.custom_keywords,
        )?;

        Ok(Validator::new(root_node, draft))
    }
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self::new()
    }
}
