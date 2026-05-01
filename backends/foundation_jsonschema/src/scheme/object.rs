//! Object type schema builder — properties, required, additional properties.

use super::{ValidationOptions, BTreeMap, Value};

/// Builder for JSON Schema object type.
///
/// WHY: Object schemas need property definitions, required field lists,
/// and additionalProperties configuration. This builder provides type-safe
/// methods for each.
#[derive(Clone)]
pub struct ObjectSchema {
    schema: BTreeMap<String, Value>,
}

impl ObjectSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("object".into()));
        Self { schema }
    }

    /// Add a required property.
    ///
    /// The property schema accepts any value convertible into `serde_json::Value`,
    /// so you can pass `scheme::string().build_schema()` or a raw `Value`.
    #[must_use]
    pub fn required(mut self, name: &str, schema: impl Into<Value>) -> Self {
        self.ensure_properties();
        self.ensure_required();

        if let Some(props) = self.schema.get_mut("properties").and_then(Value::as_object_mut) {
            props.insert(name.into(), schema.into());
        }
        if let Some(req) = self.schema.get_mut("required").and_then(Value::as_array_mut) {
            let val = Value::String(name.into());
            if !req.contains(&val) {
                req.push(val);
            }
        }
        self
    }

    /// Add an optional property (not in `required` list).
    #[must_use]
    pub fn optional(mut self, name: &str, schema: impl Into<Value>) -> Self {
        self.ensure_properties();

        if let Some(props) = self.schema.get_mut("properties").and_then(Value::as_object_mut) {
            props.insert(name.into(), schema.into());
        }
        self
    }

    /// Bulk-add properties from a map. None are marked required.
    #[must_use]
    pub fn properties(mut self, map: BTreeMap<String, Value>) -> Self {
        self.ensure_properties();

        if let Some(props) = self.schema.get_mut("properties").and_then(Value::as_object_mut) {
            for (k, v) in map {
                props.insert(k, v);
            }
        }
        self
    }

    /// Set `additionalProperties: false` (disallow unknown fields).
    #[must_use]
    pub fn strict(self) -> Self {
        self.additional_properties(Value::Bool(false))
    }

    /// Set `additionalProperties: true` (allow unknown fields, default).
    #[must_use]
    pub fn loose(self) -> Self {
        self.additional_properties(Value::Bool(true))
    }

    /// Set `additionalProperties` to a schema (unknown fields must match).
    #[must_use]
    pub fn catchall(self, schema: impl Into<Value>) -> Self {
        self.additional_properties(schema.into())
    }

    /// Add a pattern property — fields matching this regex use the given schema.
    #[must_use]
    pub fn pattern_property(mut self, re: &str, schema: impl Into<Value>) -> Self {
        self.ensure_pattern_properties();

        if let Some(pp) = self.schema
            .get_mut("patternProperties")
            .and_then(Value::as_object_mut)
        {
            pp.insert(re.into(), schema.into());
        }
        self
    }

    /// Set minimum number of properties.
    #[must_use]
    pub fn min_properties(mut self, n: usize) -> Self {
        self.schema.insert("minProperties".into(), Value::Number(n.into()));
        self
    }

    /// Set maximum number of properties.
    #[must_use]
    pub fn max_properties(mut self, n: usize) -> Self {
        self.schema.insert("maxProperties".into(), Value::Number(n.into()));
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

    /// Wrap this entire object schema in `anyOf` with `null`.
    ///
    /// Note: this differs from `optional(name, schema)` which adds an optional
    /// property. Use this when the object itself should accept null.
    #[must_use]
    pub fn optional_value(mut self) -> Self {
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

    /// Change `"type": "object"` → `"type": ["object", "null"]`.
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

    // ── Internals ──────────────────────────────────────────────

    fn additional_properties(mut self, value: Value) -> Self {
        self.schema.insert("additionalProperties".into(), value);
        self
    }

    fn ensure_properties(&mut self) {
        self.schema
            .entry("properties".into())
            .or_insert_with(|| Value::Object(Default::default()));
    }

    fn ensure_required(&mut self) {
        self.schema
            .entry("required".into())
            .or_insert_with(|| Value::Array(Vec::new()));
    }

    fn ensure_pattern_properties(&mut self) {
        self.schema
            .entry("patternProperties".into())
            .or_insert_with(|| Value::Object(Default::default()));
    }
}

/// Create an object schema builder.
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::object()
///     .required("name", scheme::string().min_len(1))
///     .optional("age", scheme::integer().min(0))
///     .strict()
///     .build_schema();
/// ```
#[must_use]
pub fn object() -> ObjectSchema {
    ObjectSchema::new()
}

// ── From<Value> for ObjectSchema ──────────────────────────────────────

impl From<ObjectSchema> for Value {
    fn from(builder: ObjectSchema) -> Self {
        builder.build_schema()
    }
}
