//! Referencing engine for URI resolution, registry, and JSON Pointer traversal.
//!
//! WHY: JSON Schema `$ref` keywords need to resolve URIs against a registry
//! of known schema resources. This module handles URI parsing, JSON Pointer
//! traversal, anchor resolution, and dynamic scope tracking.
//!
//! WHAT: `Registry` (indexed schema collection), `Resolver` (reference resolver),
//! `Resource` (schema document + draft), JSON Pointer traversal.
//!
//! HOW: The registry is built once with all known schemas, then a resolver
//! is created from it to resolve `$ref` strings.

#![allow(dead_code)]
#![allow(unused_variables)]

pub mod uri;
pub mod pointer;
pub mod resource;
pub mod anchor;
pub mod registry;
pub mod resolver;
pub mod vocabulary;
pub(crate) mod spec;

pub use registry::{Registry, RegistryBuilder};
pub use resolver::{Resolved, Resolver};
pub use resource::{Resource, ResourceRef};
pub use vocabulary::VocabularySet;
