//! Minimal `OpenAPI` spec structures for deserialization.
//!
//! WHY: We only need a subset of `OpenAPI` for endpoint extraction.
//! Minimal structures reduce memory and parsing overhead.
//!
//! WHAT: Core `OpenAPI` 3.x and GCP Discovery structures needed for processing.
//!
//! HOW: Uses serde for flexible JSON deserialization.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Root `OpenAPI` specification (handles multiple formats).
#[derive(Debug, Deserialize, Clone)]
pub struct OpenApiSpec {
    /// `OpenAPI` version (e.g., "3.0.1") - optional for GCP Discovery format
    #[serde(default)]
    pub openapi: Option<String>,

    /// API metadata - optional for GCP Discovery format
    #[serde(default)]
    pub info: Option<Info>,

    /// Server URLs (`OpenAPI` 3.x)
    #[serde(default)]
    pub servers: Option<Vec<Server>>,

    /// Path items with operations
    #[serde(default)]
    pub paths: BTreeMap<String, PathItem>,

    /// Component definitions
    #[serde(default)]
    pub components: Option<Components>,

    // GCP Discovery Document fields
    /// Base URL for API endpoints
    #[serde(default, rename = "baseUrl")]
    pub base_url: Option<String>,

    /// Root URL (GCP Discovery)
    #[serde(default, rename = "rootUrl")]
    pub root_url: Option<String>,

    /// Service path (GCP Discovery)
    #[serde(default, rename = "servicePath")]
    pub service_path: Option<String>,

    /// Schema definitions (GCP Discovery format)
    #[serde(default)]
    pub schemas: Option<BTreeMap<String, Schema>>,

    /// Resource definitions (GCP Discovery format)
    #[serde(default)]
    pub resources: Option<BTreeMap<String, Resource>>,
}

impl OpenApiSpec {
    /// Get the base URL from the spec (handles multiple formats).
    #[must_use] 
    pub fn base_url(&self) -> Option<String> {
        self.servers
            .as_ref()
            .and_then(|servers| servers.first())
            .map(|s| s.url.clone())
            .or_else(|| self.base_url.clone())
            .or_else(|| {
                match (&self.root_url, &self.service_path) {
                    (Some(root), Some(service)) => Some(format!("{root}{service}")),
                    (Some(root), None) => Some(root.clone()),
                    (None, Some(service)) => Some(service.clone()),
                    (None, None) => None,
                }
            })
    }

    /// Get all schemas from the spec (both `OpenAPI` and GCP formats).
    #[must_use] 
    pub fn all_schemas(&self) -> BTreeMap<String, &Schema> {
        let mut schemas = BTreeMap::new();

        // Standard OpenAPI: components/schemas
        if let Some(components) = &self.components {
            for (name, schema) in &components.schemas {
                schemas.insert(name.clone(), schema);
            }
        }

        // GCP Discovery: top-level schemas
        if let Some(schemas_map) = &self.schemas {
            for (name, schema) in schemas_map {
                schemas.insert(name.clone(), schema);
            }
        }

        schemas
    }

    /// Detect the spec format.
    #[must_use] 
    pub fn format(&self) -> SpecFormat {
        if self.resources.is_some() || (self.schemas.is_some() && self.components.is_none()) {
            SpecFormat::GcpDiscovery
        } else if self.components.is_some() || !self.paths.is_empty() {
            SpecFormat::OpenApi3x
        } else {
            SpecFormat::Unknown
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Info {
    pub title: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub options: Option<Operation>,
    #[serde(default)]
    pub head: Option<Operation>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Operation {
    #[serde(default, rename = "operationId")]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<Vec<Parameter>>,
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<RequestBody>,
    #[serde(default)]
    pub responses: BTreeMap<String, Response>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub deprecated: bool,

    // GCP Discovery: method-specific fields
    #[serde(default, rename = "flatPath")]
    pub flat_path: Option<String>,
    #[serde(default, rename = "path")]
    pub gcp_path: Option<String>,
    #[serde(default, rename = "id")]
    pub gcp_id: Option<String>,
    #[serde(default, rename = "parameterOrder")]
    pub parameter_order: Option<Vec<String>>,
    #[serde(default)]
    pub response: Option<GcpResponseRef>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Parameter {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "in")]
    pub parameter_in: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub schema: Option<Schema>,
    #[serde(default)]
    pub description: Option<String>,
    // GCP Discovery
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RequestBody {
    #[serde(default)]
    pub content: Option<BTreeMap<String, MediaType>>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Response {
    pub description: Option<String>,
    pub content: Option<BTreeMap<String, MediaType>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MediaType {
    pub schema: Option<Schema>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Components {
    #[serde(default)]
    pub schemas: BTreeMap<String, Schema>,
}

/// GCP Discovery Document resource with nested methods.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Resource {
    pub methods: Option<BTreeMap<String, GcpMethod>>,
    pub resources: Option<BTreeMap<String, Resource>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GcpMethod {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "flatPath")]
    pub flat_path: Option<String>,
    #[serde(default, rename = "httpMethod")]
    pub http_method: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<BTreeMap<String, GcpParameter>>,
    #[serde(default, rename = "parameterOrder")]
    pub parameter_order: Option<Vec<String>>,
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<GcpRequestBodyRef>,
    #[serde(default)]
    pub response: Option<GcpResponseRef>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GcpParameter {
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GcpRequestBodyRef {
    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GcpResponseRef {
    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,
}

/// JSON Schema structure for type resolution.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Schema {
    #[serde(default, deserialize_with = "deserialize_type_field")]
    #[serde(rename = "type")]
    pub schema_type: Option<String>,

    #[serde(default)]
    pub format: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub required: Vec<String>,

    #[serde(default)]
    pub properties: Option<BTreeMap<String, Schema>>,

    #[serde(default)]
    pub items: Option<Box<Schema>>,

    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,

    #[serde(default, rename = "allOf")]
    pub all_of: Option<Vec<Schema>>,

    #[serde(default, rename = "oneOf")]
    pub one_of: Option<Vec<Schema>>,

    #[serde(default, rename = "anyOf")]
    pub any_of: Option<Vec<Schema>>,

    #[serde(default)]
    pub default: Option<serde_json::Value>,

    #[serde(default)]
    pub example: Option<serde_json::Value>,

    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<serde_json::Value>>,

    #[serde(default, rename = "const")]
    pub const_value: Option<serde_json::Value>,

    // OpenAPI 3.1 additionalProperties (can be boolean or schema)
    #[serde(default, rename = "additionalProperties", deserialize_with = "deserialize_additional_properties")]
    pub additional_properties: Option<bool>,
}

/// Deserialize `additionalProperties` which can be a boolean or a schema object.
fn deserialize_additional_properties<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    // First try to deserialize as a boolean
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match opt {
        Some(serde_json::Value::Bool(b)) => Some(b),
        Some(serde_json::Value::Null) | None => None,
        // Schema object or any other value is treated as `true`.
        Some(_) => Some(true),
    })
}

/// Deserialize the `type` field which can be a string or an array of strings.
///
/// `OpenAPI` 3.0 uses `"type": "string"`.
/// `OpenAPI` 3.1 uses `"type": ["string", "null"]` for nullable types.
///
/// For arrays, we extract the non-null type (e.g. `["string", "null"]` becomes `"string"`).
fn deserialize_type_field<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TypeField {
        String(String),
        Array(Vec<String>),
    }

    let opt = Option::<TypeField>::deserialize(deserializer)?;
    Ok(match opt {
        Some(TypeField::String(s)) => Some(s),
        Some(TypeField::Array(arr)) => {
            // For nullable types like ["string", "null"], extract the non-null type
            let non_null: Vec<&String> = arr.iter().filter(|s| s != &"null").collect();
            if non_null.len() == 1 {
                Some(non_null[0].clone())
            } else if non_null.is_empty() {
                None
            } else {
                // Multiple non-null types - use the first one
                Some(non_null[0].clone())
            }
        }
        None => None,
    })
}

/// Spec format discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecFormat {
    OpenApi3x,
    GcpDiscovery,
    Consolidated,
    Unknown,
}
