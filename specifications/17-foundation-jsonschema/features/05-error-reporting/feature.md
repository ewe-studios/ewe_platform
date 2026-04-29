---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/05-error-reporting"
this_file: "specifications/17-foundation-jsonschema/features/05-error-reporting/feature.md"

status: complete
priority: high
created: 2026-04-28

depends_on: ["00-core-types"]

tasks:
  completed: 20
  uncompleted: 0
  total: 20
  completion_percentage: 100
---

# Feature 5: Error Reporting

## Overview

Implement structured validation error types using `foundation_errstacks`. `ValidationErrorKind` is the minimal context type (derive_more `Display + Error`), and `ValidationError` is a type alias for `ErrorTrace<ValidationErrorKind>`. This gives all the `foundation_errstacks` API — `.attach()`, `.change_context()`, frame iteration, structured output, serde serialization — without a custom error struct.

Errors report the exact location in both the JSON instance and the JSON Schema document, the specific validation rule that failed, and a human-readable message. Instance and schema paths are attached as opaque attachments for programmatic access, and embedded in the Display message.

This is derived from the error system in the reference project (`crates/jsonschema/src/error.rs`), adapted to use `foundation_errstacks`.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Feature 0 (core-types) for Location, LazyLocation, JsonType
- [ ] Read `foundation_errstacks` crate docs for ErrorTrace, IntoErrorTrace, PlainResultExt

---

## Requirements

1. **`ValidationErrorKind`** — A minimal context type (using `#[derive(Display, Error)]`) that categorizes the kind of validation failure (type mismatch, missing required, value too long, etc.). This type carries the minimal data needed for categorization; additional context is attached via `ErrorTrace::attach()`.

2. **`ValidationError` type alias** — A convenient alias `type ValidationError = ErrorTrace<ValidationErrorKind>;` so callers get the full `foundation_errstacks` API (`.attach()`, `.change_context()`, frame iteration, structured output, etc.).

3. **`ErrorIterator`** — A lazy iterator that collects all validation errors (not just the first). Must support both owned and borrowed error lifetimes.

4. **`ValidationFailure`** — A struct bundling the error with instance and schema paths
   ```rust
   /// WHY: ErrorTrace carries context frames, but validation errors also need
   /// to track WHERE in the instance and schema the failure occurred. These
   /// are attached as opaque attachments for programmatic access, and embedded
   /// in the ValidationErrorKind's Display message.
   #[derive(Debug, Clone)]
   pub struct ValidationFailure {
       pub kind: ValidationErrorKind,
       pub instance_path: Location,
       pub schema_path: Location,
   }
   ```

5. **Display formatting** — Errors must format into clear, actionable messages like: `"123" is not of type "number" at path "/age"` — this is handled by the ValidationErrorKind Display impl plus any attachments from ErrorTrace.

6. **Serialization** — Errors must be serializable to JSON for structured output. When the `serde` feature of `foundation_errstacks` is enabled, `ErrorTrace` already provides `Serialize`; we additionally provide a JSON output helper.

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/src/
├── error.rs           # ValidationErrorKind, ValidationError, ErrorIterator, ValidationFailure
```

### Component Details

The error module uses `foundation_errstacks` as the error transport. `ValidationErrorKind` is the minimal context type carrying only the data needed for categorization (matching on error kind). Paths and additional context are attached via `.attach()` and `.attach_opaque()` as the error bubbles up.

```rust
use foundation_errstacks::{ErrorTrace, Frame, IntoErrorTrace};
use derive_more::{Display, Error};

// ── ValidationErrorKind ──────────────────────────────────────────────
//
// WHY: A minimal context type — only the data needed to categorize the
// validation failure. Additional context (paths, attachments) is carried
// by the ErrorTrace wrapper. This keeps pattern matching clean.

/// Categorization of all possible validation failures.
///
/// Each variant corresponds to a specific JSON Schema keyword or
/// validation rule that can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationErrorKind {
    // Type validation
    InvalidType { expected: JsonTypeSet, actual: JsonType },

    // Enum/const
    InvalidConst { expected: serde_json::Value },
    InvalidEnum { options: Vec<serde_json::Value> },

    // String
    MinLength { min: u64, actual: u64 },
    MaxLength { max: u64, actual: u64 },
    Pattern { pattern: String },
    Format { format: String },

    // Number
    Minimum { limit: f64, actual: f64, exclusive: bool },
    Maximum { limit: f64, actual: f64, exclusive: bool },
    MultipleOf { multiple: f64, actual: f64 },

    // Object
    Required { property: String },
    MinProperties { min: u64, actual: u64 },
    MaxProperties { max: u64, actual: u64 },
    AdditionalProperties { unexpected: Vec<String> },
    PropertyNames { invalid_name: String },

    // Array
    MinItems { min: u64, actual: u64 },
    MaxItems { max: u64, actual: u64 },
    UniqueItems { first: usize, second: usize },
    Contains,
    MinContains { min: u64, actual: u64 },
    MaxContains { max: u64, actual: u64 },
    AdditionalItems { limit: usize },

    // Composition
    Not,
    AnyOf,
    OneOfNotValid,
    OneOfMultipleValid,
    IfThenElse,

    // Reference
    RefNotFound { reference: String },

    // Content
    ContentEncoding { encoding: String },
    ContentMediaType { media_type: String },

    // Schema-level
    FalseSchema,

    // Unevaluated
    UnevaluatedProperties { properties: Vec<String> },
    UnevaluatedItems { index: usize },

    // Dependencies
    DependentRequired { property: String, missing: Vec<String> },

    // Custom
    Custom { message: String },
}

impl ValidationErrorKind {
    /// Returns the JSON Schema keyword name that corresponds to this error.
    ///
    /// WHY: Enables programmatic lookup of the failing keyword for
    /// tooling, IDE integration, and error filtering.
    pub fn keyword_name(&self) -> Option<&'static str> {
        Some(match self {
            InvalidType { .. } => "type",
            InvalidConst { .. } => "const",
            InvalidEnum { .. } => "enum",
            MinLength { .. } | MaxLength { .. } => return None, // covered by schema
            MaxLength { .. } => "maxLength",
            MinLength { .. } => "minLength",
            Pattern { .. } => "pattern",
            Format { .. } => "format",
            Minimum { .. } => "minimum",
            Maximum { .. } => "maximum",
            MultipleOf { .. } => "multipleOf",
            Required { .. } => "required",
            MinProperties { .. } => "minProperties",
            MaxProperties { .. } => "maxProperties",
            AdditionalProperties { .. } => "additionalProperties",
            PropertyNames { .. } => "propertyNames",
            MinItems { .. } => "minItems",
            MaxItems { .. } => "maxItems",
            UniqueItems { .. } => "uniqueItems",
            Contains { .. } => "contains",
            MinContains { .. } => "minContains",
            MaxContains { .. } => "maxContains",
            AdditionalItems { .. } => "additionalItems",
            Not { .. } => "not",
            AnyOf { .. } => "anyOf",
            OneOfNotValid { .. } | OneOfMultipleValid { .. } => "oneOf",
            IfThenElse { .. } => "if",
            RefNotFound { .. } => "$ref",
            ContentEncoding { .. } => "contentEncoding",
            ContentMediaType { .. } => "contentMediaType",
            FalseSchema { .. } => false, // special: boolean schema
            UnevaluatedProperties { .. } => "unevaluatedProperties",
            UnevaluatedItems { .. } => "unevaluatedItems",
            DependentRequired { .. } => "dependentRequired",
            Custom { .. } => return None,
        })
    }
}

// ── Display for ValidationErrorKind ──────────────────────────────────
//
// WHY: Human-readable messages for logging and API responses.
// Uses derive_more's Display for the simpler variants and manual
// impl for complex ones.

impl Display for ValidationErrorKind {
    // Manual impl matching the message patterns table below
}

impl Error for ValidationErrorKind {}

// ── ValidationError type alias ───────────────────────────────────────
//
// WHY: Callers get the full foundation_errstacks API automatically.
// ErrorTrace provides .attach(), .attach_opaque(), frame iteration,
// Serialize (with serde feature), and more.

/// A validation error with full context for debugging.
///
/// WHY: When validation fails, users need to know exactly what went wrong,
/// where in the instance, and which schema rule caused the failure.
/// Rich error context enables automated error handling and user-friendly
/// error messages in API responses.
///
/// HOW: Created via ValidationErrorKind's constructor helpers (type_error(),
/// required(), etc.) which attach instance_path and schema_path as opaque
/// attachments. The ValidationFailure struct bundles these for programmatic
/// access.
pub type ValidationError = ErrorTrace<ValidationErrorKind>;

// ── ValidationFailure ────────────────────────────────────────────────
//
// WHY: ErrorTrace carries context frames, but validation errors also need
// to track WHERE in the instance and schema the failure occurred. These
// are attached as opaque attachments for programmatic access, and embedded
// in the ValidationErrorKind's Display message.

/// A validation error bundled with instance and schema paths.
///
/// WHY: Programmatic access to the error kind and paths without parsing
/// Display output or traversing ErrorTrace frames.
#[derive(Debug, Clone)]
pub struct ValidationFailure {
    pub kind: ValidationErrorKind,
    pub instance_path: Location,
    pub schema_path: Location,
}

// ── Constructor helpers ──────────────────────────────────────────────
//
// WHY: Ergonomic construction of errors with path attachments.
// Replaces ValidationError::new() from the original design.

/// Constructor helpers that create a ValidationError with paths attached.
///
/// WHY: Every validation error needs instance_path and schema_path.
/// These helpers attach them as opaque attachments so callers can
/// extract them via ErrorTrace::frames() or build a ValidationFailure.
pub struct ValidationErrorBuilder {
    instance_path: Location,
    schema_path: Location,
}

impl ValidationErrorBuilder {
    pub fn new(instance_path: Location, schema_path: Location) -> Self {
        Self { instance_path, schema_path }
    }

    /// Create a ValidationError from a kind, attaching paths.
    pub fn build(self, kind: ValidationErrorKind) -> ValidationError {
        kind.into_error_trace()
            .attach_opaque(self.instance_path)
            .attach_opaque(self.schema_path)
    }

    /// Create a ValidationError with an additional message attachment.
    pub fn build_with_message(self, kind: ValidationErrorKind, message: impl Into<String>) -> ValidationError {
        self.build(kind).attach(message)
    }
}

/// Extract instance_path from error frames (first Location attachment).
pub fn extract_instance_path(error: &ValidationError) -> Option<&Location> {
    error.frames().find_map(|frame| frame.attachment_as::<Location>())
}

/// Extract schema_path from error frames (second Location attachment).
pub fn extract_schema_path(error: &ValidationError) -> Option<&Location> {
    // Skip first Location (instance_path), get second
    error.frames().filter_map(|f| f.attachment_as::<Location>()).nth(1)
}

/// Build a ValidationFailure from a ValidationError.
pub fn to_failure(error: &ValidationError) -> Option<ValidationFailure> {
    let kind = error.current_context().clone();
    let mut locations = error.frames().filter_map(|f| f.attachment_as::<Location>());
    let instance_path = locations.next()?.clone();
    let schema_path = locations.next()?.clone();
    Some(ValidationFailure { kind, instance_path, schema_path })
}

// ── ErrorIterator ────────────────────────────────────────────────────

/// Lazy iterator over validation errors.
pub type ErrorIterator = Box<dyn Iterator<Item = ValidationError>>;

// ── Serde serialization ─────────────────────────────────────────────
//
// WHY: Structured JSON error output for API responses.
// ErrorTrace<C> already implements Serialize (with serde feature).
// ValidationErrorKind derives Serialize conditionally.

#[cfg(feature = "serde")]
impl serde::Serialize for ValidationErrorKind {
    // Serialize to { "kind": "<variant>", "data": {...} } for API consumers
}

#[cfg(feature = "serde")]
pub fn errors_to_json(errors: &[ValidationError]) -> serde_json::Value {
    serde_json::to_value(errors).expect("error serialization is infallible")
}

// ── std::error::Error for ValidationError ────────────────────────────
//
// WHY: Interop with std error handling. foundation_errstacks already
// provides impl Error for ErrorTrace<C> when the "std" feature is on.
// (No additional impl needed.)
```

### Error Message Patterns

Each `ValidationErrorKind` variant produces a specific message:

| ErrorKind | Message Pattern |
|-----------|----------------|
| `InvalidType` | `"value is not of type(s) {expected}"` |
| `Required` | `"'{property}' is a required property"` |
| `MinLength` | `"string is too short (length {actual}, minimum {min})"` |
| `Pattern` | `"string does not match pattern '{pattern}'"` |
| `Minimum` | `"value {actual} is less than minimum {limit}"` |
| `AdditionalProperties` | `"additional properties are not allowed: {unexpected}"` |
| `UniqueItems` | `"array items at indices {first} and {second} are not unique"` |
| `RefNotFound` | `"reference '{reference}' could not be resolved"` |

### Usage Example

```rust
// Creating a validation error:
let error = ValidationErrorBuilder::new(
    instance_path.materialize(),
    self.schema_path.clone(),
).build(ValidationErrorKind::MinLength { min: self.min, actual: len });

// Extracting paths for programmatic access:
let failure = to_failure(&error).unwrap();
assert_eq!(failure.instance_path.as_str(), "/age");
assert_eq!(failure.kind, ValidationErrorKind::Minimum { ... });

// Display gives human-readable output:
println!("{}", error); // "value 12 is less than minimum 18 at path /age"

// Iterate frames for structured debugging:
for frame in error.frames() {
    println!("  frame: {} (attachments: {})", frame.kind(), frame.attachments().len());
}

// Serialize to JSON (with serde feature):
let json = errors_to_json(&[error]);
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| ErrorTrace<ValidationErrorKind> | Reuses foundation_errstacks API; attach context as errors bubble up; serde built-in | Custom struct (duplicates ErrorTrace functionality); Box<dyn Error> (loses type info) |
| ValidationErrorKind as flat enum | Simple pattern matching, no nesting | Nested enums by category (over-structured); trait objects (lose pattern matching) |
| Box<dyn Iterator> for ErrorIterator | Flexible, works with any validator composition | Vec<ValidationError> (eager — allocates even if not consumed); custom iterator type (more code) |
| f64 for numeric limits | Matches serde_json's number representation | BigDecimal (heavy dependency); String (loses numeric comparison) |
| ValidationErrorBuilder helper | Ergonomic construction with path attachments; replaces ValidationError::new() | Inline construction (repetitive); macros (harder to read) |
| ValidationFailure struct | Programmatic access to kind + paths without frame traversal | Only ErrorTrace frames (requires traversal to extract); separate return values (awkward) |

## Tasks

- [ ] Task 1: Define `ValidationErrorKind` enum with all variants listed above
- [ ] Task 2: Implement `Display` for `ValidationErrorKind` with human-readable messages for each variant
- [ ] Task 3: Define `ValidationError` type alias (`type ValidationError = ErrorTrace<ValidationErrorKind>`)
- [ ] Task 4: Implement `ValidationErrorBuilder` — ergonomic constructor that attaches paths
- [ ] Task 5: Implement `ValidationFailure` struct and `to_failure()` helper
- [ ] Task 6: Implement `extract_instance_path()` / `extract_schema_path()` frame helpers
- [ ] Task 7: Implement `std::error::Error` for `ValidationError` (provided by foundation_errstacks std feature)
- [ ] Task 8: Define `ErrorIterator` type alias
- [ ] Task 9: Implement `Serialize` for `ValidationErrorKind` (conditional on serde feature)
- [ ] Task 10: Implement `errors_to_json()` helper (conditional on serde feature)
- [ ] Task 11: Implement `ValidationErrorKind::keyword_name()` returning the schema keyword
- [ ] Task 12: Write unit tests for `ValidationErrorKind::Display` — verify message format for every variant
- [ ] Task 13: Write unit tests for `ValidationError` Display — verify path context in output
- [ ] Task 14: Write unit tests for `ValidationError` serialization to JSON
- [ ] Task 15: Write unit tests for `ErrorIterator` — collect multiple errors, verify count and contents
- [ ] Task 16: Test `ValidationErrorBuilder` construction and path attachment
- [ ] Task 17: Test no_std compatibility of error types
- [ ] Task 18: Test `to_failure()` extraction round-trip
- [ ] Task 19: Write tests for `keyword_name()` mapping
- [ ] Task 20: Test frame iteration and attachment downcasting

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Every `ValidationErrorKind` variant has a clear, actionable Display message
- [ ] Errors include both instance path and schema path (as opaque attachments)
- [ ] JSON serialization produces well-structured output
- [ ] Zero clippy warnings
- [ ] no_std compatible
- [ ] `ValidationFailure` round-trips correctly from `ValidationError`

---

_Created: 2026-04-28_
