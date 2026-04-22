//! Foundation `OpenAPI` - `OpenAPI` spec processing and normalization.
//!
//! WHY: Both code generation and runtime need to process `OpenAPI` specs to extract
//! endpoint information, request/response types, and error schemas. Centralizing
//! this logic avoids duplication and ensures consistency.
//!
//! WHAT: Provides utilities to parse `OpenAPI` specs, extract endpoint metadata,
//! resolve $ref references, handle composition types (allOf/oneOf/anyOf), and
//! produce normalized JSON representations for quick lookup.
//!
//! HOW: Uses minimal deserialization for efficiency, builds a map of endpoints
//! to their request/response types, and outputs a simplified JSON format.
//!
//! # Example
//!
//! ```rust,no_run
//! use foundation_openapi::{SpecProcessor, normalize_spec};
//!
//! let spec_json = r#"{"openapi": "3.0.0", "info": {"title": "API", "version": "1.0"}, "paths": {}}"#;
//!
//! // Quick normalization
//! let normalized = normalize_spec(spec_json).unwrap();
//! println!("Total endpoints: {}", normalized.metadata.total_endpoints);
//!
//! // Or use the processor for more control
//! let processor = foundation_openapi::process_spec(spec_json).unwrap();
//! let endpoints = processor.endpoints();
//! ```

pub mod api_catalog;
pub mod classifier;
pub mod endpoint;
pub mod extractor;
pub mod normalizer;
pub mod spec;
pub mod type_resolver;
pub mod unified;

// Re-exports for convenient access
pub use api_catalog::{
    discover_providers, escape_rust_keyword, escape_rust_keyword_with_underscore, has_sub_apis,
    operation_id_to_fn_name, path_to_fn_suffix, sanitize_doc_comment, sanitize_field_name,
    sanitize_identifier, to_pascal_case, to_pascal_case_from_any, to_sentence_case, to_snake_case,
    ApiCatalog, ApiCatalogBuilder, ApiInfo,
};
pub use classifier::OperationTypeClassifier;
pub use endpoint::{EndpointInfo, OperationEffect, OperationType, ResponseType};
pub use extractor::EndpointExtractor;
pub use normalizer::{
    normalize_spec, process_spec, NormalizedEndpoint, NormalizedSpec, ProcessError,
    PropertyDefinition, SpecMetadata, SpecProcessor, TypeDefinition, TypeKind,
};
pub use spec::{OpenApiSpec, Schema, SpecFormat};
pub use type_resolver::TypeResolver;

// Re-exports for unified code generation
pub use unified::{
    analyze_spec as analyze_unified, AnalysisOptions, AnalysisResult, ApiGroup, GenError,
    UnifiedGenerator,
};
