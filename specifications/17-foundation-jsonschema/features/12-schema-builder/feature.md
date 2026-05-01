# Feature 12: Schema Builder API

---

## What we're building

A fluent, type-safe schema builder module (`foundation_jsonschema::scheme`) that lets users construct JSON Schema documents using Rust methods instead of writing JSON by hand.

**Zod analogy:** `z.string().min(1).email()` in Zod becomes `scheme::string().min_len(1).email()` in Rust.

**Key difference from Zod:** Zod schemas *are* validators. Our builders produce a `serde_json::Value` (a standard JSON Schema document) which is then compiled and validated by the existing `ValidationOptions::build()` pipeline. The builder is a schema construction API, not a replacement for the compiler.

---

## Motivation

Currently, constructing a schema requires verbose `serde_json::json!` macros:

```rust
let schema = json!({
    "type": "object",
    "required": ["name", "age"],
    "properties": {
        "name": {"type": "string", "minLength": 1, "maxLength": 255},
        "age": {"type": "integer", "minimum": 0, "maximum": 150},
        "email": {"type": "string", "format": "email"}
    },
    "additionalProperties": false
});
```

With this builder:

```rust
// Path A: direct pipeline — build schema into ValidationOptions, compile immediately
let validator = scheme::object()
    .required("name", scheme::string().min_len(1).max_len(255))
    .required("age", scheme::integer().min(0).max(150))
    .required("email", scheme::string().format("email"))
    .strict()
    .build()                                    // → ValidationOptions (schema embedded)
    .compile()                                  // → Validator
    .unwrap();

// Path B: extract raw JSON Schema for inspection or storage
let schema_value = scheme::object()
    .required("name", scheme::string())
    .build_schema();                            // → serde_json::Value
```

The builder version is:
- **Type-safe** — you can't set `.email()` on an integer schema
- **Autocomplete-friendly** — every method is a real method on a concrete type
- **Composable** — schemas nest naturally via `.build()` calls
- **No JSON syntax to juggle** — no macros, no braces, no string interpolation issues
- **Pipeline-friendly** — `build()` returns `ValidationOptions` so you chain straight to `.compile()`

---

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Features 0, 1, 2, 3, 4, 8

---

## Design Principles

1. **Output is JSON Schema** — Every builder produces a `serde_json::Value` that is a valid JSON Schema document. No new intermediate representation. The existing compiler pipeline is the sole validator.

2. **`build()` returns `ValidationOptions`** — The primary path is `builder.build()` → `ValidationOptions`, which owns the generated schema. This lets users chain `builder.build().compile().unwrap()` without intermediate variables.

3. **`build_schema()` returns raw `serde_json::Value`** — The escape hatch for raw schema access. Returns the JSON Schema document directly for inspection, serialization, or manual compilation.

4. **`ValidationOptions` owns the schema** — The struct stores the generated `serde_json::Value`. Accessors:
   - `schema()` → `&serde_json::Value` (reference to stored schema)
   - `into_schema(self)` → `serde_json::Value` (consume and take ownership)
   - `clone_schema()` → `serde_json::Value` (clone for independent use)

5. **`compile()` replaces `build()` on `ValidationOptions`** — The existing `ValidationOptions::build(&self, schema)` becomes `ValidationOptions::compile(self)` (no argument needed — schema is already owned).

6. **Consuming builders** — Each method consumes `self` and returns the next builder. This is the Rust-idiomatic equivalent of Zod's immutable chain (where every method returns a clone). No `Arc` or shared state needed.

7. **Type-specific traits** — A `StringSchema` builder has `.email()`, `.min_len()`, `.max_len()` but not `.min()`. An `IntegerSchema` builder has `.min()`, `.max()`, `.multiple_of()` but not `.format()`. This matches Zod's trait hierarchy (`ZodString`, `ZodNumber`, `ZodObject`, etc.).

8. **Universal modifiers on all types** — `.optional()`, `.nullable()`, `.description()`, `.default()` are methods on a shared `Schema` trait that all type-specific builders implement. In Rust, this maps to adding these methods directly on each concrete builder (no trait object overhead).

9. **Composability via nested builders** — Nested schema values accept `impl Into<serde_json::Value>`, so you can pass `scheme::string().build_schema()` or `scheme::string()` (via `From` impl).

10. **no_std compatible** — The builder module uses `alloc::collections::BTreeMap` and `alloc::vec::Vec` internally, matching the existing crate's `#![no_std]` + `alloc` pattern.

---

## Module Structure

```
backends/foundation_jsonschema/src/scheme/
├── mod.rs          # SchemaBuilder trait, exports
├── primitives.rs   # string(), integer(), number(), boolean(), null()
├── object.rs       # object(), object_with()
├── array.rs        # array(), array_of()
├── compound.rs     # union(), intersection(), any_of(), one_of(), all_of(), not_()
├── literal.rs      # literal(), r#enum()
└── reference.rs    # ref_(), dynamic_ref()
```

```rust
pub mod scheme {
    pub use primitives::*;
    pub use object::*;
    pub use array::*;
    pub use compound::*;
    pub use literal::*;
    pub use reference::*;

    /// Internal trait for all schema builders.
    /// Provides the shared build() / build_schema() interface.
    pub trait SchemaBuilder {
        /// Build the schema and return ValidationOptions with the
        /// schema embedded. Chain with `.compile()` for direct validation.
        fn build(self) -> crate::ValidationOptions;

        /// Build and return the raw JSON Schema document.
        /// Use this when you need to inspect, store, or pass the
        /// schema to a different compilation path.
        fn build_schema(self) -> serde_json::Value;
    }
}
```

---

## API Specification

### 1. Primitive Type Constructors

Each constructor returns a concrete builder type with type-specific methods.

```rust
// ── string() ──────────────────────────────────────────────
pub fn string() -> StringSchema

pub struct StringSchema {
    properties: BTreeMap<String, serde_json::Value>,
    description: Option<String>,
}

impl StringSchema {
    fn min_len(self, n: usize) -> Self;        // → {"minLength": n}
    fn max_len(self, n: usize) -> Self;        // → {"maxLength": n}
    fn len(self, n: usize) -> Self;            // → {"minLength": n, "maxLength": n}
    fn pattern(self, re: &str) -> Self;        // → {"pattern": re}
    fn format(self, f: &str) -> Self;          // → {"format": f}
    fn email(self) -> Self;                    // → {"format": "email"}
    fn uri(self) -> Self;                      // → {"format": "uri"}
    fn uuid(self) -> Self;                     // → {"format": "uuid"}
    fn date_time(self) -> Self;                // → {"format": "date-time"}
    fn hostname(self) -> Self;                 // → {"format": "hostname"}
    fn ipv4(self) -> Self;                     // → {"format": "ipv4"}
    fn ipv6(self) -> Self;                     // → {"format": "ipv6"}
    fn base64(self) -> Self;                   // → {"format": "base64"}
    fn enum_values(self, vals: Vec<String>) -> Self;  // → {"enum": [...]}
}

// ── integer() ─────────────────────────────────────────────
pub fn integer() -> IntegerSchema

impl IntegerSchema {
    fn min(self, n: i64) -> Self;              // → {"type": "integer", "minimum": n}
    fn max(self, n: i64) -> Self;              // → {"maximum": n}
    fn exclusive_min(self, n: i64) -> Self;    // → {"exclusiveMinimum": n} (6+)
    fn exclusive_max(self, n: i64) -> Self;    // → {"exclusiveMaximum": n} (6+)
    fn multiple_of(self, n: i64) -> Self;      // → {"multipleOf": n}
    fn positive(self) -> Self;                 // → {"exclusiveMinimum": 0} (6+)
    fn nonnegative(self) -> Self;              // → {"minimum": 0} (6+)
    fn negative(self) -> Self;                 // → {"exclusiveMaximum": 0} (6+)
    fn nonpositive(self) -> Self;              // → {"maximum": 0} (6+)
    fn enum_values(self, vals: Vec<i64>) -> Self;
}

// ── number() ──────────────────────────────────────────────
pub fn number() -> NumberSchema
// Same methods as integer() but uses f64 for min/max/multiple_of

// ── boolean() ─────────────────────────────────────────────
pub fn boolean() -> BooleanSchema
// Minimal: no type-specific methods (matches JSON Schema spec)

// ── null() ────────────────────────────────────────────────
pub fn null() -> NullSchema
// Minimal: {"type": "null"}
```

### 2. Object Builder

```rust
pub fn object() -> ObjectSchema

impl ObjectSchema {
    fn required(self, name: &str, schema: impl Into<serde_json::Value>) -> Self;
    fn optional(self, name: &str, schema: impl Into<serde_json::Value>) -> Self;
    fn properties(self, map: BTreeMap<String, serde_json::Value>) -> Self;

    // Key behavior
    fn strict(self) -> Self;                   // → {"additionalProperties": false}
    fn loose(self) -> Self;                    // → {"additionalProperties": true}
    fn catchall(self, schema: impl Into<serde_json::Value>) -> Self;
                                               // → {"additionalProperties": <schema>}

    // Pattern properties (advanced)
    fn pattern_property(self, re: &str, schema: impl Into<serde_json::Value>) -> Self;

    // Selectors
    fn min_properties(self, n: usize) -> Self;
    fn max_properties(self, n: usize) -> Self;
}
```

**Usage:**

```rust
// Path A: direct pipeline — build schema → ValidationOptions → compile
let validator = scheme::object()
    .required("username", scheme::string())
    .required("xp", scheme::integer().min(0))
    .optional("title", scheme::string().max_len(50))
    .strict()
    .build()                                    // → ValidationOptions
    .compile()                                  // → Result<Validator, ValidationError>
    .unwrap();

// Path B: extract raw JSON Schema
let player = scheme::object()
    .required("username", scheme::string())
    .required("xp", scheme::integer().min(0))
    .optional("title", scheme::string().max_len(50))
    .strict()
    .build_schema();                            // → serde_json::Value
// → {"type": "object", "required": ["username","xp"],
//    "properties": {"username":{"type":"string"},
//                   "xp":{"type":"integer","minimum":0},
//                   "title":{"type":"string","maxLength":50}},
//    "additionalProperties": false}
```

### 3. Array Builder

```rust
pub fn array() -> ArraySchema
pub fn array_of(items: impl Into<serde_json::Value>) -> ArraySchema

impl ArraySchema {
    fn items(self, schema: impl Into<serde_json::Value>) -> Self;
    fn prefix_items(self, schemas: Vec<serde_json::Value>) -> Self;   // Draft 2020-12
    fn additional_items(self, schema: impl Into<serde_json::Value>) -> Self;  // Draft 4-7

    fn min_items(self, n: usize) -> Self;
    fn max_items(self, n: usize) -> Self;
    fn len(self, n: usize) -> Self;
    fn unique(self) -> Self;                    // → {"uniqueItems": true}
    fn nonempty(self) -> Self;                  // → {"minItems": 1}
    fn contains(self, schema: impl Into<serde_json::Value>) -> Self;  // Draft 6+
    fn min_contains(self, n: u64) -> Self;      // Draft 2019-09+
    fn max_contains(self, n: u64) -> Self;      // Draft 2019-09+
}
```

**Usage:**

```rust
// Array of strings with constraints
let tag_list = scheme::array()
    .items(scheme::string().min_len(1).max_len(30).build_schema())
    .min_items(1)
    .max_items(10)
    .unique()
    .build_schema();

// Tuple type (fixed positions + optional additional)
let coordinate = scheme::array()
    .prefix_items(vec![
        scheme::number().build_schema(),
        scheme::number().build_schema(),
        scheme::number().build_schema(),
    ])
    .additional_items(scheme::boolean().build_schema())
    .build_schema();
```

### 4. Compound Types

```rust
// ── Union (anyOf) ─────────────────────────────────────────
pub fn union(schemas: Vec<serde_json::Value>) -> serde_json::Value
pub fn any_of(schemas: Vec<serde_json::Value>) -> serde_json::Value  // alias

// ── OneOf ─────────────────────────────────────────────────
pub fn one_of(schemas: Vec<serde_json::Value>) -> serde_json::Value

// ── AllOf (intersection) ──────────────────────────────────
pub fn all_of(schemas: Vec<serde_json::Value>) -> serde_json::Value
pub fn intersection(left: serde_json::Value, right: serde_json::Value) -> serde_json::Value

// ── Not ───────────────────────────────────────────────────
pub fn not(schema: impl Into<serde_json::Value>) -> serde_json::Value

// ── If/Then/Else ──────────────────────────────────────────
pub fn if_then(if_schema: serde_json::Value, then_schema: serde_json::Value) -> serde_json::Value
pub fn if_then_else(
    if_schema: serde_json::Value,
    then_schema: serde_json::Value,
    else_schema: serde_json::Value,
) -> serde_json::Value
```

### 5. Literal and Enum

```rust
pub fn literal(value: serde_json::Value) -> serde_json::Value   // → {"const": value}
pub fn r#enum(values: Vec<serde_json::Value>) -> serde_json::Value // → {"enum": values}
```

### 6. References

```rust
pub fn ref_(path: &str) -> serde_json::Value        // → {"$ref": path}
pub fn dynamic_ref(path: &str) -> serde_json::Value // → {"$dynamicRef": path} (2020-12)
```

### 7. Universal Modifiers (on all builders)

Each concrete builder implements these common methods:

```rust
// On every *Schema struct:
fn optional(self) -> Self        // wraps in anyOf with null type
fn nullable(self) -> Self        // adds "null" to the type
fn description(self, desc: &str) -> Self
fn default(self, value: serde_json::Value) -> Self
```

**`optional()` implementation:**
```rust
fn optional(mut self) -> Self {
    let inner = self.build();
    self.properties.insert("anyOf".into(), json!([
        inner,
        {"type": "null"}
    ]));
    self
}
```

**`nullable()` implementation:**
```rust
fn nullable(mut self) -> Self {
    // Change "type": "string" → "type": ["string", "null"]
    if let Some(json::Value::String(t)) = self.properties.get("type") {
        self.properties.insert("type".into(), json!([t, "null"]));
    }
    self
}
```

---

## Integration with Existing Compiler

### New `ValidationOptions` methods

The existing `ValidationOptions` struct gains three new methods and one new field:

```rust
pub struct ValidationOptions {
    default_draft: Draft,
    resolver: InMemoryFetcher,
    assert_format: bool,
    custom_keywords: BTreeMap<String, Box<dyn KeywordFactory>>,
    custom_formats: BTreeMap<String, Box<dyn FormatChecker>>,
    schema: Option<Value>,           // NEW: owned generated schema
}

impl ValidationOptions {
    /// Compile the embedded schema with these options.
    ///
    /// Replaces `build(schema)` when the schema was already set via
    /// a builder's `.build()` method. Consumes self and returns a Validator.
    pub fn compile(self) -> Result<Validator, ValidationError> {
        let schema = self.schema.as_ref()
            .expect("schema must be set before compile()");
        // ... same logic as existing build() ...
    }

    /// Reference to the stored schema (for inspection/serialization).
    pub fn schema(&self) -> &Value {
        self.schema.as_ref().expect("schema must be set")
    }

    /// Consume and take ownership of the stored schema.
    pub fn into_schema(mut self) -> Value {
        self.schema.take().expect("schema must be set")
    }

    /// Clone the stored schema.
    pub fn clone_schema(&self) -> Value {
        self.schema.as_ref().expect("schema must be set").clone()
    }
}
```

**Note:** The existing `build(schema)` method remains unchanged. It is still callable for users who construct schemas manually (via `json!{}` or other means):
```rust
let validator = ValidationOptions::new()
    .with_draft(Draft::Draft202012)
    .build(&schema)   // existing API: build(schema: &Value)
    .unwrap();
```

### Complete usage examples

```rust
// ── Path A: Direct builder → ValidationOptions → Validator ──
let validator = scheme::object()
    .required("name", scheme::string().min_len(1))
    .required("age", scheme::integer().min(0).max(150))
    .strict()
    .build()                                    // → ValidationOptions (schema embedded)
    .compile()                                  // → Result<Validator, ValidationError>
    .unwrap();

assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
assert!(!validator.is_valid(&json!({"name": "", "age": 200})));

// ── Path B: Builder with custom format, then compile ──
let validator = scheme::object()
    .required("color", scheme::string().format("hex-color"))
    .build()                                    // → ValidationOptions
    .with_format("hex-color", Box::new(HexColorFormat))  // add custom format
    .compile()
    .unwrap();

// ── Path C: Extract raw JSON Schema for inspection ──
let schema_value = scheme::object()
    .required("name", scheme::string())
    .build_schema();                            // → serde_json::Value
// Can be serialized, logged, or passed to other JSON Schema tools

// ── Path D: Inspect schema stored in ValidationOptions ──
let options = scheme::object()
    .required("name", scheme::string())
    .build();                                   // → ValidationOptions
let schema_ref: &Value = options.schema();     // inspect without cloning
let schema_clone: Value = options.clone_schema();  // own a copy
let validator = options.compile().unwrap();    // compile and validate
```

This is a critical design decision: **the builder does not duplicate validation logic**. It only generates the JSON Schema document and wires it into `ValidationOptions`. All validation is handled by the existing, battle-tested compiler and validator modules.

---

## Architecture

### Module internals

Each builder struct is a thin wrapper around a `BTreeMap<String, serde_json::Value>` that accumulates JSON Schema properties:

```rust
pub struct StringSchema {
    schema: BTreeMap<String, serde_json::Value>,
}

impl StringSchema {
    pub fn build_schema(self) -> serde_json::Value {
        serde_json::Value::Object(self.schema.into_iter().collect())
    }
}
```

Type constructors initialize the map with `"type"`:

```rust
pub fn string() -> StringSchema {
    let mut schema = BTreeMap::new();
    schema.insert("type".into(), serde_json::Value::String("string".into()));
    StringSchema { schema }
}
```

Type-specific methods mutate the map and return `self`:

```rust
impl StringSchema {
    pub fn min_len(mut self, n: usize) -> Self {
        self.schema.insert("minLength".into(), serde_json::Value::Number(n.into()));
        self
    }
}
```

### Builder → ValidationOptions wiring

Each builder implements the `SchemaBuilder` trait to produce `ValidationOptions`:

```rust
impl SchemaBuilder for StringSchema {
    fn build(self) -> ValidationOptions {
        let schema = self.build_schema();
        let mut options = ValidationOptions::new();
        options.schema = Some(schema);
        options
    }

    fn build_schema(self) -> serde_json::Value {
        // same as the inherent method above
    }
}
```

### `build(schema)` vs `compile()` — two compile paths

`ValidationOptions` exposes **two methods** to produce a `Validator`. Both call the same compilation logic. The only difference is where the schema comes from.

| Method | Source of schema | When to use |
|--------|------------------|-------------|
| `build(schema: &Value)` | Caller provides schema | Manual schemas (`json!{}`, file load, other builders) |
| `compile()` | Uses `self.schema` (set by builder) | Fluent builder path (`scheme::object()...`) |

```rust
impl ValidationOptions {
    // EXISTING — unchanged signature, unchanged behavior
    pub fn build(self, schema: &Value) -> Result<Validator, ValidationError> {
        // same implementation as before (no modifications needed)
    }

    // NEW — reads schema from the embedded Option<Value>
    pub fn compile(self) -> Result<Validator, ValidationError> {
        let schema = self.schema.as_ref()
            .expect("schema must be set before compile()");
        // ... same compilation logic as build() ...
    }
}
```

Both paths support the same configuration methods (`assert_format`, `with_format`, `with_keyword`, etc.). These methods consume and return `Self`, so they can be chained between `builder.build()` and `.compile()`:

```rust
// Builder path with custom formats — all config in one chain
let validator = scheme::object()
    .required("color", scheme::string().format("hex-color"))
    .build()                                    // → ValidationOptions (schema embedded)
    .assert_format(true)                        // existing builder method
    .with_format("hex-color", Box::new(HexColorFormat))  // existing builder method
    .compile()
    .unwrap();

// Manual path — same config options
let validator = ValidationOptions::new()
    .with_draft(Draft::Draft202012)
    .assert_format(true)
    .with_format("hex-color", Box::new(HexColorFormat))
    .build(&schema)                             // existing build(schema)
    .unwrap();
```

### Why BTreeMap and not HashMap

The existing crate uses `BTreeMap` throughout for deterministic iteration order. The builder follows the same pattern — important for consistent JSON output and reproducible test comparisons.

### Why `Into<serde_json::Value>` for nested schemas

Accepting `impl Into<serde_json::Value>` lets callers pass:
- A builder's `.build_schema()` output: `.items(scheme::string().build_schema())`
- A raw `serde_json::Value`: `.items(json!({"type": "string"}))`
- The builder type itself (via `From<StringSchema> for Value` impl)

This avoids forcing one specific pattern while keeping the most ergonomic path as the default.

---

## Tasks

- [ ] Task 1: Modify `ValidationOptions` — add `schema: Option<Value>` field, `.compile()`, `.schema()`, `.into_schema()`, `.clone_schema()`
- [ ] Task 2: Define `scheme/mod.rs` — `SchemaBuilder` trait, module structure, exports
- [ ] Task 3: Implement `primitives.rs` — `string()`, `integer()`, `number()`, `boolean()`, `null()` with all type-specific methods and `SchemaBuilder` impl
- [ ] Task 4: Implement `object.rs` — `object()` with `.required()`, `.optional()`, `.strict()`, `.loose()`, `.catchall()` and `SchemaBuilder` impl
- [ ] Task 5: Implement `array.rs` — `array()`, `array_of()` with `.items()`, `.min_items()`, `.max_items()`, `.unique()`, `.prefix_items()` and `SchemaBuilder` impl
- [ ] Task 6: Implement `compound.rs` — `union()`, `any_of()`, `one_of()`, `all_of()`, `intersection()`, `not_()`, `if_then()`, `if_then_else()`
- [ ] Task 7: Implement `literal.rs` — `literal()`, `r#enum()`
- [ ] Task 8: Implement `reference.rs` — `ref_()`, `dynamic_ref()`
- [ ] Task 9: Wire `scheme` module into `lib.rs` (public export)
- [ ] Task 10: Write unit tests for all primitives (string, integer, number, boolean, null)
- [ ] Task 11: Write unit tests for object builder (required/optional/strict/loose/catchall)
- [ ] Task 12: Write unit tests for array builder (items, prefix_items, min/max, unique, contains)
- [ ] Task 13: Write unit tests for compound types (union, one_of, all_of, intersection, not, if_then_else)
- [ ] Task 14: Write integration tests — build complex schemas, compile via `ValidationOptions::compile()`, validate instances
- [ ] Task 15: Write tests for ValidationOptions schema accessors (`.schema()`, `.into_schema()`, `.clone_schema()`)
- [ ] Task 16: Write examples demonstrating common patterns (user schema, coordinate tuple, discriminated union)

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Builder output produces valid JSON Schema documents (verifiable via `is_schema_valid()`)
- [ ] `ValidationOptions::compile()` works correctly with builder-embedded schema
- [ ] `ValidationOptions::schema()` returns correct reference
- [ ] `ValidationOptions::into_schema()` consumes and returns owned schema
- [ ] `ValidationOptions::clone_schema()` returns correct clone
- [ ] Existing `ValidationOptions::build(schema)` API remains unchanged and functional
- [ ] Builder-produced schemas compile and validate correctly via existing `ValidationOptions` pipeline
- [ ] Zero clippy warnings
- [ ] `no_std + alloc` compatible (no `std::collections::HashMap`, no `std::path`, etc.)

---

_Created: 2026-05-01_
