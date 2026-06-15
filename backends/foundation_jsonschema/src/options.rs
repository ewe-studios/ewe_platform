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
///
/// HOW: Two compile paths are available:
/// - `build(&self, schema)` — compile a caller-provided schema (unchanged API).
/// - `compile(self)` — compile a schema embedded via a `scheme` builder's `.build()`
///   method. Adds `.schema()`, `.into_schema()`, `.clone_schema()` accessors.
pub struct ValidationOptions {
    default_draft: Draft,
    resolver: InMemoryFetcher,
    assert_format: bool,
    custom_keywords: BTreeMap<String, Box<dyn KeywordFactory>>,
    custom_formats: BTreeMap<String, Box<dyn FormatChecker>>,
    /// Embedded schema from a `scheme` builder. Set by `SchemaBuilder::build()`.
    pub schema: Option<Value>,
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
            schema: None,
        }
    }

    /// Create a new builder with an embedded schema already set.
    ///
    /// WHY: The `scheme` builder produces `ValidationOptions` with schema
    /// embedded. This constructor lets you create one from a raw `Value`
    /// directly — e.g., for `Args { schema, validator }` patterns.
    #[must_use]
    pub fn with_schema(schema: Value) -> Self {
        let mut options = Self::new();
        options.schema = Some(schema);
        options
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

    /// Compile the embedded schema with these options.
    ///
    /// WHY: Primary compile path for schemas built via the `scheme` builder
    /// API. The schema is already embedded by `SchemaBuilder::build()`.
    ///
    /// WHAT: Returns a `Validator` ready for validating instances.
    ///
    /// HOW: Extracts the embedded schema and runs the same compilation
    /// pipeline as `build()`.
    ///
    /// # Panics
    ///
    /// Panics if no schema has been embedded. Use `ValidationOptions::build(&Value)`
    /// for manual schema construction, or call `scheme::Builder::build()` first.
    ///
    /// # Errors
    ///
    /// Returns a `ValidationError` if the embedded schema fails to compile
    /// into a `Validator`.
    pub fn compile(self) -> Result<Validator, ValidationError> {
        let schema = self.schema.as_ref()
            .expect("schema must be set before compile() — use scheme::Builder::build() or ValidationOptions::build(schema)");
        Self::_do_compile(&self, schema)
    }

    /// Reference to the stored schema.
    ///
    /// # Panics
    ///
    /// Panics if no schema has been embedded.
    #[must_use]
    pub fn schema(&self) -> &Value {
        self.schema
            .as_ref()
            .expect("schema must be set — use scheme::Builder::build() first")
    }

    /// Consume and take ownership of the stored schema.
    ///
    /// # Panics
    ///
    /// Panics if no schema has been embedded.
    #[must_use]
    pub fn into_schema(mut self) -> Value {
        self.schema
            .take()
            .expect("schema must be set — use scheme::Builder::build() first")
    }

    /// Clone the stored schema.
    ///
    /// # Panics
    ///
    /// Panics if no schema has been embedded.
    #[must_use]
    pub fn clone_schema(&self) -> Value {
        self.schema
            .as_ref()
            .expect("schema must be set — use scheme::Builder::build() first")
            .clone()
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
        Self::_do_compile(&self, schema)
    }
}

impl ValidationOptions {
    /// Shared compilation logic used by both `build(schema)` and `compile()`.
    ///
    /// WHY: Both entry points run the same pipeline — registry build, draft
    /// detection, ID extraction, compiler invocation. This avoids duplication.
    fn _do_compile(&self, schema: &Value) -> Result<Validator, ValidationError> {
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
            .with_resolver(self.resolver.clone())
            .with_draft(draft)
            .add_resource(base_uri, schema.clone())
            .build()?;

        let root_node = crate::compiler::compile(
            schema,
            &registry,
            draft,
            self.assert_format,
            self.custom_formats.clone(),
            self.custom_keywords.clone(),
        )?;

        Ok(Validator::new(root_node, draft))
    }
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self::new()
    }
}
