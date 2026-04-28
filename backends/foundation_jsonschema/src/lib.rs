//! Self-contained JSON Schema validation for ewe_platform.
//!
//! Validates JSON instances against JSON Schema documents supporting
//! Draft 4, Draft 6, Draft 7, Draft 2019-09, and Draft 2020-12.
//!
//! # Overview
//!
//! WHY: The ewe_platform needs a JSON Schema validator that works in `std` and
//! `no_std + alloc` environments without pulling in network dependencies like
//! reqwest, tokio, or wasm-bindgen. External reference resolution is handled via
//! a pluggable `JsonResolver` trait.
//!
//! WHAT: A compile-once, validate-many JSON Schema validator with structured error
//! reporting and evaluation output.
//!
//! HOW: Schemas are compiled into an immutable validator tree (`SchemaNode`).
//! Instances are validated against the compiled tree. Validation errors carry
//! full context (instance path, schema path, error category) via `foundation_errstacks`.
//!
//! # Features
//!
//! - **`std`** (default): Enables `std::error::Error` impls, tracing, and
//!   `std`-only features of `foundation_errstacks`.
//! - **`fancy-regex`**: Enables ECMA-262 compatible regex via `fancy-regex`
//!   for the `pattern` keyword.
//!
//! # Quick Start
//!
//! ```ignore
//! use foundation_jsonschema::{validator_for, Draft};
//! use serde_json::json;
//!
//! let schema = json!({"type": "object", "properties": {"name": {"type": "string"}}});
//! let validator = validator_for(&schema).unwrap();
//!
//! assert!(validator.is_valid(&json!({"name": "Alice"})));
//! assert!(!validator.is_valid(&json!({"name": 42})));
//! ```
//!
//! # External References
//!
//! By default, external `$ref` URIs are rejected. Provide a custom resolver:
//!
//! ```ignore
//! use foundation_jsonschema::ValidationOptions;
//! use serde_json::json;
//!
//! let schema = json!({"$ref": "https://example.com/types.json"});
//! let resolver = MyResolver::new(); // implement JsonResolver
//! let validator = ValidationOptions::new()
//!     .with_resolver(resolver)
//!     .build(&schema)
//!     .unwrap();
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// ── Core Types (Feature 0) ──────────────────────────────────────────

mod types;
mod paths;
mod draft;
mod resolver_trait;

// ── Error Reporting (Feature 5) ─────────────────────────────────────

mod error;

// ── Referencing Engine (Feature 1) ──────────────────────────────────

pub mod referencing;

// ── Keywords & Validators (Feature 2) ───────────────────────────────

mod keywords;

// ── Compiler (Feature 3) ────────────────────────────────────────────

mod compiler;
mod compiler_context;
mod node;
mod options;

// ── Validation Engine (Feature 4) ───────────────────────────────────

mod validator;
mod validation_context;
mod evaluation;

// ── In-Memory Fetcher (Meta-schema bundle) ───────────────────────────

mod in_memory_fetcher;

// ── Public API ──────────────────────────────────────────────────────

pub use draft::Draft;
pub use error::{
    ErrorIterator, ValidationFailure, ValidationError, ValidationErrorBuilder, ValidationErrorKind,
};
pub use paths::{LazyLocation, Location, LocationSegment};
pub use resolver_trait::{JsonResolver, ResolveError};
pub use types::{JsonType, JsonTypeSet};
pub use validator::Validator;

pub use options::ValidationOptions;
pub use in_memory_fetcher::InMemoryFetcher;

/// Validate an instance against a schema (boolean result).
///
/// WHY: Convenience function for the common case where you only need
/// to know if an instance is valid, without error details.
///
/// WHAT: Returns `true` if the instance validates against the schema.
///
/// HOW: Compiles the schema with default options, then checks validity.
/// For custom resolvers or draft selection, use `ValidationOptions`.
///
/// # Errors
///
/// Returns an error if schema compilation fails (e.g., invalid schema structure).
pub fn is_valid(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<bool, ValidationError> {
    let validator = validator_for(schema)?;
    Ok(validator.is_valid(instance))
}

/// Validate an instance against a schema, returning the first error.
///
/// WHY: Convenience function for the common case where you need the first
/// validation error but don't care about collecting all errors.
///
/// WHAT: Returns `Ok(())` if valid, or `Err` with the first validation failure.
///
/// HOW: Compiles the schema with default options, then validates.
///
/// # Errors
///
/// Returns a compilation error if the schema is invalid, or a validation error
/// if the instance fails validation.
pub fn validate(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), ValidationError> {
    let validator = validator_for(schema)?;
    validator.validate(instance)
}

/// Compile a JSON Schema into a reusable validator.
///
/// WHY: The most common entry point for schema validation. Compiles the schema
/// with default options (NoopResolver, draft auto-detection).
///
/// WHAT: Returns a `Validator` that can validate many instances against the schema.
///
/// HOW: Delegates to `ValidationOptions::new().build()`.
///
/// # Errors
///
/// Returns an error if the schema cannot be compiled (invalid structure,
/// unresolvable references, etc.).
pub fn validator_for(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
    ValidationOptions::new().build(schema)
}

// Draft-specific convenience modules
/// Draft 4 validation convenience functions.
pub mod draft4 {
    use super::*;

    /// Compile a schema with Draft 4 rules.
    pub fn compile(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        ValidationOptions::new().with_draft(Draft::Draft4).build(schema)
    }
}

/// Draft 6 validation convenience functions.
pub mod draft6 {
    use super::*;

    /// Compile a schema with Draft 6 rules.
    pub fn compile(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        ValidationOptions::new().with_draft(Draft::Draft6).build(schema)
    }
}

/// Draft 7 validation convenience functions.
pub mod draft7 {
    use super::*;

    /// Compile a schema with Draft 7 rules.
    pub fn compile(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        ValidationOptions::new().with_draft(Draft::Draft7).build(schema)
    }
}

/// Draft 2019-09 validation convenience functions.
pub mod draft201909 {
    use super::*;

    /// Compile a schema with Draft 2019-09 rules.
    pub fn compile(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        ValidationOptions::new().with_draft(Draft::Draft201909).build(schema)
    }
}

/// Draft 2020-12 validation convenience functions.
pub mod draft202012 {
    use super::*;

    /// Compile a schema with Draft 2020-12 rules.
    pub fn compile(schema: &serde_json::Value) -> Result<Validator, ValidationError> {
        ValidationOptions::new().with_draft(Draft::Draft202012).build(schema)
    }
}

#[cfg(test)]
mod tests;
