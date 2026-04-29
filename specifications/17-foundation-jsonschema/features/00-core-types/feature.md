---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/00-core-types"
this_file: "specifications/17-foundation-jsonschema/features/00-core-types/feature.md"

status: complete
priority: high
created: 2026-04-28

depends_on: []

tasks:
  completed: 28
  uncompleted: 0
  total: 28
  completion_percentage: 100
---

# Feature 0: Core Types

## Overview

Define the foundational types that every other feature depends on. These types form the vocabulary of the entire `foundation_jsonschema` crate: JSON type representation, location/path tracking, the `Draft` enum for schema version identification, and the `JsonResolver` trait for pluggable external schema resolution.

This feature also creates the crate skeleton (`Cargo.toml`, `lib.rs`) and establishes the `no_std + alloc` / `std` dual-mode foundation.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Understood all coding standards and requirements
- [ ] Noted tools and formatters required (rustfmt, clippy)

---

## Requirements

1. **Crate skeleton** — Create `backends/foundation_jsonschema/` with Cargo.toml, lib.rs, and the `#![no_std]` / `std` feature gate pattern used by `foundation_nostd`.

2. **JsonType enum** — Represent the 7 JSON Schema types: null, boolean, integer, number, string, array, object. Must support comparison with `serde_json::Value`.

3. **JsonTypeSet bitset** — A compact bitset that holds a set of `JsonType` values. Used by the `type` keyword validator which accepts either a single type or an array of types. Must support O(1) membership testing and iteration.

4. **Location and path types** — Track the position within a JSON instance (for error reporting) and within the schema document. Must support both eager (`Location`) and lazy (`LazyLocation`) construction to avoid allocations on the happy path.

5. **Draft enum** — Identify which JSON Schema draft a schema conforms to. Must support detection from the `$schema` keyword and explicit construction.

6. **JsonResolver trait** — The pluggable interface for resolving external `$ref` URIs. Uses `ErrorTrace<ResolveError>` from `foundation_errstacks` for structured error handling. Plus `NoopResolver` (always fails) and `MapResolver` (pre-loaded map).

7. **no_std compatibility** — All types in this feature must work in `no_std + alloc` environments. Use `alloc::string::String`, `alloc::vec::Vec`, `alloc::boxed::Box` etc. when `std` is not available.

## Architecture (COMPREHENSIVE)

### Technical Approach

- **Bitset for type sets**: Use a `u8` internally with one bit per JSON type (7 types fit in 7 bits). This is the approach used in the reference project — it enables O(1) membership testing without heap allocation.
- **Lazy path construction**: `LazyLocation` is a linked-list-style structure that defers `String` allocation. Only when an error actually occurs do we materialize the full path. This avoids per-validation allocation overhead.
- **Trait object erasure for resolver**: `JsonResolver` is object-safe so it can be stored as `Box<dyn JsonResolver>` in the registry.

### Component Structure

```
backends/foundation_jsonschema/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Crate root, feature gates, public re-exports
    ├── types.rs                # JsonType, JsonTypeSet
    ├── paths.rs                # Location, LazyLocation, LocationSegment
    ├── draft.rs                # Draft enum
    └── resolver_trait.rs       # JsonResolver, NoopResolver, MapResolver
```

### Component Details

1. **`lib.rs`**
   - **Purpose**: Crate root. Declares `#![no_std]` when `std` feature is off. Declares `extern crate alloc;`. Re-exports all public types.
   - **Pattern**:
     ```rust
     #![cfg_attr(not(feature = "std"), no_std)]
     
     #[cfg(not(feature = "std"))]
     extern crate alloc;
     
     // Conditional imports for alloc types when no_std
     #[cfg(not(feature = "std"))]
     use alloc::{boxed::Box, string::String, vec::Vec, format};
     ```

2. **`types.rs` — JsonType and JsonTypeSet**
   - **Purpose**: Map between JSON Schema type names and `serde_json::Value` variants.
   - **Key Types**:
     ```rust
     /// The 7 primitive types defined by JSON Schema.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
     pub enum JsonType {
         Null,
         Boolean,
         Integer,
         Number,
         String,
         Array,
         Object,
     }
     
     impl JsonType {
         /// Determine the JSON Schema type of a serde_json::Value.
         ///
         /// Note: JSON has no "integer" type — a number is classified as Integer
         /// if it has no fractional part (e.g., 1.0 is Integer, 1.5 is Number).
         pub fn of(value: &serde_json::Value) -> Self { ... }
         
         /// Return the JSON Schema string name for this type.
         pub fn as_str(&self) -> &'static str { ... }
     }
     
     /// Compact bitset holding a set of JsonType values.
     ///
     /// Internally a u8 with one bit per type. Supports O(1) membership test.
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
     pub struct JsonTypeSet(u8);
     
     impl JsonTypeSet {
         pub fn new() -> Self { ... }
         pub fn with(ty: JsonType) -> Self { ... }
         pub fn insert(&mut self, ty: JsonType) { ... }
         pub fn contains(&self, ty: JsonType) -> bool { ... }
         pub fn is_empty(&self) -> bool { ... }
         pub fn len(&self) -> usize { ... }
         pub fn iter(&self) -> impl Iterator<Item = JsonType> { ... }
     }
     ```
   - **Integer vs Number distinction**: A `serde_json::Value::Number` is `JsonType::Integer` if `n.is_i64()` or `n.is_u64()` returns true. Otherwise it's `JsonType::Number`. Both `Integer` and `Number` are considered valid when the schema says `"type": "number"` (Integer is a subset of Number).

3. **`paths.rs` — Location Tracking**
   - **Purpose**: Track where in the JSON instance and schema document we are during validation, for error reporting.
   - **Key Types**:
     ```rust
     /// A segment in a JSON path — either a property name or array index.
     #[derive(Debug, Clone, PartialEq, Eq)]
     pub enum LocationSegment {
         Property(String),
         Index(usize),
     }
     
     /// A materialized JSON Pointer path (e.g., "/foo/bar/0").
     #[derive(Debug, Clone, PartialEq, Eq)]
     pub struct Location {
         segments: Vec<LocationSegment>,
     }
     
     impl Location {
         pub fn new() -> Self { ... }
         pub fn push_property(&mut self, name: impl Into<String>) { ... }
         pub fn push_index(&mut self, idx: usize) { ... }
         pub fn as_json_pointer(&self) -> String { ... }
     }
     
     impl fmt::Display for Location {
         // Formats as JSON Pointer: "/properties/name/items/0"
     }
     
     /// Lazy path construction — defers allocation until materialized.
     ///
     /// WHY: During validation of valid instances, we never need path strings.
     /// LazyLocation avoids allocating path segments until an error requires them.
     ///
     /// HOW: A singly-linked stack of segments. Each validation step pushes a
     /// segment. Only when Display or as_json_pointer() is called does it
     /// traverse the stack and build the string.
     pub struct LazyLocation<'a> {
         parent: Option<&'a LazyLocation<'a>>,
         segment: Option<LocationSegment>,
     }
     
     impl<'a> LazyLocation<'a> {
         pub fn new() -> Self { ... }
         pub fn push_property(&'a self, name: &str) -> LazyLocation<'a> { ... }
         pub fn push_index(&'a self, idx: usize) -> LazyLocation<'a> { ... }
         pub fn materialize(&self) -> Location { ... }
     }
     ```

4. **`draft.rs` — Draft Enum**
   - **Purpose**: Identify which JSON Schema specification version a schema uses.
   - **Key Type**:
     ```rust
     /// JSON Schema specification version.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
     pub enum Draft {
         /// JSON Schema Draft 4 (uses "id" not "$id")
         Draft4,
         /// JSON Schema Draft 6 (introduces "$id", "const", "contains", etc.)
         Draft6,
         /// JSON Schema Draft 7 (adds "if/then/else", "readOnly", "writeOnly")
         Draft7,
         /// JSON Schema Draft 2019-09 (vocabularies, "$recursiveRef/Anchor")
         Draft201909,
         /// JSON Schema Draft 2020-12 (prefixItems, "$dynamicRef/Anchor")
         Draft202012,
     }
     
     impl Draft {
         /// The default draft when none is specified.
         pub const DEFAULT: Draft = Draft::Draft202012;
     
         /// Detect the draft from a schema's "$schema" keyword.
         ///
         /// Returns `None` if "$schema" is absent or unrecognized.
         pub fn detect(schema: &serde_json::Value) -> Option<Draft> { ... }
     
         /// Parse a "$schema" URI string into a Draft.
         pub fn from_schema_uri(uri: &str) -> Option<Draft> { ... }
         
         /// The canonical "$schema" URI for this draft.
         pub fn schema_uri(&self) -> &'static str { ... }
         
         /// The ID keyword name for this draft ("id" for Draft4, "$id" for others).
         pub fn id_keyword(&self) -> &'static str { ... }
     }
     ```
   - **Schema URI mapping**:
     - Draft4: `"http://json-schema.org/draft-04/schema#"`
     - Draft6: `"http://json-schema.org/draft-06/schema#"`
     - Draft7: `"http://json-schema.org/draft-07/schema#"`
     - Draft201909: `"https://json-schema.org/draft/2019-09/schema"`
     - Draft202012: `"https://json-schema.org/draft/2020-12/schema"`

5. **`resolver_trait.rs` — JsonResolver Trait**
   - **Purpose**: Pluggable external schema resolution. This is the key design divergence from the reference project.
   - **Key Types**:
     ```rust
     use foundation_errstacks::{ErrorTrace, IntoErrorTrace};
     use derive_more::{Display, Error};

     /// WHY: JSON Schema allows $ref to point to external URIs. Rather than
     /// building in HTTP/file resolution, we let the caller provide their own
     /// strategy. This keeps the crate dependency-free and works in no_std.
     ///
     /// HOW: Implement resolve() to return the JSON document at the given URI.
     /// The registry calls this during schema compilation for URIs not already
     /// in its index. Each URI is resolved at most once — results are cached.
     pub trait JsonResolver {
         fn resolve(&self, uri: &str) -> Result<serde_json::Value, ErrorTrace<ResolveError>>;
     }

     /// Context type for resolution failures.
     ///
     /// WHY: Using `foundation_errstacks::ErrorTrace` lets callers attach
     /// additional context (e.g., which registry triggered the resolution,
     /// what the base URI was) as the error bubbles up through the compile stack.
     ///
     /// WHAT: A minimal error type carrying only the URI that failed.
     ///
     /// HOW: Wrapped in ErrorTrace; attachments added by the registry.
     #[derive(Debug, Display, Error)]
     #[display("failed to resolve external reference: {uri}")]
     pub struct ResolveError {
         pub uri: String,
     }

     impl ResolveError {
         pub fn new(uri: impl Into<String>) -> Self {
             Self { uri: uri.into() }
         }
     }

     /// A resolver that always fails. Default when no resolver is provided.
     pub struct NoopResolver;

     impl JsonResolver for NoopResolver {
         fn resolve(&self, uri: &str) -> Result<serde_json::Value, ErrorTrace<ResolveError>> {
             Err(ResolveError::new(uri).into_error_trace()
                 .attach(format!("no resolver configured for: {uri}")))
         }
     }

     /// A resolver backed by a pre-loaded map of URI → schema document.
     pub struct MapResolver {
         schemas: alloc::collections::BTreeMap<String, serde_json::Value>,
     }

     impl MapResolver {
         pub fn new() -> Self { ... }
         pub fn insert(&mut self, uri: impl Into<String>, schema: serde_json::Value) -> &mut Self { ... }
         pub fn from_iter(iter: impl IntoIterator<Item = (String, serde_json::Value)>) -> Self { ... }
     }

     impl JsonResolver for MapResolver {
         fn resolve(&self, uri: &str) -> Result<serde_json::Value, ErrorTrace<ResolveError>> {
             self.schemas.get(uri).cloned().ok_or_else(||
                 ResolveError::new(uri).into_error_trace()
                     .attach(format!("not found in map resolver: {uri}")))
         }
     }
     ```
   - **Note**: We use `BTreeMap` instead of `HashMap` for no_std compatibility (no hasher randomization needed). When `std` feature is on, we could provide a `HashMap` variant, but `BTreeMap` is sufficient.

### Interface Definitions

**Public API from this feature (re-exported from lib.rs):**

```rust
// types
pub use types::{JsonType, JsonTypeSet};

// paths
pub use paths::{Location, LazyLocation, LocationSegment};

// draft
pub use draft::Draft;

// resolver
pub use resolver_trait::{JsonResolver, NoopResolver, MapResolver, ResolveError};
```

### Error Handling Strategy

- `ResolveError` is a minimal `#[derive(Display, Error)]` context type — it is wrapped in `ErrorTrace<ResolveError>` when returned.
- Additional context is attached via `.attach()` at the call site (registry, compiler).
- `ErrorTrace<ResolveError>` implements `std::error::Error` when `std` feature is enabled (via `foundation_errstacks`).
- No custom `Display` or `Error` impls needed on `ResolveError` — derived via `derive_more`.

### Performance Considerations

- `JsonTypeSet` uses a u8 bitset — zero heap allocation, O(1) operations.
- `LazyLocation` avoids allocation until an error occurs — the happy path (valid instance) does zero path allocation.
- `BTreeMap` in `MapResolver` has O(log n) lookup — acceptable since resolution happens once at compile time, not per-validation.

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| BTreeMap for MapResolver | no_std compatible, no hasher dependency | HashMap (needs std or ahash); Vec<(String,Value)> with linear scan (slower for large maps) |
| LazyLocation as linked list | Zero-alloc happy path, matches reference project pattern | Pre-allocated Vec (wastes memory on valid instances); String building (many small allocations) |
| `ResolveError` as minimal derive_more type with `ErrorTrace` wrapper | Uses project-standard error handling; context attached at each call site via `.attach()`; structured chain of errors with source locations | Box<dyn Display> (loses type safety, no attachments); custom error with message field (redundant with attachment system) |
| Draft::DEFAULT = Draft202012 | Matches JSON Schema spec recommendation and reference project default | Draft7 (more widely deployed but not the latest spec) |

## Implementation

### Files to Create/Modify

- `backends/foundation_jsonschema/Cargo.toml` — New crate manifest
- `backends/foundation_jsonschema/src/lib.rs` — Crate root with no_std support
- `backends/foundation_jsonschema/src/types.rs` — JsonType, JsonTypeSet
- `backends/foundation_jsonschema/src/paths.rs` — Location, LazyLocation, LocationSegment
- `backends/foundation_jsonschema/src/draft.rs` ��� Draft enum
- `backends/foundation_jsonschema/src/resolver_trait.rs` — JsonResolver, NoopResolver, MapResolver, ResolveError
- `Cargo.toml` (workspace root) — Add foundation_jsonschema to workspace dependencies

## Tasks

- [ ] Task 1: Create `backends/foundation_jsonschema/Cargo.toml` with workspace settings, `std` and `fancy-regex` features, and dependencies (serde, serde_json, regex, ahash, percent-encoding, itoa, ryu)
- [ ] Task 2: Create `backends/foundation_jsonschema/src/lib.rs` with `#![cfg_attr(not(feature = "std"), no_std)]`, `extern crate alloc`, module declarations, and public re-exports
- [ ] Task 3: Add `foundation_jsonschema` to workspace root `Cargo.toml` under `[workspace.dependencies]`
- [ ] Task 4: Implement `JsonType` enum with all 7 variants, `as_str()`, and `Display` impl
- [ ] Task 5: Implement `JsonType::of(value: &serde_json::Value) -> Self` with correct integer/number distinction
- [ ] Task 6: Implement `JsonTypeSet` bitset with `new()`, `with()`, `insert()`, `contains()`, `is_empty()`, `len()`
- [ ] Task 7: Implement `JsonTypeSet::iter()` returning an iterator over contained types
- [ ] Task 8: Implement `JsonTypeSet::from_schema_value()` that parses a JSON Schema `type` value (string or array of strings) into a `JsonTypeSet`
- [ ] Task 9: Implement `LocationSegment` enum with `Property(String)` and `Index(usize)` variants
- [ ] Task 10: Implement `Location` struct with `new()`, `push_property()`, `push_index()`, `pop()`
- [ ] Task 11: Implement `Location::as_json_pointer()` producing RFC 6901 JSON Pointer strings (with `~0`/`~1` escaping)
- [ ] Task 12: Implement `Display` for `Location` (delegates to `as_json_pointer()`)
- [ ] Task 13: Implement `LazyLocation` linked-list structure with `new()`, `push_property()`, `push_index()`
- [ ] Task 14: Implement `LazyLocation::materialize()` that walks the linked list and produces a `Location`
- [ ] Task 15: Implement `Draft` enum with all 5 variants plus `DEFAULT` constant
- [ ] Task 16: Implement `Draft::detect(schema: &Value) -> Option<Draft>` that reads `$schema` keyword
- [ ] Task 17: Implement `Draft::from_schema_uri(uri: &str) -> Option<Draft>` with all known URI mappings (both http and https variants, with and without trailing `#`)
- [ ] Task 18: Implement `Draft::schema_uri() -> &'static str` returning the canonical URI for each draft
- [ ] Task 19: Implement `Draft::id_keyword() -> &'static str` returning `"id"` for Draft4, `"$id"` for all others
- [ ] Task 20: Implement `ResolveError` struct with `new()`, `uri()`, `reason()`, `Display`, and conditional `std::error::Error`
- [ ] Task 21: Implement `JsonResolver` trait with `resolve(&self, uri: &str) -> Result<Value, ResolveError>`
- [ ] Task 22: Implement `NoopResolver` that always returns `Err`
- [ ] Task 23: Implement `MapResolver` with `new()`, `insert()`, `from_iter()`, and `JsonResolver` impl
- [ ] Task 24: Write unit tests for `JsonType::of()` covering all value variants including edge cases (integer-valued floats like 1.0)
- [ ] Task 25: Write unit tests for `JsonTypeSet` — insert, contains, iteration, from_schema_value
- [ ] Task 26: Write unit tests for `Location` and `LazyLocation` — JSON Pointer formatting, escaping, materialization
- [ ] Task 27: Write unit tests for `Draft` — detection, URI parsing, id_keyword
- [ ] Task 28: Write unit tests for `NoopResolver` and `MapResolver` — resolve found, resolve not found, noop always fails

## Testing

### Test Cases

1. **JsonType::of — null**
   - Given: `Value::Null`
   - When: `JsonType::of(&value)`
   - Then: `JsonType::Null`

2. **JsonType::of — integer-valued float**
   - Given: `Value::Number` where `n.as_f64() == Some(1.0)` and `n.is_i64() == true`
   - When: `JsonType::of(&value)`
   - Then: `JsonType::Integer`

3. **JsonTypeSet — membership**
   - Given: Set containing `[String, Integer]`
   - When: `set.contains(JsonType::String)`, `set.contains(JsonType::Number)`
   - Then: `true`, `false`

4. **Location — JSON Pointer escaping**
   - Given: Location with segments `["foo/bar", "~baz", "0"]`
   - When: `location.as_json_pointer()`
   - Then: `"/foo~1bar/~0baz/0"`

5. **LazyLocation — materialization**
   - Given: Lazy chain `root.push_property("a").push_index(0).push_property("b")`
   - When: `lazy.materialize().as_json_pointer()`
   - Then: `"/a/0/b"`

6. **Draft::detect — $schema present**
   - Given: `{"$schema": "https://json-schema.org/draft/2020-12/schema"}`
   - When: `Draft::detect(&schema)`
   - Then: `Some(Draft::Draft202012)`

7. **Draft::detect — $schema absent**
   - Given: `{"type": "string"}`
   - When: `Draft::detect(&schema)`
   - Then: `None`

8. **MapResolver — found**
   - Given: MapResolver with `("https://example.com/a.json", json!({"type":"string"}))`
   - When: `resolver.resolve("https://example.com/a.json")`
   - Then: `Ok(json!({"type":"string"}))`

9. **NoopResolver — always fails**
   - Given: `NoopResolver`
   - When: `resolver.resolve("anything")`
   - Then: `Err(ResolveError { uri: "anything", .. })`

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] `cargo check --no-default-features` passes (no_std mode)
- [ ] `cargo check --features std` passes (std mode)
- [ ] Zero clippy warnings
- [ ] All public types have WHY/WHAT/HOW doc comments
- [ ] No TODO/FIXME/stubs

---

_Created: 2026-04-28_
