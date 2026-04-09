//! Spec normalization for quick introspection.
//!
//! WHY: Produce a simplified JSON representation for quick introspection and code generation.
//!
//! WHAT: NormalizedSpec struct with serializable endpoint and type definitions.
//!
//! HOW: Aggregates endpoint and type information into a single serializable structure.

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::spec::{OpenApiSpec, Schema};
use crate::endpoint::EndpointInfo;
use crate::type_resolver::TypeResolver;
use crate::extractor::EndpointExtractor;

/// Normalized OpenAPI spec representation.
#[derive(Debug, Serialize)]
pub struct NormalizedSpec {
    /// Map of endpoint path -> method -> details
    pub endpoints: BTreeMap<String, BTreeMap<String, NormalizedEndpoint>>,

    /// All discovered type definitions
    pub types: BTreeMap<String, TypeDefinition>,

    /// Metadata about the spec
    pub metadata: SpecMetadata,
}

/// Normalized endpoint details.
#[derive(Debug, Serialize)]
pub struct NormalizedEndpoint {
    pub operation_id: String,
    pub request_type: Option<String>,
    pub response_type: Option<String>,
    pub error_types: BTreeMap<String, String>,
    pub path_params: Vec<String>,
    pub query_params: Vec<String>,
}

/// Type definition for normalized output.
#[derive(Debug, Serialize, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub kind: TypeKind,
    pub properties: Vec<PropertyDefinition>,
}

/// Composition type details.
#[derive(Debug, Serialize, Clone)]
pub struct CompositionDetails {
    pub composition_type: String,
    pub refs: Vec<String>,
}

/// Type kind discriminator.
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum TypeKind {
    Object,
    Composition(CompositionDetails),
    Primitive { primitive: String },
    Array { items: String },
}

/// Property definition for type details.
#[derive(Debug, Serialize, Clone)]
pub struct PropertyDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub required: bool,
}

/// Spec metadata.
#[derive(Debug, Serialize)]
pub struct SpecMetadata {
    pub spec_format: String,
    pub base_url: Option<String>,
    pub total_endpoints: usize,
    pub total_types: usize,
    pub api_version: String,
    pub api_title: String,
}

/// Spec processor - main entry point.
pub struct SpecProcessor {
    spec: Arc<OpenApiSpec>,
    schemas: Arc<BTreeMap<String, Schema>>,
}

impl SpecProcessor {
    /// Create processor from parsed spec.
    pub fn new(spec: Arc<OpenApiSpec>) -> Self {
        let schemas = spec.all_schemas();
        let schema_map: BTreeMap<String, Schema> = schemas
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self {
            spec,
            schemas: Arc::new(schema_map),
        }
    }

    /// Get all endpoints with full type information.
    pub fn endpoints(&self) -> Vec<EndpointInfo> {
        let extractor = EndpointExtractor::new(self.spec.clone());
        extractor.extract_all()
    }

    /// Get all type definitions.
    pub fn types(&self) -> Vec<TypeDefinition> {
        let resolver = TypeResolver::new(self.schemas.clone());
        let mut types = Vec::new();

        for (name, schema) in self.schemas.iter() {
            if let Some(type_def) = Self::schema_to_type_def(&resolver, &name, schema) {
                types.push(type_def);
            }
        }

        types
    }

    /// Convert a schema to a type definition.
    fn schema_to_type_def(_resolver: &TypeResolver, name: &str, schema: &Schema) -> Option<TypeDefinition> {
        let kind = if let Some(all_of) = &schema.all_of {
            let refs: Vec<String> = all_of
                .iter()
                .filter_map(|s| s.ref_path.as_ref())
                .map(|ref_path| {
                    let n = ref_path.trim_start_matches("#/components/schemas/");
                    TypeResolver::to_pascal_case(n)
                })
                .collect();

            if refs.is_empty() {
                TypeKind::Object
            } else {
                TypeKind::Composition(CompositionDetails {
                    composition_type: "allOf".to_string(),
                    refs,
                })
            }
        } else if let Some(one_of) = &schema.one_of {
            let refs: Vec<String> = one_of
                .iter()
                .filter_map(|s| s.ref_path.as_ref())
                .map(|ref_path| {
                    let n = ref_path.trim_start_matches("#/components/schemas/");
                    TypeResolver::to_pascal_case(n)
                })
                .collect();

            TypeKind::Composition(CompositionDetails {
                composition_type: "oneOf".to_string(),
                refs,
            })
        } else if let Some(any_of) = &schema.any_of {
            let refs: Vec<String> = any_of
                .iter()
                .filter_map(|s| s.ref_path.as_ref())
                .map(|ref_path| {
                    let n = ref_path.trim_start_matches("#/components/schemas/");
                    TypeResolver::to_pascal_case(n)
                })
                .collect();

            TypeKind::Composition(CompositionDetails {
                composition_type: "anyOf".to_string(),
                refs,
            })
        } else if let Some(items) = &schema.items {
            let item_type = items.ref_path.as_ref()
                .map(|ref_path| {
                    let n = ref_path.trim_start_matches("#/components/schemas/");
                    TypeResolver::to_pascal_case(n)
                })
                .or_else(|| items.schema_type.as_ref().map(|t| t.clone()))
                .unwrap_or_else(|| "unknown".to_string());

            TypeKind::Array { items: item_type }
        } else if let Some(ty) = &schema.schema_type {
            if ty == "object" {
                TypeKind::Object
            } else {
                TypeKind::Primitive { primitive: ty.clone() }
            }
        } else {
            TypeKind::Object
        };

        // Extract properties
        let properties = schema.properties.as_ref().map(|props| {
            props
                .iter()
                .map(|(name, prop_schema)| {
                    let ty = prop_schema.ref_path.as_ref()
                        .map(|ref_path| {
                            let n = ref_path.trim_start_matches("#/components/schemas/");
                            TypeResolver::to_pascal_case(n)
                        })
                        .or_else(|| prop_schema.schema_type.as_ref().map(|t| t.clone()))
                        .unwrap_or_else(|| "unknown".to_string());

                    PropertyDefinition {
                        name: name.clone(),
                        ty,
                        required: schema.required.contains(name),
                    }
                })
                .collect()
        }).unwrap_or_default();

        Some(TypeDefinition {
            name: TypeResolver::rename_if_keyword(TypeResolver::to_pascal_case(name)),
            kind,
            properties,
        })
    }

    /// Get normalized representation.
    pub fn normalize(&self) -> NormalizedSpec {
        let endpoints = self.endpoints();
        let types = self.types();

        // Build endpoint map
        let mut endpoint_map: BTreeMap<String, BTreeMap<String, NormalizedEndpoint>> = BTreeMap::new();
        for ep in &endpoints {
            let path_entry = endpoint_map.entry(ep.path.clone()).or_default();

            let response_type_str = ep.response_type.as_ref().map(|rt| rt.as_rust_type().to_string());

            path_entry.insert(
                ep.method.clone(),
                NormalizedEndpoint {
                    operation_id: ep.operation_id.clone(),
                    request_type: ep.request_type.clone(),
                    response_type: response_type_str,
                    error_types: ep.error_types.clone(),
                    path_params: ep.path_params.clone(),
                    query_params: ep.query_params.clone(),
                },
            );
        }

        // Build type map
        let type_map: BTreeMap<String, TypeDefinition> = types
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();

        let total_types = type_map.len();

        // Detect spec format
        let spec_format = match self.spec.format() {
            crate::spec::SpecFormat::OpenApi3x => "openapi_3x",
            crate::spec::SpecFormat::GcpDiscovery => "gcp_discovery",
            crate::spec::SpecFormat::Consolidated => "consolidated",
            crate::spec::SpecFormat::Unknown => "unknown",
        };

        NormalizedSpec {
            endpoints: endpoint_map,
            types: type_map.clone(),
            metadata: SpecMetadata {
                spec_format: spec_format.to_string(),
                base_url: self.spec.base_url(),
                total_endpoints: endpoints.len(),
                total_types,
                api_version: self.spec.info.as_ref().map(|i| i.version.clone()).unwrap_or_default(),
                api_title: self.spec.info.as_ref().map(|i| i.title.clone()).unwrap_or_default(),
            },
        }
    }

    /// Export normalized spec as JSON string.
    pub fn to_normalized_json(&self) -> Result<String, serde_json::Error> {
        let normalized = self.normalize();
        serde_json::to_string_pretty(&normalized)
    }

    /// Get base URL for the API.
    pub fn base_url(&self) -> Option<String> {
        self.spec.base_url()
    }

    /// Get API version from info.
    pub fn version(&self) -> Option<&str> {
        self.spec.info.as_ref().map(|i| i.version.as_str())
    }

    /// Get API title from info.
    pub fn title(&self) -> Option<&str> {
        self.spec.info.as_ref().map(|i| i.title.as_str())
    }
}

/// Convenience function to process a spec JSON string.
pub fn process_spec(json: &str) -> Result<SpecProcessor, ProcessError> {
    let spec: OpenApiSpec = serde_json::from_str(json).map_err(ProcessError::Json)?;
    Ok(SpecProcessor::new(Arc::new(spec)))
}

/// Convenience function to process and normalize a spec.
pub fn normalize_spec(json: &str) -> Result<NormalizedSpec, ProcessError> {
    let processor = process_spec(json)?;
    Ok(processor.normalize())
}

/// Error type for spec processing.
#[derive(Debug, derive_more::Display)]
pub enum ProcessError {
    /// JSON parse error
    #[display("JSON parse error: {_0}")]
    Json(serde_json::Error),

    /// Invalid OpenAPI spec structure
    #[display("Invalid OpenAPI spec: {_0}")]
    InvalidSpec(String),

    /// Unresolved $ref
    #[display("Unresolved $ref: {_0}")]
    UnresolvedRef(String),

    /// No base URL found in spec
    #[display("No base URL found in spec")]
    NoBaseUrl,

    /// No endpoints found
    #[display("No endpoints found in spec")]
    NoEndpoints,
}

impl From<serde_json::Error> for ProcessError {
    fn from(err: serde_json::Error) -> Self {
        ProcessError::Json(err)
    }
}

impl std::error::Error for ProcessError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_simple_spec() {
        let spec_json = json!({
            "openapi": "3.0.0",
            "info": { "title": "Test API", "version": "1.0.0" },
            "servers": [{ "url": "https://api.example.com" }],
            "paths": {
                "/v1/projects": {
                    "get": {
                        "operationId": "listProjects",
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    }
                }
            }
        });

        let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
        let processor = SpecProcessor::new(Arc::new(spec));
        let normalized = processor.normalize();

        assert_eq!(normalized.metadata.total_endpoints, 1);
        assert_eq!(normalized.metadata.api_title, "Test API");
        assert_eq!(normalized.metadata.api_version, "1.0.0");
    }

    #[test]
    fn to_normalized_json_produces_valid_json() {
        let spec_json = json!({
            "openapi": "3.0.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {}
        });

        let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
        let processor = SpecProcessor::new(Arc::new(spec));
        let json_output = processor.to_normalized_json();

        assert!(json_output.is_ok());
        let json_str = json_output.unwrap();
        assert!(json_str.contains("\"endpoints\""));
        assert!(json_str.contains("\"metadata\""));
    }
}
