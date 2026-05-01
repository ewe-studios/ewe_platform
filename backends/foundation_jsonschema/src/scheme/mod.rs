//! `scheme` — Fluent, type-safe JSON Schema builder.
//!
//! WHY: Constructing JSON Schema documents with `serde_json::json!` is verbose
//! and error-prone — you can set `.format("email")` on an integer schema, and
//! typos in keyword names are caught only at runtime. This module provides
//! type-specific builders where only valid methods are available.
//!
//! WHAT: Builder structs for each JSON Schema type (string, integer, object,
//! array, etc.) with type-specific constraint methods.
//!
//! HOW: Each builder accumulates JSON Schema properties in a `BTreeMap`.
//! `.build_schema()` returns the `serde_json::Value`. `.build()` returns
//! a `ValidationOptions` with the schema embedded, ready to chain into
//! `.compile()` for immediate validation.
//!
//! # Examples
//!
//! ```
//! use foundation_jsonschema::scheme;
//!
//! // Direct pipeline: build → compile → validate
//! let validator = scheme::object()
//!     .required("name", scheme::string().min_len(1))
//!     .required("age", scheme::integer().min(0).max(150))
//!     .strict()
//!     .build()
//!     .compile()
//!     .unwrap();
//!
//! // Extract raw JSON Schema
//! let schema = scheme::string().min_len(1).max_len(100).build_schema();
//! ```

pub mod array;
pub mod compound;
pub mod literal;
pub mod object;
pub mod primitives;
pub mod reference;

pub use array::{ArraySchema, array, array_of};
pub use compound::{all_of, any_of, if_then, if_then_else, intersection, not, one_of, union};
pub use literal::{literal, r#enum};
pub use object::{object, ObjectSchema};
pub use primitives::*;
pub use reference::{dynamic_ref, ref_};

// Re-export for type signatures — users rarely need this directly.
pub use crate::ValidationOptions;
use alloc::collections::BTreeMap;
use serde_json::Value;
