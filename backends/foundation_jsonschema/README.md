# foundation_jsonschema

A self-contained, `no_std`-compatible JSON Schema validation library for Rust.
Supports Draft 4, 6, 7, 2019-09, and 2020-12 with a fluent scheme builder API.

## Features

- **Five draft versions** — Draft 4, 6, 7, 2019-09, 2020-12 with automatic detection
- **Fluent builder API** — type-safe `scheme::*` builders that produce valid schemas
- **Format validation** — email, uri, uuid, date-time, hostname, ipv4, ipv6, base64
- **Compound schemas** — `anyOf`, `oneOf`, `allOf`, `not`, `if/then/else`
- **Custom keywords** — register your own validation logic
- **Custom formats** — override or extend the built-in format checkers
- **No external dependencies** — all meta-schemas resolved in-memory
- **`no_std` compatible** — works with `alloc`, regex via `fancy-regex` feature

## Quick Start

```rust
use foundation_jsonschema::{validator_for, Draft};
use serde_json::json;

// Validate a schema directly
let schema = json!({
    "type": "object",
    "required": ["name", "age"],
    "properties": {
        "name": {"type": "string", "minLength": 1},
        "age": {"type": "integer", "minimum": 0, "maximum": 150}
    }
});

let validator = validator_for(&schema).unwrap();
assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
assert!(!validator.is_valid(&json!({"name": "Alice"}))); // missing required
```

## The Scheme Builder API

The `scheme` module provides type-safe, fluent builders for constructing JSON
Schema documents. Each builder only exposes methods that make sense for its
type, catching configuration errors at compile time rather than runtime.

### Primitives

**Strings** with length, pattern, and format constraints:

```rust
use foundation_jsonschema::scheme;
use serde_json::json;

let validator = scheme::string()
    .min_len(3)
    .max_len(32)
    .pattern(r"^[a-zA-Z0-9_]+$")
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!("darkvo1d")));
assert!(!validator.is_valid(&json!("ab")));
```

**Format shorthands** for common string patterns:

```rust
let validator = scheme::string()
    .email()
    .build()
    .assert_format(true)  // enforce format validation
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!("user@example.com")));
assert!(!validator.is_valid(&json!("not-an-email")));
```

Available format shorthands: `.email()`, `.uri()`, `.uuid()`, `.date_time()`,
`.hostname()`, `.ipv4()`, `.ipv6()`, `.base64()`.

**Integers** with range and divisibility:

```rust
let validator = scheme::integer()
    .min(1)
    .max(65535)
    .description("Network port")
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!(8080)));
assert!(!validator.is_valid(&json!(0)));
assert!(!validator.is_valid(&json!(70000)));
```

Convenience shorthands: `.positive()`, `.nonnegative()`, `.negative()`,
`.nonpositive()`, `.multiple_of(5)`, `.exclusive_min(0)`, `.exclusive_max(100)`.

**Numbers** (floats):

```rust
let validator = scheme::number()
    .min(0.0)
    .max(100.0)
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!(50.5)));
assert!(!validator.is_valid(&json!(-1.0)));
assert!(!validator.is_valid(&json!(101.0)));
```

**Booleans** and **null**:

```rust
let validator = scheme::boolean()
    .default(json!(true))
    .build()
    .compile()
    .unwrap();

// Null schema — matches only null
let null_validator = scheme::null()
    .description("Explicitly null")
    .build()
    .compile()
    .unwrap();
```

### Objects

**Required and optional properties**:

```rust
let validator = scheme::object()
    .required("username", scheme::string().min_len(3).max_len(32))
    .required("email", scheme::string().email())
    .required("age", scheme::integer().min(0).max(150))
    .optional("bio", scheme::string().max_len(500))
    .build()
    .assert_format(true)
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!({
    "username": "darkvo1d",
    "email": "darkvoid@example.com",
    "age": 28
})));

assert!(validator.is_valid(&json!({
    "username": "darkvo1d",
    "email": "darkvoid@example.com",
    "age": 28,
    "bio": "A short bio"
})));
```

**Strict mode** — reject unknown properties:

```rust
let validator = scheme::object()
    .required("name", scheme::string())
    .strict()  // additionalProperties: false
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!({"name": "Alice"})));
assert!(!validator.is_valid(&json!({"name": "Alice", "extra": true})));
```

**Loose mode** — allow any extra properties (default, but explicit):

```rust
let schema = scheme::object()
    .required("id", scheme::integer())
    .loose()  // additionalProperties: true
    .build_schema();
```

**Catch-all schema** — extra properties must match a schema:

```rust
let schema = scheme::object()
    .required("name", scheme::string())
    .catchall(scheme::string())  // all extra fields must be strings
    .build_schema();
```

**Pattern properties** — validate properties matching a regex:

```rust
let schema = scheme::object()
    .pattern_property(r"^x-", scheme::string())
    .build_schema();
// Properties like "x-custom", "x-header" must be strings
```

**Nested objects**:

```rust
let validator = scheme::object()
    .required("name", scheme::string())
    .required("address", scheme::object()
        .required("street", scheme::string())
        .required("city", scheme::string())
        .required("zip", scheme::string().len(5).pattern(r"^\d{5}$"))
        .strict()
    )
    .build()
    .compile()
    .unwrap();
```

### Arrays

**Items with constraints**:

```rust
let validator = scheme::array()
    .items(scheme::string().min_len(1).max_len(20))
    .min_items(1)
    .max_items(5)
    .unique()
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!(["rust", "json", "schema"])));
assert!(!validator.is_valid(&json!([])));              // min_items: 1
assert!(!validator.is_valid(&json!(["rust", "rust"]))); // unique
assert!(!validator.is_valid(&json!(["a", "b", "c", "d", "e", "f"]))); // max_items: 5
```

**Tuple validation** (Draft 2020-12 prefixItems):

```rust
let validator = scheme::array()
    .prefix_items(vec![
        scheme::number(),  // latitude
        scheme::number(),  // longitude
        scheme::number(),  // altitude
    ])
    .len(3)
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!([1.0, 2.0, 3.0])));
assert!(!validator.is_valid(&json!([1.0, 2.0])));
assert!(!validator.is_valid(&json!([1.0, 2.0, 3.0, 4.0])));
```

**Additional items** for tuple + rest patterns:

```rust
let schema = scheme::array()
    .prefix_items(vec![scheme::string(), scheme::integer()])
    .additional_items(scheme::boolean())
    .build_schema();
// First item: string, second: integer, rest: boolean
```

**Contains**, **min_contains**, **max_contains** (Draft 6+):

```rust
let schema = scheme::array()
    .contains(scheme::integer().min(0))
    .min_contains(2)
    .max_contains(5)
    .build_schema();
```

### Nullable and Optional

**Nullable** — accepts the type or `null`:

```rust
let validator = scheme::object()
    .required("name", scheme::string().nullable())
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!({"name": "Alice"})));
assert!(validator.is_valid(&json!({"name": null})));
assert!(!validator.is_valid(&json!({"name": 42})));
```

**Optional** — wraps in `anyOf` with `null`:

```rust
let validator = scheme::object()
    .required("name", scheme::string())
    .required("middle_name", scheme::string().optional())
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!({"name": "Alice", "middle_name": "Marie"})));
assert!(validator.is_valid(&json!({"name": "Alice", "middle_name": null})));
```

Note: `.optional()` on a field schema (used with `required()`) makes the field
value nullable. Use `.optional(name, schema)` on the object builder to add a
property that is not in the `required` array.

### Enum and Literal Values

**Enum constraint** on strings:

```rust
let validator = scheme::object()
    .required("task", scheme::string())
    .required("priority", scheme::string()
        .enum_values(vec!["low".into(), "medium".into(), "high".into()])
    )
    .build()
    .compile()
    .unwrap();

assert!(validator.is_valid(&json!({"task": "deploy", "priority": "high"})));
assert!(!validator.is_valid(&json!({"task": "deploy", "priority": "critical"})));
```

**Standalone enum**:

```rust
use foundation_jsonschema::{scheme, validator_for};
use serde_json::json;

let schema = scheme::r#enum(vec![json!("red"), json!("green"), json!("blue")]);
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!("red")));
assert!(!v.is_valid(&json!("purple")));
```

**Literal / const**:

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::literal(json!("exact"));
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!("exact")));
assert!(!v.is_valid(&json!("not exact")));
```

### Compound Schemas

**anyOf** — must match at least one:

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::any_of(vec![
    scheme::string().build_schema(),
    scheme::integer().build_schema(),
]);
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!("hello")));
assert!(v.is_valid(&json!(42)));
assert!(!v.is_valid(&json!(true)));
```

**oneOf** — must match exactly one:

```rust
let schema = scheme::one_of(vec![
    scheme::string().build_schema(),
    scheme::integer().build_schema(),
]);
```

**allOf** — must match all (intersection):

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::all_of(vec![
    scheme::object().required("a", scheme::string()).build_schema(),
    scheme::object().required("b", scheme::integer()).build_schema(),
]);
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!({"a": "hi", "b": 1})));
assert!(!v.is_valid(&json!({"a": "hi"})));
```

**not** — inverted validation:

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::not(scheme::null());
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!("hello")));
assert!(!v.is_valid(&json!(null)));
```

**if/then** — conditional validation:

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::if_then(
    scheme::object().required("role", scheme::literal(json!("admin"))).build_schema(),
    scheme::object().required("permissions", scheme::array().items(scheme::string())).build_schema(),
);
let v = validator_for(&schema).unwrap();
// Non-admin: no permissions required
assert!(v.is_valid(&json!({"role": "user"})));
// Admin with permissions: valid
assert!(v.is_valid(&json!({"role": "admin", "permissions": ["read", "write"]})));
```

**if/then/else** — discriminated union:

```rust
use foundation_jsonschema::validator_for;
use serde_json::json;

let schema = scheme::if_then_else(
    scheme::object()
        .required("kind", scheme::literal(json!("circle")))
        .build_schema(),
    scheme::object()
        .required("kind", scheme::literal(json!("circle")))
        .required("radius", scheme::number().positive())
        .build_schema(),
    scheme::object()
        .required("kind", scheme::literal(json!("rect")))
        .required("width", scheme::number().positive())
        .required("height", scheme::number().positive())
        .build_schema(),
);
let v = validator_for(&schema).unwrap();
assert!(v.is_valid(&json!({"kind": "circle", "radius": 5.0})));
assert!(v.is_valid(&json!({"kind": "rect", "width": 10.0, "height": 20.0})));
```

### References

```rust
// Internal reference
let schema = scheme::ref_("#/$defs/user");

// Dynamic reference (Draft 2020-12)
let schema = scheme::dynamic_ref("#node");
```

### Schema Inspection

Build a schema without compiling it, for inspection or serialization:

```rust
use serde_json::json;

let schema = scheme::object()
    .required("name", scheme::string())
    .required("age", scheme::integer().min(0))
    .optional("email", scheme::string().email())
    .strict()
    .build_schema();

// schema is a serde_json::Value — inspect, serialize, or pass to other tools
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

### build_schema vs build

- **`build_schema(&self)`** — returns a `serde_json::Value`. Non-consuming,
  can be called multiple times. Useful for inspection.
- **`build(self)`** — returns a `ValidationOptions`. Consuming, chains into
  `.compile()` for the full build-compile-validate pipeline.

```rust
use serde_json::json;

let builder = scheme::string().min_len(1);

// Inspect multiple times
let s1 = builder.build_schema();
let s2 = builder.build_schema();

// Compile and validate (consumes builder)
let validator = builder.build().compile().unwrap();
```

## ValidationOptions

Fine-grained control over schema compilation:

```rust
use foundation_jsonschema::{scheme, Draft};

let validator = scheme::object()
    .required("email", scheme::string().email())
    .build()
    .with_draft(Draft::Draft7)
    .assert_format(true)
    .compile()
    .unwrap();
```

### Schema Accessors

When using `build()`, the schema is embedded in `ValidationOptions`:

```rust
use serde_json::json;

let opts = scheme::string().min_len(1).max_len(255).build();

// Reference without consuming
let schema_ref = opts.schema();
println!("{}", schema_ref);

// Clone the schema
let cloned = opts.clone_schema();

// Take ownership
let owned = opts.into_schema();
```

### Custom Draft Selection

```rust
use foundation_jsonschema::{scheme, Draft};

let validator = scheme::object()
    .required("value", scheme::number())
    .build()
    .with_draft(Draft::Draft202012)
    .compile()
    .unwrap();
```

### Custom Resolvers

Provide your own resource resolver for external references:

```rust
use foundation_jsonschema::InMemoryFetcher;
use foundation_jsonschema::scheme;

let resolver = InMemoryFetcher::builtin();
let validator = scheme::object()
    .build()
    .with_resolver(resolver)
    .compile()
    .unwrap();
```

## Direct Validation

For cases where you already have a `serde_json::Value` schema:

```rust
use foundation_jsonschema::{validate, is_valid, validator_for};
use serde_json::json;

let schema = json!({"type": "string", "minLength": 1});

// Quick boolean check
assert!(is_valid(&schema, &json!("hello")));

// Full validation with error details
let result = validate(&schema, &json!(""));
assert!(result.is_err());

// Reusable validator for repeated checks
let validator = validator_for(&schema).unwrap();
assert!(validator.is_valid(&json!("test")));
```

## no_std Support

Enable the `fancy-regex` feature to use `fancy-regex` instead of the `regex`
crate, allowing pattern validation without the standard library:

```toml
[dependencies]
foundation_jsonschema = { version = "0.0.1", default-features = false, features = ["fancy-regex"] }
```
