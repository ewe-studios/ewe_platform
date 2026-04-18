//! Endpoint information extracted from `OpenAPI` specs.
//!
//! WHY: We need a unified representation of endpoints with their request/response
//! types for code generation and runtime introspection.
//!
//! WHAT: `EndpointInfo` struct with all metadata needed for client generation.
//!
//! HOW: Extracted from `OpenAPI` paths or GCP Discovery resources.

use crate::spec::Parameter;
use std::collections::BTreeMap;

/// Classification of endpoint operation type.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Creates a new resource (e.g., createInstance, insertRow)
    Create,
    /// Reads/fetches resource state without modification (e.g., get, list, search)
    Read,
    /// Updates existing resource (e.g., update, patch, modify)
    Update,
    /// Removes resource (e.g., delete, remove, destroy)
    Delete,
    /// Action that may or may not modify state (e.g., cancel, trigger, invoke)
    Action(OperationEffect),
}

/// Effect classification for Action operations.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationEffect {
    /// Action modifies state (e.g., cancelOperation, startInstance)
    Mutating,
    /// Action is informational only (e.g., testIamPermissions, export)
    ReadOnly,
}

impl OperationType {
    /// Check if this operation type should wrap with `StoreStateIdentifierTask`.
    #[must_use] 
    pub fn requires_state_tracking(&self) -> bool {
        match self {
            OperationType::Create | OperationType::Update | OperationType::Delete => true,
            OperationType::Read => false,
            OperationType::Action(effect) => matches!(effect, OperationEffect::Mutating),
        }
    }
}

/// A single API endpoint extracted from an `OpenAPI` spec.
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    /// Operation ID (e.g., "getV1ComputeServices")
    pub operation_id: String,
    /// HTTP method (GET, POST, PUT, PATCH, DELETE)
    pub method: String,
    /// Path template (e.g., "/v1/compute-services/{id}")
    pub path: String,
    /// Path parameter names in order (e.g. `projectId`, `databaseId`).
    pub path_params: Vec<String>,
    /// Query parameter names
    pub query_params: Vec<String>,
    /// Request body type name (if any)
    pub request_type: Option<String>,
    /// Response type discriminator
    pub response_type: Option<ResponseType>,
    /// Error response types by status code
    pub error_types: BTreeMap<String, String>,
    /// Success status codes
    pub success_codes: Vec<String>,
    /// Base URL for this endpoint
    pub base_url: Option<String>,
    /// Summary/description
    pub summary: Option<String>,
    /// Operation type classification
    pub operation_type: OperationType,
    /// Whether this endpoint is deprecated
    pub deprecated: bool,
}

/// Response type discriminator.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseType {
    /// Generated struct type (e.g., "`GetProjectResponse`")
    Generated(String),
    /// Raw JSON value (for composition types like oneOf/anyOf)
    JsonValue,
    /// No content (204 responses)
    NoContent,
}

impl ResponseType {
    /// Get the Rust type string for this response type.
    #[must_use] 
    pub fn as_rust_type(&self) -> &str {
        match self {
            ResponseType::Generated(name) => name,
            ResponseType::JsonValue => "serde_json::Value",
            ResponseType::NoContent => "()",
        }
    }

    /// Check if this is a generatable type (not `JsonValue` or `NoContent`).
    #[must_use] 
    pub fn is_generated(&self) -> bool {
        matches!(self, ResponseType::Generated(_))
    }
}

impl EndpointInfo {
    /// Generate a struct name for the request arguments.
    #[must_use] 
    pub fn args_struct_name(&self) -> String {
        let pascal_case_op = Self::to_pascal_case(&self.operation_id);
        format!("{pascal_case_op}Args")
    }

    /// Generate a function name from the operation ID.
    #[must_use] 
    pub fn fn_name(&self) -> String {
        Self::to_snake_case(&self.operation_id)
    }

    /// Convert identifier to `PascalCase`.
    #[must_use] 
    pub fn to_pascal_case(s: &str) -> String {
        // First split by delimiters (., -, _, @)
        let parts: Vec<&str> = s.split(['.', '-', '_', '@']).collect();

        let mut result = String::new();

        for part in parts {
            // Within each part, split on case transitions and numbers
            let mut sub_parts = Vec::new();
            let mut current = String::new();
            let mut prev_was_digit = false;
            let mut prev_was_lower = false;

            for c in part.chars() {
                let is_digit = c.is_ascii_digit();
                let is_upper = c.is_ascii_uppercase();
                let is_lower = c.is_ascii_lowercase();

                // Split on digit -> letter or letter -> digit transitions
                if is_digit && !prev_was_digit && !current.is_empty() {
                    sub_parts.push(current.clone());
                    current.clear();
                } else if is_upper && prev_was_lower && !current.is_empty() {
                    // Split on camelCase transition
                    sub_parts.push(current.clone());
                    current.clear();
                } else if is_lower && prev_was_digit && !current.is_empty() {
                    // Split on digit -> letter transition
                    sub_parts.push(current.clone());
                    current.clear();
                }

                current.push(c);
                prev_was_digit = is_digit;
                prev_was_lower = is_lower && !is_upper;
            }

            if !current.is_empty() {
                sub_parts.push(current);
            }

            // Capitalize each sub-part
            for sub in sub_parts {
                let mut chars = sub.chars();
                if let Some(first) = chars.next() {
                    result.push(first.to_ascii_uppercase());
                    result.push_str(chars.as_str());
                }
            }
        }

        result
    }

    /// Convert identifier to `snake_case`.
    #[must_use] 
    pub fn to_snake_case(s: &str) -> String {
        let mut parts = Vec::new();
        let mut current = String::new();

        let chars: Vec<char> = s.chars().collect();
        for i in 0..chars.len() {
            let c = chars[i];
            if !c.is_alphanumeric() {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else if c.is_uppercase() {
                if !current.is_empty() {
                    let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
                    if next_is_lower || current.chars().last().is_some_and(char::is_lowercase) {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                current.push(c.to_ascii_lowercase());
            } else {
                current.push(c);
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts.join("_")
    }

    /// Extract path parameters from a path template.
    ///
    /// # Panics
    ///
    /// Panics if the internal placeholder regex (a fixed literal pattern)
    /// fails to compile, which is not expected to happen.
    #[must_use]
    pub fn extract_path_params(path: &str) -> Vec<String> {
        let re = regex::Regex::new(r"\{([^}]+)\}").expect("hard-coded placeholder regex must compile");
        re.captures_iter(path)
            .map(|cap| cap[1].to_string())
            .collect()
    }

    /// Extract parameters by location from `OpenAPI` operation parameters.
    #[must_use] 
    pub fn extract_parameters(parameters: &[Parameter]) -> (Vec<String>, Vec<String>) {
        let mut path_params = Vec::new();
        let mut query_params = Vec::new();

        for param in parameters {
            // Skip $ref parameters (they're resolved elsewhere)
            if param.ref_path.is_some() {
                continue;
            }

            // Skip parameters without names
            let name = match &param.name {
                Some(n) => n.clone(),
                None => continue,
            };

            match param.parameter_in.as_deref() {
                Some("query") => {
                    query_params.push(name);
                }
                Some("path") => {
                    path_params.push(name);
                }
                _ => {}
            }
        }

        (path_params, query_params)
    }

    /// Extract parameters by location from GCP method parameters.
    #[must_use] 
    pub fn extract_gcp_parameters(
        parameter_order: Option<&[String]>,
        params: &BTreeMap<String, GcpParameter>,
    ) -> (Vec<String>, Vec<String>) {
        let mut path_params = Vec::new();
        let mut query_params = Vec::new();

        // Use parameterOrder for path params in correct order
        if let Some(order) = parameter_order {
            for param_name in order {
                if let Some(param) = params.get(param_name) {
                    match param.location.as_deref() {
                        Some("query") => query_params.push(param_name.clone()),
                        // "path" or unspecified locations both default to a path parameter
                        // when listed in `parameterOrder`.
                        _ => path_params.push(param_name.clone()),
                    }
                }
            }
        }

        // Add remaining params as query params
        for (param_name, param) in params {
            if !path_params.contains(param_name) {
                if param.location.as_deref() == Some("path") {
                    // Already handled above as a path parameter.
                } else {
                    // "query" or unspecified locations default to query parameters.
                    query_params.push(param_name.clone());
                }
            }
        }

        (path_params, query_params)
    }
}

/// GCP Parameter structure for extraction.
#[derive(Debug, Clone)]
pub struct GcpParameter {
    pub param_type: Option<String>,
    pub format: Option<String>,
    pub required: bool,
    pub description: Option<String>,
    pub location: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_path_params() {
        let params = EndpointInfo::extract_path_params("/v1/projects/{projectId}");
        assert_eq!(params, vec!["projectId".to_string()]);

        let params =
            EndpointInfo::extract_path_params("/v1/folders/{folderId}/files/{fileId}");
        assert_eq!(params, vec!["folderId".to_string(), "fileId".to_string()]);
    }

    #[test]
    fn converts_to_pascal_case() {
        assert_eq!(EndpointInfo::to_pascal_case("getV1Projects"), "GetV1Projects");
        assert_eq!(EndpointInfo::to_pascal_case("get_v1_projects"), "GetV1Projects");
        assert_eq!(EndpointInfo::to_pascal_case("treasury.transaction"), "TreasuryTransaction");
        assert_eq!(EndpointInfo::to_pascal_case("Custom-pages"), "CustomPages");
    }

    #[test]
    fn converts_to_snake_case() {
        assert_eq!(EndpointInfo::to_snake_case("getV1Projects"), "get_v1_projects");
        assert_eq!(EndpointInfo::to_snake_case("GetV1Projects"), "get_v1_projects");
    }

    #[test]
    fn generates_args_struct_name() {
        let endpoint = EndpointInfo {
            operation_id: "getV1Projects".to_string(),
            method: "GET".to_string(),
            path: "/v1/projects".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Read,
            deprecated: false,
        };
        assert_eq!(endpoint.args_struct_name(), "GetV1ProjectsArgs");
    }

    #[test]
    fn generates_fn_name() {
        let endpoint = EndpointInfo {
            operation_id: "getV1Projects".to_string(),
            method: "GET".to_string(),
            path: "/v1/projects".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Read,
            deprecated: false,
        };
        assert_eq!(endpoint.fn_name(), "get_v1_projects");
    }
}
