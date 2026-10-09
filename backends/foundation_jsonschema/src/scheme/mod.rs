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
//! ## Direct pipeline: build → compile → validate
//!
//! ```
//! use foundation_jsonschema::scheme;
//!
//! let validator = scheme::object()
//!     .required("name", scheme::string().min_len(1))
//!     .required("age", scheme::integer().min(0).max(150))
//!     .strict()
//!     .build()
//!     .compile()
//!     .unwrap();
//!
//! let ok = validator.is_valid(&serde_json::json!({"name": "Alice", "age": 30}));
//! assert!(ok);
//! ```
//!
//! ## Extract raw JSON Schema
//!
//! ```
//! use foundation_jsonschema::scheme;
//!
//! let schema = scheme::string().min_len(1).max_len(100).build_schema();
//! assert_eq!(schema["minLength"], 1);
//! assert_eq!(schema["maxLength"], 100);
//! ```
//!
//! ## User profile schema with optional fields
//!
//! ```
//! use foundation_jsonschema::scheme;
//!
//! let validator = scheme::object()
//!     .required("username", scheme::string().min_len(3).max_len(32))
//!     .required("email", scheme::string().email())
//!     .optional("bio", scheme::string().max_len(500))
//!     .optional("avatar_url", scheme::string().uri().nullable())
//!     .strict()
//!     .build()
//!     .compile()
//!     .unwrap();
//!
//! // Minimal user (only required fields)
//! assert!(validator.is_valid(&serde_json::json!({
//!     "username": "alice",
//!     "email": "alice@example.com"
//! })));
//!
//! // Full user with optional fields
//! assert!(validator.is_valid(&serde_json::json!({
//!     "username": "alice",
//!     "email": "alice@example.com",
//!     "bio": "Hello world"
//! })));
//! ```
//!
//! ## Coordinate tuple (fixed-length array)
//!
//! ```
//! use foundation_jsonschema::scheme;
//!
//! let schema = scheme::array()
//!     .prefix_items(vec![
//!         scheme::number().build_schema(),  // x
//!         scheme::number().build_schema(),  // y
//!         scheme::number().build_schema(),  // z
//!     ])
//!     .build_schema();
//!
//! assert_eq!(schema["prefixItems"].as_array().unwrap().len(), 3);
//! ```
//!
//! ## Discriminated union (oneOf with const discriminator)
//!
//! ```
//! use foundation_jsonschema::{scheme, ValidationOptions};
//! use serde_json::json;
//!
//! let schema = scheme::one_of(vec![
//!     scheme::object()
//!         .required("type", scheme::literal(json!("circle")))
//!         .required("radius", scheme::number().positive())
//!         .strict()
//!         .build_schema(),
//!     scheme::object()
//!         .required("type", scheme::literal(json!("rectangle")))
//!         .required("width", scheme::number().positive())
//!         .required("height", scheme::number().positive())
//!         .strict()
//!         .build_schema(),
//! ]);
//!
//! let validator = ValidationOptions::new().build(&schema).unwrap();
//!
//! assert!(validator.is_valid(&json!({"type": "circle", "radius": 5.0})));
//! assert!(validator.is_valid(&json!({"type": "rectangle", "width": 3.0, "height": 4.0})));
//! assert!(!validator.is_valid(&json!({"type": "circle", "width": 3.0})));
//! ```

pub mod array;
pub mod compound;
pub mod literal;
pub mod object;
pub mod primitives;
pub mod reference;

pub use array::{array, array_of, ArraySchema};
pub use compound::{all_of, any_of, if_then, if_then_else, intersection, not, one_of, union};
pub use literal::{literal, r#enum};
pub use object::{object, ObjectSchema};
pub use primitives::*;
pub use reference::{dynamic_ref, ref_};

// Re-export for type signatures — users rarely need this directly.
pub use crate::ValidationOptions;
use alloc::collections::BTreeMap;
use serde_json::Value;
