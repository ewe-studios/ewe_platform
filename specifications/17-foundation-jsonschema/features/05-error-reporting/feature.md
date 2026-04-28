---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/05-error-reporting"
this_file: "specifications/17-foundation-jsonschema/features/05-error-reporting/feature.md"

status: pending
priority: high
created: 2026-04-28

depends_on: ["00-core-types"]

tasks:
  completed: 0
  uncompleted: 20
  total: 20
  completion_percentage: 0
---

# Feature 5: Error Reporting

## Overview

Implement structured validation error types with detailed path tracking, error categorization, and iteration support. Errors must report the exact location in both the JSON instance and the JSON Schema document, the specific validation rule that failed, and a human-readable message.

This is derived from the error system in the reference project (`crates/jsonschema/src/error.rs`).

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Feature 0 (core-types) for Location, LazyLocation, JsonType

---

## Requirements

1. **ValidationError** — A structured error type containing:
   - The failing JSON value (or a description of it)
   - Instance path (where in the JSON instance the error occurred)
   - Schema path (where in the schema the failing keyword is)
   - Error kind (an enum categorizing the failure)
   - Human-readable message

2. **ErrorKind enum** — Categorize all possible validation failures: type mismatch, missing required property, value too long, pattern mismatch, ref not found, etc.

3. **ErrorIterator** — A lazy iterator that collects all validation errors (not just the first). Must support both owned and borrowed error lifetimes.

4. **Display formatting** — Errors must format into clear, actionable messages like: `"123" is not of type "number" at path "/age"`

5. **Serialization** — Errors must be serializable to JSON for structured output.

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/src/
├── error.rs           # ValidationError, ErrorKind, ErrorIterator, Display impls
```

### Component Details

1. **`error.rs`**
   ```rust
   /// A validation error with full context for debugging.
   ///
   /// WHY: When validation fails, users need to know exactly what went wrong,
   /// where in the instance, and which schema rule caused the failure.
   /// Rich error context enables automated error handling and user-friendly
   /// error messages in API responses.
   #[derive(Debug)]
   pub struct ValidationError {
       /// What kind of validation failure occurred.
       pub kind: ErrorKind,
       /// JSON Pointer path to the failing value in the instance.
       pub instance_path: Location,
       /// JSON Pointer path to the failing keyword in the schema.
       pub schema_path: Location,
       /// Human-readable description of the failure.
       message: String,
   }
   
   /// Categorization of all possible validation failures.
   ///
   /// Each variant corresponds to a specific JSON Schema keyword or
   /// validation rule that can fail.
   #[derive(Debug, Clone, PartialEq)]
   pub enum ErrorKind {
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
   
   /// Lazy iterator over validation errors.
   pub type ErrorIterator = Box<dyn Iterator<Item = ValidationError>>;
   
   impl ValidationError {
       pub fn new(kind: ErrorKind, instance_path: Location, schema_path: Location) -> Self { ... }
       pub fn kind(&self) -> &ErrorKind { ... }
       pub fn instance_path(&self) -> &Location { ... }
       pub fn schema_path(&self) -> &Location { ... }
   }
   
   impl fmt::Display for ValidationError {
       // Format: "{message} at instance path {instance_path}"
   }
   
   impl fmt::Display for ErrorKind {
       // Format each variant as a human-readable message
   }
   
   #[cfg(feature = "std")]
   impl std::error::Error for ValidationError {}
   ```

### Error Message Patterns

Each `ErrorKind` variant produces a specific message:

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

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| Owned String in message | Error messages may outlive the validation context | &str (lifetime issues); Cow<str> (more complex API) |
| ErrorKind as flat enum | Simple pattern matching, no nesting | Nested enums by category (over-structured); trait objects (lose pattern matching) |
| Box<dyn Iterator> for ErrorIterator | Flexible, works with any validator composition | Vec<ValidationError> (eager — allocates even if not consumed); custom iterator type (more code) |
| f64 for numeric limits | Matches serde_json's number representation | BigDecimal (heavy dependency); String (loses numeric comparison) |

## Tasks

- [ ] Task 1: Define `ErrorKind` enum with all variants listed above
- [ ] Task 2: Implement `Display` for `ErrorKind` with human-readable messages for each variant
- [ ] Task 3: Define `ValidationError` struct with `kind`, `instance_path`, `schema_path`, `message` fields
- [ ] Task 4: Implement `ValidationError::new()` that auto-generates message from ErrorKind
- [ ] Task 5: Implement `Display` for `ValidationError` — include path context
- [ ] Task 6: Implement `std::error::Error` for `ValidationError` (feature-gated)
- [ ] Task 7: Define `ErrorIterator` type alias
- [ ] Task 8: Implement `Serialize` for `ErrorKind` (using serde) for JSON output
- [ ] Task 9: Implement `Serialize` for `ValidationError` for JSON output
- [ ] Task 10: Implement helper constructors: `ValidationError::type_error()`, `ValidationError::required()`, etc.
- [ ] Task 11: Write unit tests for `ErrorKind::Display` — verify message format for every variant
- [ ] Task 12: Write unit tests for `ValidationError::Display` — verify path context in output
- [ ] Task 13: Write unit tests for `ValidationError` serialization to JSON
- [ ] Task 14: Write unit tests for `ErrorIterator` — collect multiple errors, verify count and contents
- [ ] Task 15: Test error construction helpers
- [ ] Task 16: Test no_std compatibility of error types
- [ ] Task 17: Implement `PartialEq` for `ValidationError` (for test assertions)
- [ ] Task 18: Implement `Clone` for `ValidationError`
- [ ] Task 19: Implement `ErrorKind::keyword_name()` returning the schema keyword that caused the error
- [ ] Task 20: Write tests for `keyword_name()` mapping

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Every `ErrorKind` variant has a clear, actionable Display message
- [ ] Errors include both instance path and schema path
- [ ] JSON serialization produces well-structured output
- [ ] Zero clippy warnings
- [ ] no_std compatible

---

_Created: 2026-04-28_
