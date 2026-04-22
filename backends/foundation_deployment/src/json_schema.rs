//! Shared JSON Schema and `OpenAPI` response type extraction utilities.
//!
//! WHY: Both the resource generator (bin/platform) and generated clients need to
//! extract response types from `OpenAPI` specs. Centralizing this logic avoids
//! duplicating complex composition type handling.
//!
//! WHAT: Functions to extract response type names from `OpenAPI` operation responses,
//! handling $ref references, composition types (allOf/oneOf/anyOf), and edge cases.
//!
//! HOW: Analyzes response schemas to determine if they're generatable types or
//! should fall back to `serde_json::Value`.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Minimal schema structure for response type extraction.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct JsonSchema {
    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,
    #[serde(default)]
    pub properties: Option<BTreeMap<String, JsonSchema>>,
    #[serde(default, rename = "allOf")]
    pub all_of: Option<Vec<JsonSchema>>,
    #[serde(default, rename = "oneOf")]
    pub one_of: Option<Vec<JsonSchema>>,
    #[serde(default, rename = "anyOf")]
    pub any_of: Option<Vec<JsonSchema>>,
}

/// Response content structure for media type extraction.
#[derive(Debug, Deserialize, Default)]
pub struct ResponseContent {
    pub content: Option<BTreeMap<String, MediaType>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MediaType {
    pub schema: Option<JsonSchema>,
}

/// Extract the response type name from an `OpenAPI` operation's responses.
///
/// Returns:
/// - `Some(type_name)` for generatable types (objects with properties or simple types)
/// - `Some("serde_json::Value".to_string())` for composition-only types (oneOf/anyOf/allOf without properties)
/// - `Some("()".to_string())` for 204 No Content responses
/// - `None` if no suitable response type found
///
/// # Arguments
///
/// * `responses` - The responses object from an `OpenAPI` operation
/// * `components_schemas` - Optional schemas from components for resolving $ref targets
#[must_use]
pub fn extract_response_type(
    responses: &BTreeMap<String, ResponseContent>,
    components_schemas: Option<&BTreeMap<String, JsonSchema>>,
) -> Option<String> {
    // Look for 200, 201, 202, 204 responses in order of preference
    for status in &["200", "201", "202", "204", "default"] {
        if let Some(response) = responses.get(*status) {
            // 204 No Content
            if *status == "204" {
                return Some("()".to_string());
            }

            if let Some(content) = &response.content {
                if let Some(media) = content.get("application/json") {
                    if let Some(schema) = &media.schema {
                        // Check if this is a generatable type
                        let schema_to_check = if let Some(ref_path) = &schema.ref_path {
                            // It's a $ref, look up the actual schema
                            components_schemas
                                .and_then(|schemas| {
                                    let type_name =
                                        ref_path.trim_start_matches("#/components/schemas/");
                                    schemas.get(type_name)
                                })
                                .unwrap_or(schema)
                        } else {
                            schema
                        };

                        // Check if the type is generatable (has properties or simple structure)
                        let is_generatable = schema_to_check.properties.is_some()
                            || (schema_to_check.all_of.is_none()
                                && schema_to_check.any_of.is_none()
                                && schema_to_check.one_of.is_none());

                        if is_generatable {
                            return extract_type_name_from_schema(
                                schema_to_check,
                                components_schemas,
                            );
                        }
                        // Pure composition type - use serde_json::Value
                        return Some("serde_json::Value".to_string());
                    }
                }
            }
        }
    }
    None
}

/// Extract type name from a schema's $ref path.
///
/// Handles various reference formats:
/// - `#/components/schemas/ServiceName` → `ServiceName`
/// - `GoogleCloudRunV2Service` → `GoogleCloudRunV2Service`
pub fn extract_type_name_from_ref(ref_path: &str) -> Option<String> {
    // Handle OpenAPI format: #/components/schemas/TypeName
    if ref_path.starts_with("#/") {
        return ref_path.split('/').next_back().map(String::from);
    }

    // Handle GCP format: GoogleCloudRunV2Service (already the type name)
    ref_path.split('.').next_back().map(String::from)
}

/// Extract type name from a schema, resolving $ref if present.
fn extract_type_name_from_schema(
    schema: &JsonSchema,
    _components_schemas: Option<&BTreeMap<String, JsonSchema>>,
) -> Option<String> {
    if let Some(ref_path) = &schema.ref_path {
        let type_name = extract_type_name_from_ref(ref_path)?;

        // Apply PascalCase normalization for generated types
        Some(normalize_type_name(&type_name))
    } else {
        // Inline schema - no named type
        None
    }
}

/// Normalize a type name to match generated Rust type conventions.
///
/// Converts identifiers with dots, hyphens, underscores to `PascalCase`:
/// - `treasury.transaction` → `TreasuryTransaction`
/// - `Custom-pages` → `CustomPages`
/// - `iam_response_collection_accounts` → `IamResponseCollectionAccounts`
#[must_use]
pub fn normalize_type_name(name: &str) -> String {
    name.split(['.', '-', '@', '_'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + chars.as_str()
        })
        .collect()
}

/// Extract path parameter names from an `OpenAPI` path template.
///
/// Examples:
/// - `/v1/projects/{projectId}` → `["projectId"]`
/// - `/v1/folders/{folderId}/files/{fileId}` → `["folderId", "fileId"]`
///
/// # Panics
///
/// Panics if the regex pattern is invalid (which cannot happen with the current
/// static pattern `\{([^}]+)\}`).
#[must_use]
pub fn extract_path_params(path: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\{([^}]+)\}").unwrap();
    re.captures_iter(path)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_type_name_from_openapi_ref() {
        assert_eq!(
            extract_type_name_from_ref("#/components/schemas/ServiceName"),
            Some("ServiceName".to_string())
        );
    }

    #[test]
    fn extracts_type_name_from_gcp_ref() {
        assert_eq!(
            extract_type_name_from_ref("GoogleCloudRunV2Service"),
            Some("GoogleCloudRunV2Service".to_string())
        );
    }

    #[test]
    fn normalizes_dotted_names() {
        assert_eq!(
            normalize_type_name("treasury.transaction"),
            "TreasuryTransaction"
        );
    }

    #[test]
    fn normalizes_hyphenated_names() {
        assert_eq!(normalize_type_name("Custom-pages"), "CustomPages");
    }

    #[test]
    fn normalizes_snake_case_names() {
        assert_eq!(
            normalize_type_name("iam_response_collection"),
            "IamResponseCollection"
        );
    }

    #[test]
    fn extracts_path_params() {
        let params = extract_path_params("/v1/projects/{projectId}");
        assert_eq!(params, vec!["projectId".to_string()]);

        let params = extract_path_params("/v1/folders/{folderId}/files/{fileId}");
        assert_eq!(params, vec!["folderId".to_string(), "fileId".to_string()]);
    }
}
