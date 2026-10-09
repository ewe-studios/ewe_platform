//! Primitive type schema builders — string, integer, number, boolean, null.

use super::{BTreeMap, ValidationOptions, Value};

/// Builder for JSON Schema string type.
///
/// WHY: String schemas need length, pattern, and format constraints.
/// This builder provides type-safe methods for each.
///
/// HOW: Accumulates JSON Schema properties in a `BTreeMap`.
/// `.build_schema()` converts to a `serde_json::Value`.
/// `.build()` embeds the schema into `ValidationOptions` for compilation.
#[derive(Clone)]
pub struct StringSchema {
    schema: BTreeMap<String, Value>,
}

impl StringSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("string".into()));
        Self { schema }
    }

    /// Set minimum string length (`"minLength"`).
    #[must_use]
    pub fn min_len(mut self, n: usize) -> Self {
        self.schema
            .insert("minLength".into(), Value::Number(n.into()));
        self
    }

    /// Set maximum string length (`"maxLength"`).
    #[must_use]
    pub fn max_len(mut self, n: usize) -> Self {
        self.schema
            .insert("maxLength".into(), Value::Number(n.into()));
        self
    }

    /// Set exact string length (both `minLength` and `maxLength`).
    #[must_use]
    pub fn len(self, n: usize) -> Self {
        self.min_len(n).max_len(n)
    }

    /// Set regex pattern (`"pattern"`).
    #[must_use]
    pub fn pattern(mut self, re: &str) -> Self {
        self.schema
            .insert("pattern".into(), Value::String(re.into()));
        self
    }

    /// Set format (`"format"`).
    #[must_use]
    pub fn format(mut self, f: &str) -> Self {
        self.schema.insert("format".into(), Value::String(f.into()));
        self
    }

    /// Shorthand for `.format("email")`.
    #[must_use]
    pub fn email(self) -> Self {
        self.format("email")
    }

    /// Shorthand for `.format("uri")`.
    #[must_use]
    pub fn uri(self) -> Self {
        self.format("uri")
    }

    /// Shorthand for `.format("uuid")`.
    #[must_use]
    pub fn uuid(self) -> Self {
        self.format("uuid")
    }

    /// Shorthand for `.format("date-time")`.
    #[must_use]
    pub fn date_time(self) -> Self {
        self.format("date-time")
    }

    /// Shorthand for `.format("hostname")`.
    #[must_use]
    pub fn hostname(self) -> Self {
        self.format("hostname")
    }

    /// Shorthand for `.format("ipv4")`.
    #[must_use]
    pub fn ipv4(self) -> Self {
        self.format("ipv4")
    }

    /// Shorthand for `.format("ipv6")`.
    #[must_use]
    pub fn ipv6(self) -> Self {
        self.format("ipv6")
    }

    /// Shorthand for `.format("base64")`.
    #[must_use]
    pub fn base64(self) -> Self {
        self.format("base64")
    }

    /// Set enum values for this string type.
    #[must_use]
    pub fn enum_values(mut self, vals: Vec<String>) -> Self {
        self.schema.insert(
            "enum".into(),
            Value::Array(vals.into_iter().map(Value::String).collect()),
        );
        self
    }

    /// Wrap this type in `anyOf` with `null` — makes the field accept `null` values.
    #[must_use]
    pub fn optional(mut self) -> Self {
        let inner = Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        self.schema.clear();
        let mut null_schema = serde_json::Map::default();
        null_schema.insert("type".into(), Value::String("null".into()));
        self.schema.insert(
            "anyOf".into(),
            Value::Array(vec![inner, Value::Object(null_schema)]),
        );
        self
    }

    /// Change `"type": "string"` → `"type": ["string", "null"]`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        if let Some(Value::String(t)) = self.schema.get("type").cloned() {
            self.schema.insert(
                "type".into(),
                Value::Array(vec![Value::String(t), Value::String("null".into())]),
            );
        }
        self
    }

    /// Add a description to the schema (`"description"`).
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema
            .insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Add a default value (`"default"`).
    #[must_use]
    pub fn default(mut self, value: Value) -> Self {
        self.schema.insert("default".into(), value);
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    /// Chain with `.compile()` for immediate validation.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

// ── Integer Schema ────────────────────────────────────────────

/// Builder for JSON Schema integer type.
///
/// WHY: Integer schemas need numeric range and divisibility constraints.
/// This builder provides type-safe methods for each.
#[derive(Clone)]
pub struct IntegerSchema {
    schema: BTreeMap<String, Value>,
}

impl IntegerSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("integer".into()));
        Self { schema }
    }

    /// Set minimum value (inclusive, `"minimum"`).
    #[must_use]
    pub fn min(mut self, n: i64) -> Self {
        self.schema
            .insert("minimum".into(), Value::Number(n.into()));
        self
    }

    /// Set maximum value (inclusive, `"maximum"`).
    #[must_use]
    pub fn max(mut self, n: i64) -> Self {
        self.schema
            .insert("maximum".into(), Value::Number(n.into()));
        self
    }

    /// Set exclusive minimum (Draft 6+, `"exclusiveMinimum"`).
    #[must_use]
    pub fn exclusive_min(mut self, n: i64) -> Self {
        self.schema
            .insert("exclusiveMinimum".into(), Value::Number(n.into()));
        self
    }

    /// Set exclusive maximum (Draft 6+, `"exclusiveMaximum"`).
    #[must_use]
    pub fn exclusive_max(mut self, n: i64) -> Self {
        self.schema
            .insert("exclusiveMaximum".into(), Value::Number(n.into()));
        self
    }

    /// Set multiple-of divisor (`"multipleOf"`).
    #[must_use]
    pub fn multiple_of(mut self, n: i64) -> Self {
        self.schema
            .insert("multipleOf".into(), Value::Number(n.into()));
        self
    }

    /// Shorthand for `.exclusive_min(0)` (must be positive, Draft 6+).
    #[must_use]
    pub fn positive(self) -> Self {
        self.exclusive_min(0)
    }

    /// Shorthand for `.min(0)` (must be non-negative, Draft 6+).
    #[must_use]
    pub fn nonnegative(self) -> Self {
        self.min(0)
    }

    /// Shorthand for `.exclusive_max(0)` (must be negative, Draft 6+).
    #[must_use]
    pub fn negative(self) -> Self {
        self.exclusive_max(0)
    }

    /// Shorthand for `.max(0)` (must be non-positive, Draft 6+).
    #[must_use]
    pub fn nonpositive(self) -> Self {
        self.max(0)
    }

    /// Set enum values for this integer type.
    #[must_use]
    pub fn enum_values(mut self, vals: Vec<i64>) -> Self {
        self.schema.insert(
            "enum".into(),
            Value::Array(vals.into_iter().map(|n| Value::Number(n.into())).collect()),
        );
        self
    }

    /// Wrap this type in `anyOf` with `null`.
    #[must_use]
    pub fn optional(mut self) -> Self {
        let inner = self.build_schema();
        self.schema.clear();
        let mut null_schema = serde_json::Map::default();
        null_schema.insert("type".into(), Value::String("null".into()));
        self.schema.insert(
            "anyOf".into(),
            Value::Array(vec![inner, Value::Object(null_schema)]),
        );
        self
    }

    /// Change `"type": "integer"` → `"type": ["integer", "null"]`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        if let Some(Value::String(t)) = self.schema.get("type").cloned() {
            self.schema.insert(
                "type".into(),
                Value::Array(vec![Value::String(t), Value::String("null".into())]),
            );
        }
        self
    }

    /// Add a description to the schema (`"description"`).
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema
            .insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Add a default value (`"default"`).
    #[must_use]
    pub fn default(mut self, value: Value) -> Self {
        self.schema.insert("default".into(), value);
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

// ── Number Schema ──────────────────────────────────────────────

/// Builder for JSON Schema number type (integer or float).
///
/// WHY: Number schemas need numeric range constraints with f64 precision.
#[derive(Clone)]
pub struct NumberSchema {
    schema: BTreeMap<String, Value>,
}

impl NumberSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("number".into()));
        Self { schema }
    }

    /// Set minimum value (inclusive, `"minimum"`).
    #[must_use]
    pub fn min(mut self, n: f64) -> Self {
        self.schema.insert(
            "minimum".into(),
            serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
        );
        self
    }

    /// Set maximum value (inclusive, `"maximum"`).
    #[must_use]
    pub fn max(mut self, n: f64) -> Self {
        self.schema.insert(
            "maximum".into(),
            serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
        );
        self
    }

    /// Set exclusive minimum (Draft 6+, `"exclusiveMinimum"`).
    #[must_use]
    pub fn exclusive_min(mut self, n: f64) -> Self {
        self.schema.insert(
            "exclusiveMinimum".into(),
            serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
        );
        self
    }

    /// Set exclusive maximum (Draft 6+, `"exclusiveMaximum"`).
    #[must_use]
    pub fn exclusive_max(mut self, n: f64) -> Self {
        self.schema.insert(
            "exclusiveMaximum".into(),
            serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
        );
        self
    }

    /// Set multiple-of divisor (`"multipleOf"`).
    #[must_use]
    pub fn multiple_of(mut self, n: f64) -> Self {
        self.schema.insert(
            "multipleOf".into(),
            serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
        );
        self
    }

    /// Shorthand for `.exclusive_min(0.0)`.
    #[must_use]
    pub fn positive(self) -> Self {
        self.exclusive_min(0.0)
    }

    /// Shorthand for `.min(0.0)`.
    #[must_use]
    pub fn nonnegative(self) -> Self {
        self.min(0.0)
    }

    /// Shorthand for `.exclusive_max(0.0)`.
    #[must_use]
    pub fn negative(self) -> Self {
        self.exclusive_max(0.0)
    }

    /// Shorthand for `.max(0.0)`.
    #[must_use]
    pub fn nonpositive(self) -> Self {
        self.max(0.0)
    }

    /// Wrap this type in `anyOf` with `null`.
    #[must_use]
    pub fn optional(mut self) -> Self {
        let inner = self.build_schema();
        self.schema.clear();
        let mut null_schema = serde_json::Map::default();
        null_schema.insert("type".into(), Value::String("null".into()));
        self.schema.insert(
            "anyOf".into(),
            Value::Array(vec![inner, Value::Object(null_schema)]),
        );
        self
    }

    /// Change `"type": "number"` → `"type": ["number", "null"]`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        if let Some(Value::String(t)) = self.schema.get("type").cloned() {
            self.schema.insert(
                "type".into(),
                Value::Array(vec![Value::String(t), Value::String("null".into())]),
            );
        }
        self
    }

    /// Add a description to the schema (`"description"`).
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema
            .insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Add a default value (`"default"`).
    #[must_use]
    pub fn default(mut self, value: Value) -> Self {
        self.schema.insert("default".into(), value);
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

// ── Boolean Schema ────────────────────────────────────────────

/// Builder for JSON Schema boolean type.
///
/// WHY: Boolean schemas exist for completeness in the type hierarchy,
/// though JSON Schema has no boolean-specific constraints.
#[derive(Clone)]
pub struct BooleanSchema {
    schema: BTreeMap<String, Value>,
}

impl BooleanSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("boolean".into()));
        Self { schema }
    }

    /// Add a description to the schema (`"description"`).
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema
            .insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Add a default value (`"default"`).
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
        self.schema.insert(
            "anyOf".into(),
            Value::Array(vec![inner, Value::Object(null_schema)]),
        );
        self
    }

    /// Change `"type": "boolean"` → `"type": ["boolean", "null"]`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        if let Some(Value::String(t)) = self.schema.get("type").cloned() {
            self.schema.insert(
                "type".into(),
                Value::Array(vec![Value::String(t), Value::String("null".into())]),
            );
        }
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

// ── Null Schema ────────────────────────────────────────────────

/// Builder for JSON Schema null type.
///
/// WHY: Null schemas are useful in union types and optional fields.
#[derive(Clone)]
pub struct NullSchema {
    schema: BTreeMap<String, Value>,
}

impl NullSchema {
    fn new() -> Self {
        let mut schema = BTreeMap::new();
        schema.insert("type".into(), Value::String("null".into()));
        Self { schema }
    }

    /// Add a description to the schema (`"description"`).
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.schema
            .insert("description".into(), Value::String(desc.into()));
        self
    }

    /// Build and return the raw JSON Schema document.
    #[must_use]
    pub fn build_schema(&self) -> Value {
        Value::Object(
            self.schema
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Build the schema and return `ValidationOptions` with it embedded.
    #[must_use]
    pub fn build(self) -> ValidationOptions {
        let mut options = ValidationOptions::new();
        options.schema = Some(self.build_schema());
        options
    }
}

// ── Constructors ──────────────────────────────────────────────

/// Create a string schema builder.
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::string().min_len(1).max_len(255).email().build_schema();
/// ```
#[must_use]
pub fn string() -> StringSchema {
    StringSchema::new()
}

/// Create an integer schema builder.
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::integer().min(0).max(150).build_schema();
/// ```
#[must_use]
pub fn integer() -> IntegerSchema {
    IntegerSchema::new()
}

/// Create a number schema builder (integer or float).
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::number().min(0.0).max(100.0).build_schema();
/// ```
#[must_use]
pub fn number() -> NumberSchema {
    NumberSchema::new()
}

/// Create a boolean schema builder.
#[must_use]
pub fn boolean() -> BooleanSchema {
    BooleanSchema::new()
}

/// Create a null schema builder.
#[must_use]
pub fn null() -> NullSchema {
    NullSchema::new()
}

// ── From<Value> for all builders ──────────────────────────────────────

impl From<StringSchema> for Value {
    fn from(builder: StringSchema) -> Self {
        builder.build_schema()
    }
}

impl From<IntegerSchema> for Value {
    fn from(builder: IntegerSchema) -> Self {
        builder.build_schema()
    }
}

impl From<NumberSchema> for Value {
    fn from(builder: NumberSchema) -> Self {
        builder.build_schema()
    }
}

impl From<BooleanSchema> for Value {
    fn from(builder: BooleanSchema) -> Self {
        builder.build_schema()
    }
}

impl From<NullSchema> for Value {
    fn from(builder: NullSchema) -> Self {
        builder.build_schema()
    }
}
