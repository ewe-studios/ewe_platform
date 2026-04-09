//! Endpoint extraction from OpenAPI specs.
//!
//! WHY: Extract endpoint metadata from both standard OpenAPI paths and GCP Discovery resources.
//!
//! WHAT: EndpointExtractor with format-aware extraction logic.
//!
//! HOW: Iterates paths (OpenAPI) or resources (GCP) and extracts operation metadata.

use crate::spec::{OpenApiSpec, Operation, Response, GcpMethod, Schema};
use crate::endpoint::{EndpointInfo, ResponseType, GcpParameter};
use crate::type_resolver::TypeResolver;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// Extracts endpoints from OpenAPI specs.
pub struct EndpointExtractor {
    spec: Arc<OpenApiSpec>,
    resolver: TypeResolver,
}

impl EndpointExtractor {
    /// Create extractor from spec.
    pub fn new(spec: Arc<OpenApiSpec>) -> Self {
        let schemas = spec.all_schemas();
        let schema_map: BTreeMap<String, Schema> = schemas
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let resolver = TypeResolver::new(Arc::new(schema_map));
        Self { spec, resolver }
    }

    /// Extract all endpoints from the spec.
    /// Handles both OpenAPI 3.x and GCP Discovery formats.
    pub fn extract_all(&self) -> Vec<EndpointInfo> {
        let mut endpoints = Vec::new();

        // Try standard OpenAPI paths first
        endpoints.extend(self.extract_from_paths());

        // Try GCP Discovery resources
        endpoints.extend(self.extract_from_resources());

        // Deduplicate by operation_id + method
        let mut seen = HashSet::new();
        endpoints.retain(|e| {
            let key = format!("{}:{}", e.operation_id, e.method);
            seen.insert(key)
        });

        endpoints
    }

    /// Extract endpoints from standard OpenAPI paths.
    pub fn extract_from_paths(&self) -> Vec<EndpointInfo> {
        let mut endpoints = Vec::new();
        let base_url = self.spec.base_url();

        for (path, path_item) in &self.spec.paths {
            let methods = [
                ("GET", &path_item.get),
                ("POST", &path_item.post),
                ("PUT", &path_item.put),
                ("PATCH", &path_item.patch),
                ("DELETE", &path_item.delete),
            ];

            for (method_name, operation_opt) in methods {
                if let Some(operation) = operation_opt {
                    if let Some(info) = self.extract_operation(operation, path, method_name, &base_url) {
                        endpoints.push(info);
                    }
                }
            }
        }

        endpoints
    }

    /// Extract endpoints from GCP Discovery resources (recursive).
    pub fn extract_from_resources(&self) -> Vec<EndpointInfo> {
        let mut endpoints = Vec::new();
        let base_url = self.spec.base_url();

        if let Some(resources) = &self.spec.resources {
            self.extract_resources_recursive(resources, &mut endpoints, &base_url);
        }

        endpoints
    }

    /// Recursively extract endpoints from GCP resources.
    fn extract_resources_recursive(
        &self,
        resources: &BTreeMap<String, crate::spec::Resource>,
        endpoints: &mut Vec<EndpointInfo>,
        base_url: &Option<String>,
    ) {
        for (_resource_name, resource) in resources {
            // Extract methods from this resource
            if let Some(methods) = &resource.methods {
                for (_method_name, method) in methods {
                    if let Some(info) = self.extract_gcp_method(method, base_url) {
                        endpoints.push(info);
                    }
                }
            }

            // Recurse into nested resources
            if let Some(nested) = &resource.resources {
                self.extract_resources_recursive(nested, endpoints, base_url);
            }
        }
    }

    /// Extract endpoint info from an OpenAPI operation.
    fn extract_operation(
        &self,
        operation: &Operation,
        path: &str,
        method: &str,
        base_url: &Option<String>,
    ) -> Option<EndpointInfo> {
        let operation_id = operation.operation_id.clone()?;

        // Extract path parameters from path template
        let path_params = EndpointInfo::extract_path_params(path);

        // Extract parameters from parameters array
        let (mut param_path, param_query) = if let Some(parameters) = &operation.parameters {
            EndpointInfo::extract_parameters(parameters)
        } else {
            (Vec::new(), Vec::new())
        };

        // Use path template params if parameter array didn't provide path params
        if param_path.is_empty() && !path_params.is_empty() {
            param_path = path_params.clone();
        }

        // Extract request type
        let request_type = operation.request_body.as_ref()
            .and_then(|rb| rb.content.get("application/json"))
            .and_then(|media| media.schema.as_ref())
            .and_then(|schema| schema.ref_path.as_ref())
            .map(|ref_path| {
                let name = ref_path.trim_start_matches("#/components/schemas/");
                TypeResolver::rename_if_keyword(TypeResolver::to_pascal_case(name))
            });

        // Extract response type from success status codes
        let (response_type, success_codes, error_types) = self.extract_responses(&operation.responses);

        Some(EndpointInfo {
            operation_id,
            method: method.to_string(),
            path: path.to_string(),
            path_params: param_path,
            query_params: param_query,
            request_type,
            response_type,
            error_types,
            success_codes,
            base_url: base_url.clone(),
            summary: operation.summary.clone(),
        })
    }

    /// Extract responses from operation responses.
    fn extract_responses(
        &self,
        responses: &BTreeMap<String, Response>,
    ) -> (Option<ResponseType>, Vec<String>, BTreeMap<String, String>) {
        let mut response_type: Option<ResponseType> = None;
        let mut success_codes = Vec::new();
        let mut error_types = BTreeMap::new();

        let success_priority = ["200", "201", "202", "204", "default"];

        for (status, response) in responses {
            if status.starts_with('2') {
                success_codes.push(status.clone());

                // Get response type for first success code (priority order)
                if response_type.is_none() {
                    response_type = self.resolver.get_response_type(response);

                    // Handle 204 No Content specially
                    if status == "204" && response_type.is_none() {
                        response_type = Some(ResponseType::NoContent);
                    }
                }
            } else if status.starts_with(['4', '5']) {
                // Extract error type
                if let Some(content) = &response.content {
                    if let Some(media) = content.get("application/json") {
                        if let Some(schema) = &media.schema {
                            if let Some(ref_path) = &schema.ref_path {
                                let name = ref_path.trim_start_matches("#/components/schemas/");
                                let normalized = TypeResolver::to_pascal_case(name);
                                error_types.insert(status.clone(), TypeResolver::rename_if_keyword(normalized));
                            }
                        }
                    }
                }
            }
        }

        // Sort success codes by priority
        success_codes.sort_by(|a, b| {
            let a_idx = success_priority.iter().position(|s| s == a).unwrap_or(99);
            let b_idx = success_priority.iter().position(|s| s == b).unwrap_or(99);
            a_idx.cmp(&b_idx)
        });

        (response_type, success_codes, error_types)
    }

    /// Extract endpoint info from a GCP method.
    fn extract_gcp_method(
        &self,
        method: &GcpMethod,
        base_url: &Option<String>,
    ) -> Option<EndpointInfo> {
        let operation_id = method.id.clone()?;

        // Use flatPath for actual URL pattern (not path which is a template)
        let path = method.flat_path.as_deref().or(method.path.as_deref()).unwrap_or("");

        // Extract path parameters
        let _path_params = method.parameter_order.clone().unwrap_or_default();

        // Extract parameters
        let params: BTreeMap<String, GcpParameter> = method.parameters.as_ref()
            .map(|p| {
                p.iter()
                    .map(|(k, v)| {
                        (k.clone(), GcpParameter {
                            param_type: v.param_type.clone(),
                            format: v.format.clone(),
                            required: v.required,
                            description: v.description.clone(),
                            location: v.location.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let (param_path, param_query) = EndpointInfo::extract_gcp_parameters(
            method.parameter_order.as_deref(),
            &params,
        );

        // Extract request type
        let request_type = method.request_body.as_ref()
            .and_then(|rb| rb.ref_path.as_ref())
            .map(|ref_path| {
                let name = ref_path.trim_start_matches("#/components/schemas/");
                let name = name.trim_start_matches("#/schemas/");
                TypeResolver::rename_if_keyword(TypeResolver::to_pascal_case(name))
            });

        // Extract response type
        let response_type = method.response.as_ref()
            .and_then(|resp| resp.ref_path.as_ref())
            .map(|ref_path| {
                let name = ref_path.trim_start_matches("#/components/schemas/");
                let name = name.trim_start_matches("#/schemas/");
                let normalized = TypeResolver::to_pascal_case(name);
                ResponseType::Generated(TypeResolver::rename_if_keyword(normalized))
            });

        Some(EndpointInfo {
            operation_id,
            method: method.http_method.clone().unwrap_or_else(|| "GET".to_string()),
            path: path.to_string(),
            path_params: param_path,
            query_params: param_query,
            request_type,
            response_type,
            error_types: BTreeMap::new(), // GCP typically uses a common error type
            success_codes: vec!["200".to_string()],
            base_url: base_url.clone(),
            summary: method.description.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_from_simple_openapi() {
        let spec_json = json!({
            "openapi": "3.0.0",
            "info": { "title": "Test", "version": "1.0.0" },
            "servers": [{ "url": "https://api.example.com" }],
            "paths": {
                "/v1/projects": {
                    "get": {
                        "operationId": "listProjects",
                        "responses": {
                            "200": {
                                "description": "Success",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/ProjectsResponse" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "ProjectsResponse": {
                        "type": "object",
                        "properties": {
                            "items": { "type": "array", "items": { "type": "string" } }
                        }
                    }
                }
            }
        });

        let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
        let extractor = EndpointExtractor::new(Arc::new(spec));
        let endpoints = extractor.extract_all();

        assert_eq!(endpoints.len(), 1);
        let ep = &endpoints[0];
        assert_eq!(ep.operation_id, "listProjects");
        assert_eq!(ep.method, "GET");
        assert_eq!(ep.path, "/v1/projects");
        assert!(ep.response_type.is_some());
    }

    #[test]
    fn extracts_path_params_from_template() {
        let params = EndpointInfo::extract_path_params("/v1/projects/{projectId}");
        assert_eq!(params, vec!["projectId".to_string()]);
    }
}
