//! WHY: Generates type-safe API client functions from OpenAPI specifications.
//!
//! WHAT: Reads OpenAPI specs from `artefacts/cloud_providers/`, uses foundation_openapi
//! for endpoint extraction, and generates Rust functions using `SimpleHttpClient`.
//!
//! HOW: For each endpoint, generates four functions:
//! - `{endpoint}_builder()` - Returns `ClientRequestBuilder` for customization
//! - `{endpoint}_task()` - Returns `TaskIterator` for composition/wrapping
//! - `{endpoint}_execute()` - Takes builder, returns `StreamIterator` via valtron
//! - `{endpoint}()` - Convenience function combining both
//!
//! Design Philosophy:
//! - No client structs - just plain functions
//! - No hidden state - pass `SimpleHttpClient` explicitly
//! - Use `build_send_request()` to get `SendRequestTask` directly
//! - Apply valtron combinators: `map_ready()`, `map_pending()`, `execute()`
//! - Task functions return `TaskIterator` for user composition before `execute()`

use foundation_openapi::{
    to_pascal_case, operation_id_to_fn_name, sanitize_doc_comment,
};
use regex::Regex;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, derive_more::Display)]
#[allow(dead_code)] // Some variants used for future extensions
pub enum GenClientError {
    #[display("failed to read {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[display("json parse error for {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },

    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[display("unsupported schema type {type_name}")]
    UnsupportedType { type_name: String },

    #[display("fmt error: {_0}")]
    FmtError(std::fmt::Error),

    #[display("parse failed: {_0}")]
    ParseFailed(String),
}

impl std::error::Error for GenClientError {}

impl From<std::io::Error> for GenClientError {
    fn from(e: std::io::Error) -> Self {
        GenClientError::ReadFile {
            path: String::new(),
            source: e,
        }
    }
}

impl From<serde_json::Error> for GenClientError {
    fn from(e: serde_json::Error) -> Self {
        GenClientError::Json {
            path: String::new(),
            source: e,
        }
    }
}

impl From<std::fmt::Error> for GenClientError {
    fn from(e: std::fmt::Error) -> Self {
        GenClientError::FmtError(e)
    }
}

// ---------------------------------------------------------------------------
// Intermediate representation
// ---------------------------------------------------------------------------

/// Represents an API endpoint for code generation.
#[derive(Debug, Clone)]
struct ApiEndpoint {
    /// Full path like `/projects/{project}/services`
    path: String,
    /// HTTP method
    method: String,
    /// OpenAPI operationId if present
    operation_id: Option<String>,
    /// Summary from OpenAPI
    summary: Option<String>,
    /// Path parameters like `project` from `{project}`
    path_params: Vec<ParameterInfo>,
    /// Query parameters
    query_params: Vec<ParameterInfo>,
    /// Request body type name if present (e.g., `Service`)
    request_body_type: Option<String>,
    /// Response body type name (e.g., `ListServicesResponse`)
    response_type: Option<String>,
    /// Base URL from OpenAPI spec (servers[0].url or baseUrl)
    base_url: Option<String>,
    /// For GCP: list of placeholder names in flatPath order (e.g., ["sitesId"])
    /// Used to map path template placeholders to parameter names
    path_placeholders: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields reserved for future query parameter support
struct ParameterInfo {
    name: String,           // Sanitized Rust-safe name
    original_name: String,  // Original API parameter name for query strings
    rust_type: String,
    required: bool,
    description: Option<String>,
}

// ---------------------------------------------------------------------------
// Client generator
// ---------------------------------------------------------------------------

pub struct ClientGenerator {
    artefacts_dir: PathBuf,
    output_dir: PathBuf,
}

impl ClientGenerator {
    pub fn new(artefacts_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            artefacts_dir,
            output_dir,
        }
    }

    /// Generate clients for all providers.
    pub fn generate_all(&self) -> Result<(), GenClientError> {
        let providers = self.discover_providers()?;
        tracing::info!("Found {} providers: {:?}", providers.len(), providers);

        for provider in &providers {
            self.generate_for_provider(provider)?;
        }

        Ok(())
    }

    /// Generate clients for a single provider.
    pub fn generate_for_provider(&self, provider: &str) -> Result<(), GenClientError> {
        tracing::info!("Generating clients for: {}", provider);
        let provider_dir = self.artefacts_dir.join(provider);

        // Discover sub-APIs
        let sub_apis = self.discover_sub_apis(&provider_dir);

        let provider_output_dir = self.output_dir.join(provider).join("clients");
        std::fs::create_dir_all(&provider_output_dir)?;

        // Generate types.rs (shared types for all providers)
        self.generate_shared_types(&provider_output_dir, provider)?;

        if sub_apis.is_empty() {
            // Single spec: provider/openapi.json -> clients/mod.rs
            let spec_path = provider_dir.join("openapi.json");
            if spec_path.exists() {
                if let Err(e) = self.generate_clients_for_spec(
                    provider,
                    &spec_path,
                    &provider_output_dir.join("mod.rs"),
                ) {
                    tracing::warn!("    Failed to generate client for {}: {}", provider, e);
                }
            }
        } else {
            // Multi-API: generate one file per API, then mod.rs
            for api_name in &sub_apis {
                let spec_path = provider_dir.join(api_name).join("openapi.json");
                let output_path = provider_output_dir.join(format!("{api_name}.rs"));
                if let Err(e) = self.generate_clients_for_spec(
                    &format!("{}/{}", provider, api_name),
                    &spec_path,
                    &output_path,
                ) {
                    tracing::warn!("    Failed to generate client for {}/{}: {}", provider, api_name, e);
                }
            }
            // Generate mod.rs
            self.generate_mod_rs(provider, &sub_apis, &provider_output_dir.join("mod.rs"))?;
        }

        // Format the generated code
        self.format_directory(&provider_output_dir)?;

        Ok(())
    }

    fn discover_providers(&self) -> Result<Vec<String>, GenClientError> {
        let mut providers = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.artefacts_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Check if this is a provider (has openapi.json or sub-APIs)
                    if entry.path().join("openapi.json").exists()
                        || !self.discover_sub_apis(&entry.path()).is_empty()
                    {
                        providers.push(name);
                    }
                }
            }
        }
        providers.sort();
        Ok(providers)
    }

    fn discover_sub_apis(&self, provider_dir: &Path) -> Vec<String> {
        let mut apis = Vec::new();
        if let Ok(entries) = std::fs::read_dir(provider_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.path().join("openapi.json").exists() {
                        apis.push(name);
                    }
                }
            }
        }
        apis.sort();
        apis
    }

    fn generate_clients_for_spec(
        &self,
        label: &str,
        spec_path: &Path,
        output_path: &Path,
    ) -> Result<(), GenClientError> {
        tracing::info!("  Processing: {} ({})", label, spec_path.display());

        let content = std::fs::read_to_string(spec_path)?;

        // Check if this is an error response (not a valid spec)
        // Error responses have ONLY an "error" field with no other spec content
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = json.as_object() {
                // If the only key is "error" and it has no baseUrl, resources, paths, or servers, it's an error response
                if obj.len() == 1
                    && obj.contains_key("error")
                    && !obj.contains_key("baseUrl")
                    && !obj.contains_key("resources")
                    && !obj.contains_key("paths")
                    && !obj.contains_key("servers")
                {
                    // Extract error details for logging
                    let error_msg = obj.get("error")
                        .and_then(|e| e.as_object())
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    let error_status = obj.get("error")
                        .and_then(|e| e.as_object())
                        .and_then(|e| e.get("status"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("UNKNOWN");
                    tracing::warn!("    Spec returned error: {} - {}", error_status, error_msg);
                    // Write empty module with a note - error types are in types.rs
                    let mut out = String::new();
                    writeln!(out, "//! Auto-generated API clients for {}.", label)?;
                    writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
                    writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
                    writeln!(out, "//!")?;
                    writeln!(out, "//! Feature flag: `{}`", Self::label_to_feature_name(label))?;
                    writeln!(out, "//! NOTE: API spec unavailable - only error types are available in types.rs")?;
                    writeln!(out)?;
                    writeln!(out, "#![cfg(feature = \"{}\")]", Self::label_to_feature_name(label))?;
                    std::fs::write(output_path, out)?;
                    return Ok(());
                }

                // Check if spec has paths/resources but is missing base URL
                let has_paths = obj.contains_key("paths") || obj.contains_key("resources");
                let has_base_url = obj.contains_key("baseUrl")
                    || obj.contains_key("servers")
                    || (obj.contains_key("rootUrl") && obj.contains_key("servicePath"));
                if has_paths && !has_base_url {
                    tracing::warn!("    Spec missing base URL (no 'servers', 'baseUrl', or 'rootUrl'+'servicePath') - skipping");
                    // Write empty module with a note
                    let mut out = String::new();
                    writeln!(out, "//! Auto-generated API clients for {}.", label)?;
                    writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
                    writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
                    writeln!(out, "//!")?;
                    writeln!(out, "//! Feature flag: `{}`", Self::label_to_feature_name(label))?;
                    writeln!(out, "//! NOTE: Spec missing base URL - add 'servers' or 'baseUrl' to the OpenAPI spec")?;
                    writeln!(out)?;
                    writeln!(out, "#![cfg(feature = \"{}\")]", Self::label_to_feature_name(label))?;
                    std::fs::write(output_path, out)?;
                    return Ok(());
                }
            }
        }

        // Use foundation_openapi for spec parsing and endpoint extraction
        let processor = foundation_openapi::process_spec(&content)
            .or_else(|_| {
                // Try unwrapping from nested structure (e.g., {"openapi.json": {...}})
                let wrapped: serde_json::Value = serde_json::from_str(&content)
                    .map_err(foundation_openapi::ProcessError::Json)?;
                if let Some(obj) = wrapped.as_object() {
                    for key in ["openapi.json", "openapi", "spec"] {
                        if let Some(inner) = obj.get(key) {
                            if let Ok(proc) = foundation_openapi::process_spec(&inner.to_string()) {
                                return Ok(proc);
                            }
                        }
                    }
                }
                Err(foundation_openapi::ProcessError::InvalidSpec("Failed to parse spec".to_string()))
            })
            .map_err(|e| GenClientError::ParseFailed(format!("OpenAPI parse error: {}", e)))?;

        // Get endpoints from foundation_openapi
        let endpoints_foundation = processor.endpoints();

        // Convert to ApiEndpoint format for codegen
        let endpoints: Vec<ApiEndpoint> = endpoints_foundation
            .into_iter()
            .map(|ep| {
                // Extract path placeholders from path template
                let placeholder_re = Regex::new(r"\{(\+)?([^}]+)\}").unwrap();
                let path_placeholders: Vec<String> = placeholder_re
                    .captures_iter(&ep.path)
                    .map(|cap| cap[2].to_string())
                    .collect();

                // Extract base URL
                let base_url = processor.base_url();

                ApiEndpoint {
                    path: ep.path,
                    method: ep.method,
                    operation_id: Some(ep.operation_id),
                    summary: ep.summary,
                    path_params: ep.path_params.iter().map(|p| ParameterInfo {
                        name: p.replace(['-', '.', '~', '/', '@', ':', '<', '>', '[', ']'], "_"),
                        original_name: p.clone(),
                        rust_type: "String".to_string(),
                        required: true,
                        description: None,
                    }).collect(),
                    query_params: ep.query_params.iter().map(|p| ParameterInfo {
                        name: p.replace(['-', '.', '~', '/', '@', ':', '<', '>', '[', ']'], "_"),
                        original_name: p.clone(),
                        rust_type: "String".to_string(),
                        required: false,
                        description: None,
                    }).collect(),
                    request_body_type: ep.request_type,
                    response_type: ep.response_type.map(|rt| rt.as_rust_type().to_string()),
                    base_url,
                    path_placeholders,
                }
            })
            .collect();
        tracing::info!("    Found {} endpoints", endpoints.len());

        if endpoints.is_empty() {
            tracing::warn!("    No endpoints found, skipping");
            // Write empty module
            let mut out = String::new();
            writeln!(out, "//! Auto-generated API clients for {}.", label)?;
            writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
            writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
            writeln!(out, "//!")?;
            writeln!(out, "//! Feature flag: `{}`", Self::label_to_feature_name(label))?;
            writeln!(out)?;
            writeln!(out, "#![cfg(feature = \"{}\")]", Self::label_to_feature_name(label))?;
            std::fs::write(output_path, out)?;
            return Ok(());
        }

        // Generate code
        let mut out = String::new();
        let feature_name = Self::label_to_feature_name(label);
        let provider_module = label.split('/').next().unwrap_or(label).replace('-', "_");

        // File header
        writeln!(out, "//! Auto-generated API clients for {}.", label)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Feature flag: `{}`", feature_name)?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}\")]", feature_name)?;
        writeln!(out)?;

        // =============================================================================
        // DEDUPLICATION PHASE: Collect unique items to generate
        // =============================================================================

        // Track unique endpoint signatures (path + method) to identify true duplicates
        let mut seen_signatures = std::collections::HashSet::new();
        // Track unique function names (builder, task, execute, convenience) - for final validation
        let mut seen_func_names = std::collections::HashSet::new();
        // Track function names -> list of endpoints that share this base name
        // Key: base fn_name, Value: Vec of (endpoint, suffix_count)
        let mut fn_name_to_endpoints: std::collections::HashMap<String, Vec<(&ApiEndpoint, Option<usize>)>> = std::collections::HashMap::new();
        // Track unique Args structs by name - map struct name -> endpoint that defines it
        let mut args_struct_map: std::collections::HashMap<String, &ApiEndpoint> = std::collections::HashMap::new();
        // Track which endpoints can use convenience fn (Args doesn't conflict)
        let mut endpoints_with_valid_convenience = std::collections::HashSet::new();
        // Map of final fn_name -> endpoint for code generation
        let mut final_endpoints: Vec<(&ApiEndpoint, String)> = Vec::new();

        for endpoint in &endpoints {
            let base_fn_name = self.endpoint_to_fn_name(endpoint);
            let signature = format!("{}:{}", endpoint.path, endpoint.method);

            // Check if this is an exact duplicate (same path + method)
            if seen_signatures.contains(&signature) {
                tracing::debug!("    Skipping exact duplicate endpoint {}:{} (already processed)",
                    endpoint.method, endpoint.path);
                continue;
            }
            seen_signatures.insert(signature);

            // Check for function name collision with different endpoints
            let final_fn_name = if let Some(existing) = fn_name_to_endpoints.get_mut(&base_fn_name) {
                // Collision detected - check if it's same or different endpoint
                // Since we already filtered exact duplicates above, this is a different endpoint
                // Add a numeric suffix to differentiate
                let suffix = existing.len() + 1;
                let new_fn_name = format!("{}_{}", base_fn_name, suffix);
                existing.push((endpoint, Some(suffix)));
                tracing::debug!("    Endpoint {}: function name '{}' collides - using '{}'",
                    endpoint.path, base_fn_name, new_fn_name);
                new_fn_name
            } else {
                // No collision - first endpoint with this base name
                fn_name_to_endpoints.insert(base_fn_name.clone(), vec![(endpoint, None)]);
                base_fn_name
            };

            // Register function names with the final name (may include suffix)
            let func_names = vec![
                format!("{}_builder", final_fn_name),
                format!("{}_task", final_fn_name),
                format!("{}_execute", final_fn_name),
                final_fn_name.clone(),
            ];

            for name in func_names {
                // These should be unique now since we added suffix
                if seen_func_names.contains(&name) {
                    tracing::warn!("    Unexpected duplicate function name: {}", name);
                }
                seen_func_names.insert(name);
            }

            // Check Args struct - if same struct name, reuse existing definition
            let has_params = !endpoint.path_params.is_empty()
                || !endpoint.query_params.is_empty()
                || endpoint.request_body_type.is_some();

            if has_params {
                // Use the final fn_name (with suffix if needed) for Args struct
                let args_struct_name = format!("{}Args", to_pascal_case(&final_fn_name));

                // Check if this Args struct conflicts with a response/request type
                let conflicts_with_type = endpoint.response_type.as_ref().is_some_and(|t| t == &args_struct_name)
                    || endpoint.request_body_type.as_ref().is_some_and(|t| t == &args_struct_name);

                if conflicts_with_type {
                    tracing::warn!("    Endpoint {}: Args struct '{}' conflicts with existing type - skipping convenience function",
                        endpoint.path, args_struct_name);
                } else if args_struct_map.contains_key(&args_struct_name) {
                    // Same Args struct already defined by another endpoint - that's fine, reuse it
                    tracing::debug!("    Endpoint {}: reusing Args struct '{}' from another endpoint",
                        endpoint.path, args_struct_name);
                    endpoints_with_valid_convenience.insert(final_fn_name.clone());
                } else {
                    // New unique Args struct
                    args_struct_map.insert(args_struct_name, endpoint);
                    endpoints_with_valid_convenience.insert(final_fn_name.clone());
                }
            } else {
                // No params = no Args struct needed
                endpoints_with_valid_convenience.insert(final_fn_name.clone());
            }

            final_endpoints.push((endpoint, final_fn_name));
        }

        tracing::info!("    Validated {} unique endpoints ({} with name suffixes), {} unique Args structs",
            final_endpoints.len(),
            final_endpoints.iter().filter(|(_, n)| n.contains('_') && !n.ends_with("_execute") && !n.ends_with("_builder") && !n.ends_with("_task")).count(),
            args_struct_map.len());

        // =============================================================================
        // CODE GENERATION PHASE
        // =============================================================================

        // Imports - write each import only once
        writeln!(out, "use crate::providers::{}::clients::types::*;", provider_module)?;
        writeln!(out, "use foundation_core::valtron::{{execute, BoxedSendExecutionAction, StreamIterator, StreamIteratorExt, TaskIterator, TaskIteratorExt}};")?;
        writeln!(out, "use foundation_core::wire::simple_http::client::{{")?;
        writeln!(out, "    body_reader, ClientRequestBuilder, DnsResolver, RequestIntro, SimpleHttpClient, SystemDnsResolver,")?;
        writeln!(out, "}};")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out, "use serde::Serialize;")?;
        // Only import resources if we have response types that need them
        let used_types: std::collections::HashSet<String> = final_endpoints
            .iter()
            .filter_map(|(ep, _)| ep.response_type.clone())
            .collect();
        if !used_types.is_empty() {
            writeln!(out, "use crate::providers::{}::resources::*;", provider_module)?;
        }
        writeln!(out, "use foundation_db::state::resource_identifier::ResourceIdentifier;")?;
        writeln!(out)?;

        // Track generated Args structs for convenience functions
        let mut generated_args_structs: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Generate functions for each unique endpoint
        // Endpoints with Args struct conflicts skip the convenience function only
        for (endpoint, fn_name) in &final_endpoints {
            let skip_convenience = !endpoints_with_valid_convenience.contains(fn_name);

            self.generate_builder_fn(&mut out, endpoint, fn_name)?;
            self.generate_task_fn(&mut out, endpoint, fn_name)?;
            self.generate_execute_fn(&mut out, endpoint, fn_name)?;

            if !skip_convenience {
                self.generate_convenience_fn(&mut out, endpoint, fn_name, &mut generated_args_structs)?;
            } else {
                tracing::debug!("    Skipping convenience function for {} (Args conflict)", fn_name);
            }
        }

        // Generate ResourceIdentifier implementations for each endpoint
        for (endpoint, fn_name) in &final_endpoints {
            if let Some(response_type) = &endpoint.response_type {
                self.generate_resource_identifier_impl(&mut out, endpoint, response_type, fn_name, label)?;
            }
        }

        // =============================================================================
        // POST-GENERATION VALIDATION SUMMARY
        // =============================================================================

        // Count generated items for logging
        let convenience_count = endpoints_with_valid_convenience.len();
        let total_funcs = (final_endpoints.len() * 3) + convenience_count; // builder, task, execute + optional convenience
        let total_args = generated_args_structs.len();

        tracing::info!("    Generated: {} endpoints, {} functions ({} builder, {} task, {} execute, {} convenience), {} Args structs",
            final_endpoints.len(),
            total_funcs,
            final_endpoints.len(),  // builder functions
            final_endpoints.len(),  // task functions
            final_endpoints.len(),  // execute functions
            convenience_count,       // convenience functions
            total_args);

        let skipped_convenience = final_endpoints.len() - endpoints_with_valid_convenience.len();
        if skipped_convenience > 0 {
            tracing::warn!("    Skipped {} convenience functions due to Args struct conflicts",
                skipped_convenience);
        }

        tracing::info!("    Types used: {} types", used_types.len());
        if !generated_args_structs.is_empty() {
            tracing::info!("    Args structs: {} generated", generated_args_structs.len());
        }

        // Write output
        std::fs::write(output_path, out)?;
        tracing::info!("    Written: {}", output_path.display());

        Ok(())
    }

    fn generate_builder_fn(&self, out: &mut String, endpoint: &ApiEndpoint, fn_name: &str) -> Result<(), GenClientError> {

        // Doc comment
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", sanitize_doc_comment(summary, true))?;
        }
        writeln!(out, "///")?;
        writeln!(out, "/// Returns `ClientRequestBuilder` for customization.")?;
        writeln!(out, "/// Use `{}_execute()` to send, or `{}` for simplest API.", fn_name, fn_name)?;
        writeln!(out)?;

        // Function signature - generic over DNS resolver type
        write!(out, "pub fn {}_builder<R>(\n    client: &SimpleHttpClient<R>,", fn_name)?;

        // Path parameters - borrowed references
        for param in &endpoint.path_params {
            writeln!(out)?;
            write!(out, "    {}: &{},", self.escape_keyword(&param.name), param.rust_type)?;
        }

        // Query parameters (optional) - borrowed references
        for param in &endpoint.query_params {
            writeln!(out)?;
            // For Vec types, take &Vec; for other types, take reference
            let param_type = &param.rust_type;
            write!(out, "    {}: &Option<{}>,", self.escape_keyword(&param.name), param_type)?;
        }

        // Request body
        if let Some(body_type) = &endpoint.request_body_type {
            writeln!(out)?;
            write!(out, "    body: &{}", body_type)?;
        }

        writeln!(out)?;
        writeln!(out, ") -> Result<ClientRequestBuilder<R>, ApiError>")?;
        writeln!(out, "where")?;
        writeln!(out, "    R: DnsResolver + Clone,")?;
        writeln!(out, "{{")?;

        // Build URL
        writeln!(out)?;
        writeln!(out, "    // Build URL")?;

        // Build URL format string by replacing placeholders with {}
        // Replace placeholders in order of appearance, passing params positionally
        let mut url_format = endpoint.path.clone();

        // For each placeholder in order, replace its first occurrence with {}
        // and track which param to pass for it
        let mut params_to_pass: Vec<&str> = Vec::new();
        let mut used_params: std::collections::HashSet<&str> = std::collections::HashSet::new();

        for placeholder in endpoint.path_placeholders.iter() {
            let placeholder_pattern = format!("{{{}}}", placeholder);
            if let Some(pos) = url_format.find(&placeholder_pattern) {
                let before = &url_format[..pos];
                let after = &url_format[pos + placeholder_pattern.len()..];
                url_format = format!("{}{{}}{}", before, after);

                // Find the matching param - try exact match first, then substring match
                let matched_param = endpoint.path_params.iter()
                    .find(|p| {
                        // Skip already used params
                        if used_params.contains(p.name.as_str()) {
                            return false;
                        }
                        // Exact match
                        &p.name == placeholder || &p.original_name == placeholder
                    })
                    .or_else(|| {
                        // Substring match - check if placeholder is contained in param name or vice versa
                        endpoint.path_params.iter().find(|p| {
                            if used_params.contains(p.name.as_str()) {
                                return false;
                            }
                            let placeholder_lower = placeholder.to_lowercase();
                            let name_lower = p.name.to_lowercase();
                            let original_lower = p.original_name.to_lowercase();
                            placeholder_lower.contains(&name_lower)
                                || name_lower.contains(&placeholder_lower)
                                || placeholder_lower.contains(&original_lower)
                                || original_lower.contains(&placeholder_lower)
                        })
                    });

                if let Some(param) = matched_param {
                    params_to_pass.push(&param.name);
                    used_params.insert(param.name.as_str());
                }
            }
        }

        // Fallback: if we still have placeholders but no params matched, use positional matching
        // This handles cases where placeholder names don't match param names at all
        let placeholder_count = url_format.matches("{}").count();
        if placeholder_count > params_to_pass.len() {
            // Reset and use simple positional matching
            url_format = endpoint.path.clone();
            params_to_pass.clear();
            used_params.clear();

            // Count placeholders in path and match params positionally
            let placeholder_re = Regex::new(r"\{[^}]+\}").unwrap();
            let placeholder_positions: Vec<_> = placeholder_re.find_iter(&endpoint.path).collect();

            for (i, param) in endpoint.path_params.iter().enumerate() {
                if i < placeholder_positions.len() {
                    params_to_pass.push(&param.name);
                    url_format = placeholder_re.replace(url_format.as_str(), "{}").to_string();
                }
            }
        }

        let base_url = endpoint.base_url.as_deref().unwrap_or("https://api.example.com");
        writeln!(out, "    let endpoint_url = format!(")?;
        writeln!(out, "        \"{}{}\",", base_url, url_format)?;
        // Pass path parameters as references - they all implement Display or AsRef<str>
        for param_name in &params_to_pass {
            let escaped = self.escape_keyword(param_name);
            writeln!(out, "        {},", escaped)?;
        }
        writeln!(out, "    );")?;

        // Build request
        writeln!(out)?;
        writeln!(out, "    // Build request")?;

        // Build query string if there are query parameters
        if !endpoint.query_params.is_empty() {
            writeln!(out, "    let mut query_parts = Vec::new();")?;
            for param in &endpoint.query_params {
                let param_name = &param.name;
                let original_name = &param.original_name;
                let rust_type = &param.rust_type;
                let escaped_name = self.escape_keyword(param_name);

                // Check if this is a Vec type - handle array params differently
                if rust_type.starts_with("Vec<") {
                    // For array params: if let Some(vals) = param { for val in vals { ... } }
                    writeln!(out, "    if let Some(vals) = {}.as_ref() {{", escaped_name)?;
                    writeln!(out, "        for val in vals {{")?;
                    writeln!(out, "            query_parts.push(format!(\"{}={{}}\", val));", original_name)?;
                    writeln!(out, "        }}")?;
                    writeln!(out, "    }}")?;
                } else {
                    writeln!(out, "    if let Some(val) = {}.as_ref() {{", escaped_name)?;
                    // For String types, use val directly; for others, use format
                    // Note: Both branches are identical currently, keeping for future extension
                    writeln!(out, "        query_parts.push(format!(\"{}={{}}\", val));", original_name)?;
                    writeln!(out, "    }}")?;
                }
            }
            writeln!(out)?;
            writeln!(out, "    let url_with_query = if query_parts.is_empty() {{")?;
            writeln!(out, "        endpoint_url")?;
            writeln!(out, "    }} else {{")?;
            writeln!(out, "        format!(\"{{}}?{{}}\", endpoint_url, query_parts.join(\"&\"))")?;
            writeln!(out, "    }};")?;
            writeln!(out)?;
            let method_lower = endpoint.method.to_lowercase();
            writeln!(out, "    let builder = client.{}(&url_with_query)", method_lower)?;
        } else {
            let method_lower = endpoint.method.to_lowercase();
            writeln!(out, "    let builder = client.{}(&endpoint_url)", method_lower)?;
        }
        writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;")?;

        // Add request body
        if endpoint.request_body_type.is_some() {
            writeln!(out)?;
            writeln!(out, "    builder.body_json(body)")?;
            writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))")?;
        } else {
            writeln!(out)?;
            writeln!(out, "    Ok(builder)")?;
        }

        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    fn generate_task_fn(&self, out: &mut String, endpoint: &ApiEndpoint, fn_name: &str) -> Result<(), GenClientError> {
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");

        // Doc comment explaining use cases
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", sanitize_doc_comment(summary, true))?;
        }
        writeln!(out, "///")?;
        writeln!(out, "/// Takes a `ClientRequestBuilder`, builds the request, applies valtron combinators,")?;
        writeln!(out, "/// and returns a `TaskIterator` for customization before execution.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// Use this function when you need to:")?;
        writeln!(out, "/// - Wrap the task with custom valtron combinators")?;
        writeln!(out, "/// - Compose multiple tasks before execution")?;
        writeln!(out, "/// - Intercept task execution for logging or testing")?;
        writeln!(out, "///")?;
        writeln!(out, "/// For direct execution, use `{}_execute()` or `{}`.", fn_name, fn_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Arguments")?;
        writeln!(out, "///")?;
        writeln!(out, "/// * `builder` - A `ClientRequestBuilder`, typically from `{}_builder()`", fn_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Errors")?;
        writeln!(out, "///")?;
        writeln!(out, "/// Returns an error if the request cannot be built.")?;
        writeln!(out)?;

        // Function signature - takes builder only
        writeln!(out, "pub fn {}_task(", fn_name)?;
        writeln!(out, "    builder: ClientRequestBuilder<SystemDnsResolver>,")?;
        writeln!(out, ") -> Result<")?;
        writeln!(out, "    impl TaskIterator<")?;
        writeln!(out, "        Ready = Result<ApiResponse<{}>, ApiError>,", return_type)?;
        writeln!(out, "        Pending = ApiPending,")?;
        writeln!(out, "        Spawner = BoxedSendExecutionAction")?;
        writeln!(out, "    > + Send + 'static,")?;
        writeln!(out, "    ApiError,")?;
        writeln!(out, "> {{")?;

        // Valtron combinator chain - NO execute() call
        writeln!(out)?;
        writeln!(out, "    Ok(builder")?;
        writeln!(out, "        .build_send_request()")?;
        writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?")?;
        writeln!(out, "        .map_ready(|intro| match intro {{")?;
        writeln!(out, "            RequestIntro::Success {{ stream, intro, headers, .. }} => {{")?;
        writeln!(out, "                let status_code: usize = intro.0.into();")?;
        writeln!(out)?;
        writeln!(out, "                if status_code < 200 || status_code >= 300 {{")?;
        writeln!(out, "                    // Capture body for error parsing")?;
        writeln!(out, "                    let body = body_reader::collect_string(stream);")?;
        writeln!(out, "                    // Try to parse as structured API error")?;
        writeln!(out, "                    if let Ok(error_body) = serde_json::from_str::<ApiErrorBody>(&body) {{")?;
        writeln!(out, "                        return Err(ApiError::ApiError(error_body.error));")?;
        writeln!(out, "                    }}")?;
        writeln!(out, "                    // Fall back to raw HTTP status error")?;
        writeln!(out, "                    return Err(ApiError::HttpStatus {{")?;
        writeln!(out, "                        code: status_code as u16,")?;
        writeln!(out, "                        headers: headers.clone(),")?;
        writeln!(out, "                        body: Some(body),")?;
        writeln!(out, "                    }});")?;
        writeln!(out, "                }}")?;
        writeln!(out)?;
        writeln!(out, "                let body = body_reader::collect_string(stream);")?;

        if return_type == "()" {
            writeln!(out, "                Ok(ApiResponse {{")?;
            writeln!(out, "                    status: status_code as u16,")?;
            writeln!(out, "                    headers: headers.clone(),")?;
            writeln!(out, "                    body: (),")?;
            writeln!(out, "                }})")?;
        } else {
            writeln!(out, "                let parsed: {} = serde_json::from_str(&body)", return_type)?;
            writeln!(out, "                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;")?;
            writeln!(out)?;
            writeln!(out, "                Ok(ApiResponse {{")?;
            writeln!(out, "                    status: status_code as u16,")?;
            writeln!(out, "                    headers: headers.clone(),")?;
            writeln!(out, "                    body: parsed,")?;
            writeln!(out, "                }})")?;
        }

        writeln!(out, "            }}")?;
        writeln!(out, "            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),")?;
        writeln!(out, "        }})")?;
        writeln!(out, "        .map_pending(|_| ApiPending::Sending))")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    fn generate_execute_fn(&self, out: &mut String, endpoint: &ApiEndpoint, fn_name: &str) -> Result<(), GenClientError> {
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");

        // Doc comment - updated to mention task function
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", sanitize_doc_comment(summary, true))?;
        }
        writeln!(out, "///")?;
        writeln!(out, "/// Takes a `ClientRequestBuilder`, builds and executes the request,")?;
        writeln!(out, "/// and returns the parsed response via a `StreamIterator`.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// For full customization, use `{}_builder()` to create the builder,", fn_name)?;
        writeln!(out, "/// modify it, then call this function with your customized builder.")?;
        writeln!(out, "/// For task-level control, use `{}_task()`.", fn_name)?;
        writeln!(out, "/// For the simplest API, use `{}()`.", fn_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Arguments")?;
        writeln!(out, "///")?;
        writeln!(out, "/// * `builder` - A `ClientRequestBuilder`, typically from `{}_builder()`", fn_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Errors")?;
        writeln!(out, "///")?;
        writeln!(out, "/// Returns an error if the request cannot be built.")?;
        writeln!(out, "/// HTTP errors during execution are returned via the StreamIterator.")?;
        writeln!(out)?;

        // Function signature
        writeln!(out, "pub fn {}_execute(", fn_name)?;
        writeln!(out, "    builder: ClientRequestBuilder<SystemDnsResolver>,")?;
        writeln!(out, ") -> Result<")?;
        writeln!(out, "    impl StreamIterator<")?;
        writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type)?;
        writeln!(out, "        P = ApiPending")?;
        writeln!(out, "    > + Send + 'static,")?;
        writeln!(out, "    ApiError,")?;
        writeln!(out, "> {{")?;

        // Delegate to task function
        writeln!(out)?;
        writeln!(out, "    let task = {}_task(builder)?;", fn_name)?;
        writeln!(out, "    execute(task, None).map_err(|e| ApiError::RequestBuildFailed(e.to_string()))")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    fn generate_convenience_fn(
        &self,
        out: &mut String,
        endpoint: &ApiEndpoint,
        fn_name: &str,
        generated_args_structs: &mut std::collections::HashSet<String>,
    ) -> Result<(), GenClientError> {
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");
        let struct_name = format!("{}Args", to_pascal_case(fn_name));

        // Generate argument struct with JsonHash
        let has_params = !endpoint.path_params.is_empty()
            || !endpoint.query_params.is_empty()
            || endpoint.request_body_type.is_some();

        // Only generate Args struct if we haven't already generated it
        let should_generate_args = has_params && !generated_args_structs.contains(&struct_name);

        if should_generate_args {
            writeln!(out, "/// Arguments for [`{}`].", fn_name)?;
            writeln!(out, "#[derive(Debug, Clone, Serialize, JsonHash)]")?;
            writeln!(out, "pub struct {} {{", struct_name)?;

            for param in &endpoint.path_params {
                writeln!(out, "    /// Path parameter: {}", param.name)?;
                writeln!(out, "    pub {}: {},", self.escape_keyword(&param.name), param.rust_type)?;
            }

            for param in &endpoint.query_params {
                writeln!(out, "    /// Query parameter: {}", param.name)?;
                writeln!(out, "    pub {}: Option<{}>,", self.escape_keyword(&param.name), param.rust_type)?;
            }

            if let Some(body_type) = &endpoint.request_body_type {
                writeln!(out, "    /// Request body.")?;
                writeln!(out, "    pub body: {},", body_type)?;
            }

            writeln!(out, "}}")?;
            writeln!(out)?;

            // Track that we generated this Args struct
            generated_args_structs.insert(struct_name.clone());
        }

        // Doc comment
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", sanitize_doc_comment(summary, true))?;
        }
        writeln!(out, "///")?;
        writeln!(out, "/// Simplest API - builds and executes the request in one call.")?;
        writeln!(out, "/// For customization, use `{}_builder()` + `{}_execute()`.", fn_name, fn_name)?;
        writeln!(out, "/// For task-level control, use `{}_task()`.", fn_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Errors")?;
        writeln!(out, "///")?;
        writeln!(out, "/// Returns an error if the request cannot be built.")?;
        writeln!(out)?;

        // Function signature
        if has_params {
            writeln!(out, "pub fn {}(", fn_name)?;
            writeln!(out, "    client: &SimpleHttpClient,")?;
            writeln!(out, "    args: &{},", struct_name)?;
            writeln!(out, ") -> Result<")?;
        } else {
            writeln!(out, "pub fn {}(", fn_name)?;
            writeln!(out, "    client: &SimpleHttpClient,")?;
            writeln!(out, ") -> Result<")?;
        }

        writeln!(out, "    impl StreamIterator<")?;
        writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type)?;
        writeln!(out, "        P = ApiPending")?;
        writeln!(out, "    > + Send + 'static,")?;
        writeln!(out, "    ApiError,")?;
        writeln!(out, "> {{")?;

        // Call builder then execute - pass references
        writeln!(out)?;
        write!(out, "    let builder = {}_builder(client", fn_name)?;
        for param in &endpoint.path_params {
            write!(out, ", &args.{}", self.escape_keyword(&param.name))?;
        }
        for param in &endpoint.query_params {
            // Pass references since builder takes &Option<T>
            write!(out, ", &args.{}", self.escape_keyword(&param.name))?;
        }
        if endpoint.request_body_type.is_some() {
            write!(out, ", &args.body")?;
        }
        writeln!(out, ")?;")?;
        writeln!(out, "    {}_execute(builder)", fn_name)?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    /// Generate ResourceIdentifier trait implementation for an endpoint's response type.
    ///
    /// This allows the response type to be used with StoreStateIdentifierTask for automatic
    /// state tracking.
    fn generate_resource_identifier_impl(
        &self,
        out: &mut String,
        endpoint: &ApiEndpoint,
        response_type: &str,
        fn_name: &str,
        label: &str,
    ) -> Result<(), GenClientError> {
        // Skip endpoints without params - they don't have Args types
        let has_params = !endpoint.path_params.is_empty()
            || !endpoint.query_params.is_empty()
            || endpoint.request_body_type.is_some();
        if !has_params {
            return Ok(());
        }

        let args_struct_name = format!("{}Args", to_pascal_case(fn_name));

        // Extract provider and kind from the label and response type
        // e.g., "gcp/cloudkms" -> provider="gcp", kind_prefix="gcp::cloudkms"
        let (provider, kind_prefix) = if let Some(pos) = label.find('/') {
            (&label[..pos], &format!("{}::{}", &label[..pos].replace('-', "_"), &label[pos+1..].replace('-', "_")))
        } else {
            (label, &label.replace('-', "_"))
        };

        // Build resource kind: e.g., "gcp::cloudkms::AutokeyConfig"
        let resource_kind = format!("{}::{}", kind_prefix, response_type);

        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// ResourceIdentifier implementation for {}", response_type)?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;
        writeln!(out, "/// ResourceIdentifier implementation for {} with {} input.", response_type, args_struct_name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// WHY: Enables automatic state tracking via StoreStateIdentifierTask.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// HOW: Computes resource ID from input path parameters.")?;
        writeln!(out, "impl ResourceIdentifier<{}> for {} {{", args_struct_name, response_type)?;
        writeln!(out, "    fn generate_resource_id(&self, input: &{}) -> String {{", args_struct_name)?;

        // Generate resource ID from path parameters
        // e.g., for path "/v1/folders/{folderName}/autokeyConfig"
        // and resource "AutokeyConfig", generate:
        // format!("gcp::cloudkms::AutokeyConfig/folders/{}", input.folder_name)
        if !endpoint.path_params.is_empty() {
            write!(out, "        format!(\"{}", resource_kind)?;
            for _param in &endpoint.path_params {
                write!(out, "/{{}}")?;
            }
            write!(out, "\"")?;
            for param in &endpoint.path_params {
                write!(out, ", input.{}", self.escape_keyword(&param.name))?;
            }
            writeln!(out, ")")?;
        } else {
            // No path parameters - just use the resource kind
            writeln!(out, "        \"{}\".to_string()", resource_kind)?;
        }

        writeln!(out, "    }}")?;
        writeln!(out)?;
        writeln!(out, "    fn resource_kind(&self) -> &'static str {{")?;
        writeln!(out, "        \"{}\"", resource_kind)?;
        writeln!(out, "    }}")?;
        writeln!(out)?;
        writeln!(out, "    fn provider(&self) -> &'static str {{")?;
        writeln!(out, "        \"{}\"", provider)?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    fn generate_shared_types(&self, output_dir: &Path, provider: &str) -> Result<(), GenClientError> {
        let types_path = output_dir.join("types.rs");
        let mut out = String::new();

        writeln!(out, "//! Shared types for {} API clients.", provider)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Feature flag: `{}`", provider)?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}\")]", provider)?;
        writeln!(out)?;
        writeln!(out, "use foundation_core::wire::simple_http::SimpleHeaders;")?;
        writeln!(out)?;
        writeln!(out, "use serde::{{Deserialize, Serialize}};")?;
        writeln!(out)?;

        // ApiErrorDetails - represents structured API error responses
        writeln!(out, "/// Structured error response from cloud provider APIs.")?;
        writeln!(out, "/// Matches Google Cloud Error format: https://cloud.google.com/apis/design/errors")?;
        writeln!(out, "#[derive(Debug, Clone, Deserialize, Serialize)]")?;
        writeln!(out, "pub struct ApiErrorDetails {{")?;
        writeln!(out, "    /// HTTP status code")?;
        writeln!(out, "    pub code: i32,")?;
        writeln!(out, "    /// Human-readable error message")?;
        writeln!(out, "    pub message: String,")?;
        writeln!(out, "    /// Error status string (e.g., \"INVALID_ARGUMENT\", \"NOT_FOUND\")")?;
        writeln!(out, "    pub status: String,")?;
        writeln!(out, "    /// Additional error details (provider-specific)")?;
        writeln!(out, "    #[serde(skip_serializing_if = \"Option::is_none\")]")?;
        writeln!(out, "    pub details: Option<serde_json::Value>,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(out, "impl std::fmt::Display for ApiErrorDetails {{")?;
        writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")?;
        writeln!(out, "        write!(f, \"{{}}: {{}}\", self.status, self.message)")?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(out, "/// Wrapper for API error responses matching Google Cloud format.")?;
        writeln!(out, "#[derive(Debug, Clone, Deserialize, Serialize)]")?;
        writeln!(out, "pub struct ApiErrorBody {{")?;
        writeln!(out, "    pub error: ApiErrorDetails,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        // ApiResponse
        writeln!(out, "/// Generic API response with status, headers, and parsed body.")?;
        writeln!(out, "#[derive(Debug, Clone)]")?;
        writeln!(out, "pub struct ApiResponse<T> {{")?;
        writeln!(out, "    pub status: u16,")?;
        writeln!(out, "    pub headers: SimpleHeaders,")?;
        writeln!(out, "    pub body: T,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        // ApiError
        writeln!(out, "/// Provider-agnostic error type for API operations.")?;
        writeln!(out, "#[derive(Debug)]")?;
        writeln!(out, "pub enum ApiError {{")?;
        writeln!(out, "    RequestBuildFailed(String),")?;
        writeln!(out, "    RequestSendFailed(String),")?;
        writeln!(out, "    HttpStatus {{ code: u16, headers: SimpleHeaders, body: Option<String> }},")?;
        writeln!(out, "    ApiError(ApiErrorDetails),")?;
        writeln!(out, "    ParseFailed(String),")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(out, "impl std::fmt::Display for ApiError {{")?;
        writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")?;
        writeln!(out, "        match self {{")?;
        writeln!(out, "            ApiError::RequestBuildFailed(e) => write!(f, \"request build failed: {{}}\", e),")?;
        writeln!(out, "            ApiError::RequestSendFailed(e) => write!(f, \"request send failed: {{}}\", e),")?;
        writeln!(out, "            ApiError::HttpStatus {{ code, body, .. }} => {{")?;
        writeln!(out, "                write!(f, \"HTTP status {{}}\", code)?;")?;
        writeln!(out, "                if let Some(b) = body {{")?;
        writeln!(out, "                    write!(f, \": {{}}\", b)?;")?;
        writeln!(out, "                }}")?;
        writeln!(out, "                Ok(())")?;
        writeln!(out, "            }}")?;
        writeln!(out, "            ApiError::ApiError(e) => write!(f, \"API error: {{}}\", e),")?;
        writeln!(out, "            ApiError::ParseFailed(e) => write!(f, \"parse failed: {{}}\", e),")?;
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(out, "impl std::error::Error for ApiError {{}}")?;
        writeln!(out)?;

        // ApiPending
        writeln!(out, "/// Progress states for API operations.")?;
        writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]")?;
        writeln!(out, "pub enum ApiPending {{")?;
        writeln!(out, "    Building,")?;
        writeln!(out, "    Sending,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        std::fs::write(&types_path, out)?;
        tracing::info!("    Generated: {}", types_path.display());

        Ok(())
    }

    fn generate_mod_rs(&self, provider: &str, sub_apis: &[String], output_path: &Path) -> Result<(), GenClientError> {
        let mut out = String::new();

        writeln!(out, "//! Auto-generated API clients for {}.", provider)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_provider_clients`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Feature flag: `{}`", provider)?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}\")]", provider)?;
        writeln!(out)?;
        writeln!(out, "pub mod types;")?;
        writeln!(out)?;

        for api in sub_apis {
            writeln!(out, "pub mod {};", api)?;
        }

        std::fs::write(output_path, out)?;
        Ok(())
    }

    fn endpoint_to_fn_name(&self, endpoint: &ApiEndpoint) -> String {
        operation_id_to_fn_name(endpoint.operation_id.as_deref(), &endpoint.method, &endpoint.path)
    }

    fn label_to_feature_name(label: &str) -> String {
        label.split('/').next().unwrap_or(label).replace('-', "_")
    }

    fn escape_keyword(&self, name: &str) -> String {
        match name {
            "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
            | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
            | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
            | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "async" | "await" | "dyn" | "override" => format!("{}_rs", name),
            _ => name.to_string(),
        }
    }

    fn format_directory(&self, dir: &Path) -> Result<(), GenClientError> {
        tracing::info!("  Formatting: {}", dir.display());

        // Find all .rs files
        let mut rust_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "rs") {
                    rust_files.push(path);
                }
            }
        }

        if rust_files.is_empty() {
            return Ok(());
        }

        // Run rustfmt on each file
        let output = Command::new("rustfmt")
            .args(&rust_files)
            .output()
            .map_err(|e| GenClientError::WriteFile {
                path: format!("rustfmt {}", dir.display()),
                source: std::io::Error::other(
                    format!("rustfmt failed: {}", e),
                ),
            })?;

        if !output.status.success() {
            tracing::warn!(
                "  rustfmt warnings:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }
}
