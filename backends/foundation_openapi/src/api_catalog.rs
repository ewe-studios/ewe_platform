//! API catalog for code generation.
//!
//! WHY: Both client generation and provider wrapper generation need to know
//!      what APIs exist, their endpoints, and what functions to generate.
//!      This provides a single source of truth instead of regex parsing.
//!
//! WHAT: `ApiCatalog` struct with per-API endpoint listings and function names.
//!
//! HOW: Uses `foundation_openapi` spec processing to extract structured data
//!      about providers, APIs, endpoints, and their builder/task functions.

use crate::{process_spec, EndpointInfo, OperationType};
use std::path::Path;

/// Catalog of all APIs for a provider.
#[derive(Debug, Clone)]
pub struct ApiCatalog {
    /// Provider name (e.g., "gcp", "stripe")
    pub provider: String,
    /// List of APIs within this provider
    pub apis: Vec<ApiInfo>,
}

/// Information about a single API (e.g., "compute", "cloudkms" for GCP).
#[derive(Debug, Clone)]
pub struct ApiInfo {
    /// API name (e.g., "compute", "admin")
    pub name: String,
    /// Base URL for this API
    pub base_url: Option<String>,
    /// All endpoints in this API
    pub endpoints: Vec<EndpointInfo>,
}

impl ApiInfo {
    /// Get the builder function name for an endpoint.
    #[must_use] 
    pub fn builder_fn_name(&self, endpoint: &EndpointInfo) -> String {
        to_snake_case(&endpoint.operation_id) + "_builder"
    }

    /// Get the task function name for an endpoint.
    #[must_use] 
    pub fn task_fn_name(&self, endpoint: &EndpointInfo) -> String {
        to_snake_case(&endpoint.operation_id) + "_task"
    }

    /// Get the execute function name for an endpoint.
    #[must_use] 
    pub fn execute_fn_name(&self, endpoint: &EndpointInfo) -> String {
        to_snake_case(&endpoint.operation_id) + "_execute"
    }

    /// Get the convenience function name for an endpoint.
    #[must_use] 
    pub fn convenience_fn_name(&self, endpoint: &EndpointInfo) -> String {
        to_snake_case(&endpoint.operation_id)
    }

    /// Get the args struct name for an endpoint.
    #[must_use] 
    pub fn args_struct_name(&self, endpoint: &EndpointInfo) -> String {
        to_pascal_case(&endpoint.operation_id) + "Args"
    }
}

/// Builder for creating an `ApiCatalog` from various sources.
pub struct ApiCatalogBuilder {
    provider: String,
}

impl ApiCatalogBuilder {
    #[must_use] 
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
        }
    }

    /// Build catalog from a single `OpenAPI` spec file.
    ///
    /// # Errors
    ///
    /// Returns an error string if the file cannot be read from `spec_path`
    /// or if its contents cannot be parsed as an `OpenAPI` spec.
    pub fn from_spec_file(&self, spec_path: &Path) -> Result<ApiCatalog, String> {
        let content = std::fs::read_to_string(spec_path)
            .map_err(|e| format!("Failed to read spec: {e}"))?;

        self.from_spec_content(&content)
    }

    /// Build catalog from spec content string.
    ///
    /// # Errors
    ///
    /// Returns an error string if `content` cannot be parsed as an `OpenAPI`
    /// spec, including when wrapped in a known envelope object.
    pub fn from_spec_content(&self, content: &str) -> Result<ApiCatalog, String> {
        // Try direct processing first
        let processor = process_spec(content)
            .or_else(|_| {
                // Try unwrapping from nested structure (e.g., {"openapi.json": {...}})
                let wrapped: serde_json::Value = serde_json::from_str(content)
                    .map_err(|e| format!("JSON parse error: {e}"))?;
                if let Some(obj) = wrapped.as_object() {
                    for key in ["openapi.json", "openapi", "spec"] {
                        if let Some(inner) = obj.get(key) {
                            if let Ok(proc) = process_spec(&inner.to_string()) {
                                return Ok(proc);
                            }
                        }
                    }
                }
                Err("Failed to parse spec".to_string())
            })
            .map_err(|e| format!("Failed to process spec: {e}"))?;

        let endpoints = processor.endpoints();
        let base_url = processor.base_url();

        let api = ApiInfo {
            name: self.provider.clone(),
            base_url,
            endpoints,
        };

        Ok(ApiCatalog {
            provider: self.provider.clone(),
            apis: vec![api],
        })
    }

    /// Build catalog from multiple `OpenAPI` spec files (for multi-API providers like GCP).
    ///
    /// # Errors
    ///
    /// Returns an error string if any of the provided spec contents fail
    /// to parse. The error includes the offending API name.
    pub fn from_spec_files(
        &self,
        specs: Vec<(String, String)>, // (api_name, spec_content)
    ) -> Result<ApiCatalog, String> {
        let mut apis = Vec::new();

        for (api_name, content) in specs {
            let processor = process_spec(&content)
                .map_err(|e| format!("Failed to process spec for {api_name}: {e}"))?;

            let endpoints = processor.endpoints();
            let base_url = processor.base_url();

            apis.push(ApiInfo {
                name: api_name,
                base_url,
                endpoints,
            });
        }

        // Sort APIs by name for consistent ordering
        apis.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(ApiCatalog {
            provider: self.provider.clone(),
            apis,
        })
    }

    /// Build catalog from a directory of API specs (`provider/api_name/openapi.json`).
    /// Skips specs that fail to parse, logging a warning.
    ///
    /// # Errors
    ///
    /// Returns an error string if reading the directory or its single
    /// top-level `openapi.json` fails. Per-API parse failures are logged
    /// and skipped rather than propagated.
    pub fn from_provider_dir(&self, provider_dir: &Path) -> Result<ApiCatalog, String> {
        let mut apis = Vec::new();

        // Check for single spec at provider/openapi.json
        let single_spec = provider_dir.join("openapi.json");
        if single_spec.exists() {
            return self.from_spec_file(&single_spec);
        }

        // Look for sub-APIs in provider/api_name/openapi.json
        if let Ok(entries) = std::fs::read_dir(provider_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let api_name = entry.file_name().to_string_lossy().to_string();
                    let spec_path = entry.path().join("openapi.json");
                    if spec_path.exists() {
                        let content = match std::fs::read_to_string(&spec_path) {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("    Warning: Failed to read {}: {}", spec_path.display(), e);
                                continue;
                            }
                        };

                        // Try to process the spec, skip if it fails
                        let processor = match process_spec(&content)
                            .or_else(|_| {
                                let wrapped: serde_json::Value = serde_json::from_str(&content)
                                    .map_err(|e| format!("JSON parse error: {e}"))?;
                                if let Some(obj) = wrapped.as_object() {
                                    for key in ["openapi.json", "openapi", "spec"] {
                                        if let Some(inner) = obj.get(key) {
                                            if let Ok(proc) = process_spec(&inner.to_string()) {
                                                return Ok(proc);
                                            }
                                        }
                                    }
                                }
                                Err("Failed to parse spec".to_string())
                            }) {
                            Ok(p) => p,
                            Err(e) => {
                                eprintln!("    Warning: Failed to process {api_name}: {e}");
                                continue;
                            }
                        };

                        let endpoints = processor.endpoints();
                        let base_url = processor.base_url();

                        if !endpoints.is_empty() {
                            apis.push(ApiInfo {
                                name: api_name,
                                base_url,
                                endpoints,
                            });
                        }
                    }
                }
            }
        }

        // Sort APIs by name for consistent ordering
        apis.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(ApiCatalog {
            provider: self.provider.clone(),
            apis,
        })
    }
}

impl ApiCatalog {
    /// Create a catalog builder for the given provider.
    #[must_use] 
    pub fn builder(provider: &str) -> ApiCatalogBuilder {
        ApiCatalogBuilder::new(provider)
    }

    /// Get total number of endpoints across all APIs.
    #[must_use] 
    pub fn total_endpoints(&self) -> usize {
        self.apis.iter().map(|api| api.endpoints.len()).sum()
    }

    /// Get all endpoints across all APIs.
    pub fn all_endpoints(&self) -> impl Iterator<Item = &EndpointInfo> {
        self.apis.iter().flat_map(|api| &api.endpoints)
    }

    /// Get a specific API by name.
    #[must_use] 
    pub fn get_api(&self, name: &str) -> Option<&ApiInfo> {
        self.apis.iter().find(|api| api.name == name)
    }

    /// Get mutating endpoints (operations that require state tracking) for an API.
    #[must_use] 
    pub fn mutating_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| ep.operation_type.requires_state_tracking())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get read-only endpoints (operations that don't modify state) for an API.
    #[must_use] 
    pub fn read_only_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| !ep.operation_type.requires_state_tracking())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get create endpoints for an API.
    #[must_use] 
    pub fn create_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| matches!(ep.operation_type, OperationType::Create))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get read endpoints for an API.
    #[must_use] 
    pub fn read_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| matches!(ep.operation_type, OperationType::Read))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get update endpoints for an API.
    #[must_use] 
    pub fn update_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| matches!(ep.operation_type, OperationType::Update))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get delete endpoints for an API.
    #[must_use] 
    pub fn delete_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| matches!(ep.operation_type, OperationType::Delete))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get action endpoints for an API.
    #[must_use] 
    pub fn action_endpoints(&self, api_name: &str) -> Vec<&EndpointInfo> {
        self.get_api(api_name)
            .map(|api| {
                api.endpoints
                    .iter()
                    .filter(|ep| matches!(ep.operation_type, OperationType::Action(_)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Discovers providers from an artefacts directory.
///
/// A provider is a directory that either:
/// - Contains an openapi.json file directly, OR
/// - Contains subdirectories each with an openapi.json file
///
/// # Errors
///
/// Returns an [`std::io::Error`] if `artefacts_dir` exists but cannot be
/// read (e.g. permission denied). A non-existent path is treated as an
/// empty result and is not an error.
pub fn discover_providers(artefacts_dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut providers = Vec::new();

    if !artefacts_dir.exists() {
        return Ok(providers);
    }

    for entry in std::fs::read_dir(artefacts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(std::string::ToString::to_string);
            if let Some(name) = name {
                // Check if this is a provider (has openapi.json or sub-APIs)
                if path.join("openapi.json").exists() || has_sub_apis(&path) {
                    providers.push(name);
                }
            }
        }
    }

    providers.sort();
    Ok(providers)
}

/// Check if a provider directory has sub-APIs.
#[must_use] 
pub fn has_sub_apis(provider_dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(provider_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && entry.path().join("openapi.json").exists() {
                return true;
            }
        }
    }
    false
}

/// Convert a string to `PascalCase`.
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

            // Split on digit <-> letter and lower -> upper transitions.
            let boundary = !current.is_empty()
                && ((is_digit && !prev_was_digit)
                    || (is_lower && prev_was_digit)
                    || (is_upper && prev_was_lower));
            if boundary {
                sub_parts.push(current.clone());
                current.clear();
            }

            current.push(c);
            prev_was_digit = is_digit;
            prev_was_lower = is_lower;
        }

        if !current.is_empty() {
            sub_parts.push(current);
        }

        // Capitalize each sub-part
        for sub in sub_parts {
            if let Some(first) = sub.chars().next() {
                result.push_str(&first.to_uppercase().to_string());
                if sub.len() > 1 {
                    result.push_str(&sub[1..]);
                }
            }
        }
    }

    result
}

/// Convert `PascalCase` or camelCase to `snake_case`.
#[must_use] 
pub fn to_snake_case(s: &str) -> String {
    let pascal = to_pascal_case(s);

    // Then convert PascalCase to snake_case
    let mut result = String::new();
    for (i, c) in pascal.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert `snake_case` function name to sentence case for docs.
#[must_use] 
pub fn to_sentence_case(s: &str) -> String {
    let mut result = s.replace('_', " ");
    if let Some(first) = result.chars().next() {
        result.replace_range(..1, &first.to_uppercase().to_string());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("getV1ComputeServices"), "GetV1ComputeServices");
        assert_eq!(to_pascal_case("reports_activities_list"), "ReportsActivitiesList");
        assert_eq!(to_pascal_case("admin.channels.stop"), "AdminChannelsStop");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("getV1ComputeServices"), "get_v1_compute_services");
        assert_eq!(to_snake_case("ReportsActivitiesList"), "reports_activities_list");
    }
}
