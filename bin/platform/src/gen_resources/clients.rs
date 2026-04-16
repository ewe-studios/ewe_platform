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

use foundation_openapi::to_pascal_case;
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
                        rust_type: "Option<String>".to_string(),
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

        // Imports
        let provider_module = label.split('/').next().unwrap_or(label).replace('-', "_");
        writeln!(out, "use foundation_core::valtron::{{execute, BoxedSendExecutionAction, StreamIterator, StreamIteratorExt, TaskIterator, TaskIteratorExt}};")?;
        writeln!(out, "use foundation_core::wire::simple_http::client::{{")?;
        writeln!(out, "    body_reader, ClientRequestBuilder, DnsResolver, RequestIntro, SimpleHttpClient, SystemDnsResolver,")?;
        writeln!(out, "}};")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out, "use serde::Serialize;")?;
        writeln!(out, "use crate::providers::{}::clients::types::*;", provider_module)?;
        writeln!(out, "use crate::providers::{}::resources::*;", provider_module)?;
        writeln!(out, "use foundation_db::state::resource_identifier::ResourceIdentifier;")?;
        writeln!(out)?;

        // Deduplicate endpoints by all generated function names to avoid collisions.
        // Each endpoint generates 4 functions: {fn_name}_builder, {fn_name}_task, {fn_name}_execute, {fn_name}
        // We need to check all of these for potential collisions.
        let mut seen_names = std::collections::HashSet::new();
        let mut unique_endpoints = Vec::new();
        for endpoint in &endpoints {
            let fn_name = self.endpoint_to_fn_name(endpoint);

            // Generate all function names this endpoint would create
            let func_names = vec![
                format!("{}_builder", fn_name),
                format!("{}_task", fn_name),
                format!("{}_execute", fn_name),
                fn_name.clone(),
            ];

            // Check if any of these names would collide
            let would_collide = func_names.iter().any(|name| seen_names.contains(name));

            if !would_collide {
                // Add all function names to the seen set
                for name in func_names {
                    seen_names.insert(name);
                }
                unique_endpoints.push(endpoint);
            }
        }

        // Generate functions for each unique endpoint
        for endpoint in &unique_endpoints {
            self.generate_builder_fn(&mut out, endpoint)?;
            self.generate_task_fn(&mut out, endpoint)?;
            self.generate_execute_fn(&mut out, endpoint)?;
            self.generate_convenience_fn(&mut out, endpoint)?;
        }

        // Generate ResourceIdentifier implementations for each endpoint
        for endpoint in &unique_endpoints {
            if let Some(response_type) = &endpoint.response_type {
                self.generate_resource_identifier_impl(&mut out, endpoint, response_type, label)?;
            }
        }

        // Write output
        std::fs::write(output_path, out)?;
        tracing::info!("    Written: {}", output_path.display());

        Ok(())
    }

    fn generate_builder_fn(&self, out: &mut String, endpoint: &ApiEndpoint) -> Result<(), GenClientError> {
        let fn_name = self.endpoint_to_fn_name(endpoint);

        // Doc comment
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", self.sanitize_doc(summary, true))?;
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

    fn generate_task_fn(&self, out: &mut String, endpoint: &ApiEndpoint) -> Result<(), GenClientError> {
        let fn_name = self.endpoint_to_fn_name(endpoint);
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");

        // Doc comment explaining use cases
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", self.sanitize_doc(summary, true))?;
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

    fn generate_execute_fn(&self, out: &mut String, endpoint: &ApiEndpoint) -> Result<(), GenClientError> {
        let fn_name = self.endpoint_to_fn_name(endpoint);
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");

        // Doc comment - updated to mention task function
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", self.sanitize_doc(summary, true))?;
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

    fn generate_convenience_fn(&self, out: &mut String, endpoint: &ApiEndpoint) -> Result<(), GenClientError> {
        let fn_name = self.endpoint_to_fn_name(endpoint);
        let return_type = endpoint.response_type.as_deref().unwrap_or("()");
        let struct_name = format!("{}Args", to_pascal_case(&fn_name));

        // Generate argument struct with JsonHash
        let has_params = !endpoint.path_params.is_empty()
            || !endpoint.query_params.is_empty()
            || endpoint.request_body_type.is_some();

        if has_params {
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
        }

        // Doc comment
        writeln!(out, "/// {} {}", endpoint.method, endpoint.path)?;
        if let Some(summary) = &endpoint.summary {
            writeln!(out, "/// {}", self.sanitize_doc(summary, true))?;
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
        label: &str,
    ) -> Result<(), GenClientError> {
        let fn_name = self.endpoint_to_fn_name(endpoint);
        let args_struct_name = format!("{}Args", to_pascal_case(&fn_name));

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
        if let Some(ref op_id) = endpoint.operation_id {
            // Replace special characters with underscores before converting to snake case
            let clean_id = op_id
                .replace(['-', '.', '@', ':', '<', '>', '[', ']', '(', ')', '\'', ',', '~', '/'], "_");  // Replace slashes
            return self.to_snake_case(&clean_id);
        }

        // Generate from method + path
        let path_part = endpoint
            .path
            .trim_matches('/')
            .replace('/', "_")
            .replace(['{', '}'], "")
            .replace(['-', '.', '@', ':', '<', '>', '[', ']'], "_") // Replace ] in path
            .replace(['(', ')', '\''], "") // Remove apostrophes from path
            .replace([',', '~', '/'], "_"); // Replace slashes in path
        format!("{}_{}", endpoint.method.to_lowercase(), path_part)
    }

    fn to_snake_case(&self, s: &str) -> String {
        // First replace special characters with underscores
        let normalized = s
            .replace(['-', '.', '@', ':', '<', '>', '[', ']', '(', ')', '\'', ',', '~', '/'], "_");  // Replace slashes

        let mut result = String::new();
        let mut prev_was_upper = false;
        let mut prev_was_underscore = false;

        for (i, c) in normalized.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 && !prev_was_upper && !prev_was_underscore {
                    result.push('_');
                }
                result.push(c.to_lowercase().next().unwrap());
                prev_was_upper = true;
                prev_was_underscore = false;
            } else if c.is_numeric() {
                result.push(c);
                prev_was_upper = false;
                prev_was_underscore = false;
            } else if c == '_' {
                // Skip consecutive underscores
                if !prev_was_underscore {
                    result.push('_');
                }
                prev_was_upper = false;
                prev_was_underscore = true;
            } else {
                result.push(c);
                prev_was_upper = false;
                prev_was_underscore = false;
            }
        }

        result
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

    fn sanitize_doc(&self, text: &str, full: bool) -> String {
        let mut result = text.to_string();

        // 0. Normalize existing backticks - remove them and we'll re-add properly later
        // This handles cases where the spec already has backticks that may be unpaired
        // or in invalid Rust doc format
        result = result.replace('`', "");

        // 1. Wrap path-like patterns in backticks
        let path_re = Regex::new(r"([a-z]+/[\w{/}-]+)").unwrap();
        result = path_re.replace_all(&result, "`$1`").to_string();

        // 2. Wrap type-like patterns
        let type_re = Regex::new(r"\b(String|i32|i64|bool|f64|Vec<[^>]+>|Option<[^>]+>)\b").unwrap();
        result = type_re.replace_all(&result, "`$1`").to_string();

        // 3. Wrap enum-like values
        let enum_re = Regex::new(r"\b(TRUE|FALSE|OK|ERROR|PENDING|ACTIVE|INACTIVE)\b").unwrap();
        result = enum_re.replace_all(&result, "`$1`").to_string();

        // 4. Wrap code-like identifiers (camelCase or single words that look like code)
        // Matches things like `returnPartialSuccess`, `maxResults`, etc.
        let code_re = Regex::new(r"\b([a-z][a-zA-Z0-9]+(?:[A-Z][a-zA-Z0-9]+)+)\b").unwrap();
        result = code_re.replace_all(&result, "`$1`").to_string();

        // 5. Wrap boolean literals
        let bool_re = Regex::new(r"\b(true|false|null)\b").unwrap();
        result = bool_re.replace_all(&result, "`$1`").to_string();

        // 6. Convert bare URLs to angle-bracket links
        let url_re = Regex::new(r"(https?://[^\s<>\[\]()]+)").unwrap();
        result = url_re.replace_all(&result, "<$1>").to_string();

        // 7. Escape stray angle brackets not in backticks or URLs
        let mut escaped = String::new();
        let mut in_backticks = false;
        let mut in_url = false;
        let mut chars = result.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '`' {
                in_backticks = !in_backticks;
                escaped.push(c);
            } else if c == '<' && !in_backticks {
                // Check if this looks like a URL start
                let rest: String = chars.clone().collect();
                if rest.starts_with("http://") || rest.starts_with("https://") {
                    in_url = true;
                    escaped.push(c);
                } else {
                    escaped.push_str("&lt;");
                }
            } else if c == '>' && !in_backticks && in_url {
                in_url = false;
                escaped.push(c);
            } else if c == '>' && !in_backticks {
                escaped.push_str("&gt;");
            } else {
                escaped.push(c);
            }
        }
        result = escaped;

        // 8. For field comments, use only first line
        if !full {
            result = result.lines().next().unwrap_or(&result).to_string();
        } else {
            // For full comments, ensure each line is properly formatted
            // Join lines with "/// " prefix for multi-line descriptions
            result = result
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join(" ");
        }

        result
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
