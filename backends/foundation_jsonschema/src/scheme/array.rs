//! Array type schema builder — items, length, uniqueness constraints.

use super::{BTreeMap, ValidationOptions, Value};

/// Builder for JSON Schema array type.
///
/// WHY: Array schemas need item constraints, length limits, and uniqueness
/// checks. This builder provides type-safe methods for each.
#[derive(Clone)]
pub struct ArraySchema {
    schema: BTreeMap<String, Value>,
}

impl ArraySchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("array".into()));
        Self { schema }
    }

    /// Set the schema for all items in the array.
    #[must_use]
    pub fn items(mut self, schema: impl Into<Value>) -> Self {
        self.schema.insert("items".into(), schema.into());
        self
    }

    /// Set prefix items (tuple validation, Draft 2020-12).
    #[must_use]
    pub fn prefix_items(mut self, schemas: Vec<impl Into<Value>>) -> Self {
        self.schema.insert(
            "prefixItems".into(),
            Value::Array(schemas.into_iter().map(Into::into).collect()),
        );
        self
    }

    /// Set additional items schema (Draft 4-7).
    #[must_use]
    pub fn additional_items(mut self, schema: impl Into<Value>) -> Self {
        self.schema.insert("additionalItems".into(), schema.into());
        self
    }

    /// Set minimum number of items.
    #[must_use]
    pub fn min_items(mut self, n: usize) -> Self {
        self.schema.insert("minItems".into(), Value::Number(n.into()));
        self
    }

    /// Set maximum number of items.
    #[must_use]
    pub fn max_items(mut self, n: usize) -> Self {
        self.schema.insert("maxItems".into(), Value::Number(n.into()));
        self
    }

    /// Set exact number of items.
    #[must_use]
    pub fn len(self, n: usize) -> Self {
        self.min_items(n).max_items(n)
    }

    /// Require all items to be unique.
    #[must_use]
    pub fn unique(mut self) -> Self {
        self.schema.insert("uniqueItems".into(), Value::Bool(true));
        self
    }

    /// Require at least one item (shorthand for `.min_items(1)`).
    #[must_use]
    pub fn nonempty(self) -> Self {
        self.min_items(1)
    }

    /// Require at least one item to match the given schema (Draft 6+).
    #[must_use]
    pub fn contains(mut self, schema: impl Into<Value>) -> Self {
        self.schema.insert("contains".into(), schema.into());
        self
    }

    /// Set minimum number of items that must match `contains` (Draft 2019-09+).
    #[must_use]
    pub fn min_contains(mut self, n: u64) -> Self {
        self.schema.insert("minContains".into(), Value::Number(n.into()));
        self
    }

    /// Set maximum number of items that can match `contains` (Draft 2019-09+).
    #[must_use]
    pub fn max_contains(mut self, n: u64) -> Self {
        self.schema.insert("maxContains".into(), Value::Number(n.into()));
        self
    }

    /// Add a description to the schema.
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema.insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Add a default value.
    #[must_use]
    pub fn default(mut self, value: Value) -> Self {
        self.schema.insert("default".into(), value);
        self
    }

    /// Wrap this type in `anyOf` with `null`.
    #[must_use]
    pub fn optional(mut self) -> Self {
        let inner = self.build_schema();
        self.schema.clear();
        let mut null_schema = serde_json::Map::default();
        null_schema.insert("type".into(), Value::String("null".into()));
        self.schema.insert("anyOf".into(), Value::Array(vec![
            inner,
            Value::Object(null_schema),
        ]));
        self
    }

    /// Change `"type": "array"` → `"type": ["array", "null"]`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        if let Some(Value::String(t)) = self.schema.get("type").cloned() {
            self.schema.insert("type".into(), Value::Array(vec![
                Value::String(t),
                Value::String("null".into()),
            ]));
        }
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(self.schema.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

/// Create an array schema builder.
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::array()
///     .items(scheme::string().min_len(1))
///     .min_items(1)
///     .max_items(10)
///     .unique()
///     .build_schema();
/// ```
#[must_use]
pub fn array() -> ArraySchema {
    ArraySchema::new()
}

/// Create an array schema builder pre-configured with an items schema.
///
/// Shorthand for `array().items(schema)`.
#[must_use]
pub fn array_of(schema: impl Into<Value>) -> ArraySchema {
    array().items(schema)
}

// ── From<Value> for ArraySchema ──────────────────────────────────────

impl From<ArraySchema> for Value {
    fn from(builder: ArraySchema) -> Self {
        builder.build_schema()
    }
}
