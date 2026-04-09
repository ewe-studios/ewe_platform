//! WHY: Generates type-safe API client functions from OpenAPI specifications.
//!
//! WHAT: Reads OpenAPI specs from `artefacts/cloud_providers/`, extracts endpoint definitions,
//! and generates Rust functions that use `SimpleHttpClient` to make API calls.
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

use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::de::Error;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, derive_more::Display)]
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
// OpenAPI structures (minimal for endpoint extraction)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OpenApiSpec {
    #[serde(default)]
    openapi: Option<String>,
    #[serde(default)]
    info: Option<Info>,
    #[serde(default)]
    paths: BTreeMap<String, PathItem>,
    // GCP Discovery Document format
    #[serde(default)]
    resources: Option<BTreeMap<String, Resource>>,
    #[serde(default, rename = "baseUrl")]
    base_url: Option<String>,
    // Standard OpenAPI 3.x servers
    #[serde(default)]
    servers: Option<Vec<Server>>,
    // GCP Discovery Document basePath (alternative to baseUrl)
    #[serde(default, rename = "basePath")]
    base_path: Option<String>,
    // GCP Discovery Document rootUrl
    #[serde(default, rename = "rootUrl")]
    root_url: Option<String>,
    // GCP Discovery Document servicePath
    #[serde(default, rename = "servicePath")]
    service_path: Option<String>,
    // OpenAPI 3.x components
    #[serde(default)]
    components: Option<Components>,
}

#[derive(Debug, Deserialize, Default)]
struct Components {
    #[serde(default, rename = "schemas")]
    schemas: Option<BTreeMap<String, SchemaRef>>,
}

#[derive(Debug, Deserialize)]
struct Server {
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize)]
struct Info {
    title: String,
    version: String,
}

// GCP Discovery Document resource structure
#[derive(Debug, Deserialize, Default)]
struct Resource {
    #[serde(default)]
    methods: Option<BTreeMap<String, GcpMethod>>,
    #[serde(default)]
    resources: Option<BTreeMap<String, Resource>>,
}

#[derive(Debug, Deserialize, Default)]
struct GcpMethod {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "flatPath")]
    flat_path: Option<String>,
    #[serde(default)]
    http_method: Option<String>,
    #[serde(default)]
    parameters: Option<BTreeMap<String, GcpParameter>>,
    #[serde(default, rename = "parameterOrder")]
    parameter_order: Option<Vec<String>>,
    #[serde(default, rename = "request")]
    request_body: Option<GcpRequestBody>,
    #[serde(default)]
    response: Option<GcpResponse>,
}

#[derive(Debug, Deserialize, Default)]
struct GcpParameter {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    #[serde(rename = "type")]
    param_type: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GcpRequestBody {
    #[serde(default, rename = "$ref")]
    ref_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GcpResponse {
    #[serde(default, rename = "$ref")]
    ref_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PathItem {
    #[serde(default)]
    get: Option<Operation>,
    #[serde(default)]
    post: Option<Operation>,
    #[serde(default)]
    put: Option<Operation>,
    #[serde(default)]
    patch: Option<Operation>,
    #[serde(default)]
    delete: Option<Operation>,
}

#[derive(Debug, Deserialize, Default)]
struct Operation {
    #[serde(default)]
    operation_id: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Vec<Parameter>,
    #[serde(default, rename = "requestBody")]
    request_body: Option<RequestBody>,
    #[serde(default)]
    responses: BTreeMap<String, Response>,
}

#[derive(Debug, Deserialize, Default)]
struct Parameter {
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "in", default)]
    location: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    schema: Option<ParameterSchema>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ParameterSchema {
    #[serde(default, rename = "type")]
    param_type: Option<serde_json::Value>,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RequestBody {
    #[serde(default)]
    content: BTreeMap<String, MediaType>,
}

#[derive(Debug, Deserialize, Default)]
struct Response {
    content: Option<BTreeMap<String, MediaType>>,
}

#[derive(Debug, Deserialize, Default)]
struct MediaType {
    schema: Option<SchemaRef>,
}

#[derive(Debug, Deserialize, Default)]
struct SchemaRef {
    #[serde(default, rename = "$ref")]
    ref_path: Option<String>,
    #[serde(default)]
    properties: Option<serde_json::Value>,
    #[serde(default, rename = "anyOf")]
    any_of: Option<Vec<SchemaRef>>,
    #[serde(default, rename = "oneOf")]
    one_of: Option<Vec<SchemaRef>>,
    #[serde(default, rename = "allOf")]
    all_of: Option<Vec<SchemaRef>>,
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
                        || self.discover_sub_apis(&entry.path()).is_empty() == false
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

        // Try parsing as OpenAPI spec directly first
        let spec: OpenApiSpec = serde_json::from_str(&content)
            .or_else(|parse_err| {
                // Try unwrapping from nested structure (e.g., {"openapi.json": {...}})
                let wrapped: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|_| serde_json::Error::custom("failed to parse wrapped spec"))?;
                if let Some(obj) = wrapped.as_object() {
                    // Try common wrapper keys
                    for key in ["openapi.json", "openapi", "spec"] {
                        if let Some(inner) = obj.get(key) {
                            if let Ok(spec) = serde_json::from_value(inner.clone()) {
                                return Ok(spec);
                            }
                        }
                    }
                }
                // Return original error
                Err(parse_err)
            })?;

        // Extract endpoints
        let endpoints = self.extract_endpoints(&spec)?;
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
        writeln!(out, "pub mod types;")?;
        writeln!(out)?;

        // Imports
        let provider_module = label.split('/').next().unwrap_or(label).replace('-', "_");
        writeln!(out, "use foundation_core::valtron::{{execute, BoxedSendExecutionAction, StreamIterator, StreamIteratorExt, TaskIterator, TaskIteratorExt}};")?;
        writeln!(out, "use foundation_core::wire::simple_http::client::{{")?;
        writeln!(out, "    body_reader, ClientRequestBuilder, RequestIntro, SimpleHttpClient, SystemDnsResolver,")?;
        writeln!(out, "}};")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out, "use serde::Serialize;")?;
        writeln!(out, "use crate::providers::{}::clients::types::*;", provider_module)?;
        writeln!(out, "use crate::providers::{}::resources::*;", provider_module)?;
        writeln!(out)?;

        // Deduplicate endpoints by generated struct name to avoid duplicate definitions
        // (e.g., indicator-types and indicatorTypes both become IndicatorTypes)
        let mut seen_struct_names = std::collections::HashSet::new();
        let mut unique_endpoints = Vec::new();
        for endpoint in &endpoints {
            let fn_name = self.endpoint_to_fn_name(endpoint);
            let struct_name = format!("{}Args", self.to_pascal_case(&fn_name));
            if seen_struct_names.insert(struct_name) {
                unique_endpoints.push(endpoint);
            }
        }

        // Generate functions for each unique endpoint
        for endpoint in unique_endpoints {
            self.generate_builder_fn(&mut out, endpoint)?;
            self.generate_task_fn(&mut out, endpoint)?;
            self.generate_execute_fn(&mut out, endpoint)?;
            self.generate_convenience_fn(&mut out, endpoint)?;
        }

        // Write output
        std::fs::write(output_path, out)?;
        tracing::info!("    Written: {}", output_path.display());

        Ok(())
    }

    fn extract_endpoints(&self, spec: &OpenApiSpec) -> Result<Vec<ApiEndpoint>, GenClientError> {
        let mut endpoints = Vec::new();

        // Regex to extract placeholders like {sitesId} or {+name} from paths
        let placeholder_re = Regex::new(r"\{(\+)?([^}]+)\}").unwrap();

        // Extract base URL from multiple possible sources:
        // 1. OpenAPI 3.x: servers[0].url
        // 2. GCP Discovery: baseUrl
        // 3. GCP Discovery: rootUrl + servicePath
        let base_url: String = spec
            .servers
            .as_ref()
            .and_then(|servers| servers.first())
            .map(|s| s.url.clone())
            .or(spec.base_url.clone())
            .or_else(|| {
                // Combine rootUrl and servicePath for GCP
                match (&spec.root_url, &spec.service_path) {
                    (Some(root), Some(service)) => Some(format!("{}{}", root, service)),
                    (Some(root), None) => Some(root.clone()),
                    (None, Some(service)) => Some(service.clone()),
                    (None, None) => None,
                }
            })
            .ok_or_else(|| GenClientError::ParseFailed(
                "No base URL found in spec - missing 'servers' (OpenAPI 3.x), 'baseUrl', or 'rootUrl'+'servicePath' (GCP Discovery)".to_string()
            ))?;

        // Extract from standard OpenAPI paths
        for (path, path_item) in &spec.paths {
            // Check each HTTP method
            let methods = [
                ("get", &path_item.get),
                ("post", &path_item.post),
                ("put", &path_item.put),
                ("patch", &path_item.patch),
                ("delete", &path_item.delete),
            ];

            for (method_name, operation_opt) in methods {
                if let Some(operation) = operation_opt {
                    let mut path_params = Vec::new();
                    let mut query_params = Vec::new();

                    // Extract parameters
                    for param in &operation.parameters {
                        // Skip parameters without name or location
                        let original_name = match &param.name {
                            Some(n) => n.clone(),
                            None => continue,
                        };
                        let name = original_name
                            .replace('.', "_")
                            .replace('-', "_")
                            .replace('<', "_")
                            .replace('>', "_")
                            .replace('[', "_")
                            .replace(']', "_")
                            .replace('~', "_");
                        let location = match &param.location {
                            Some(l) => l.clone(),
                            None => continue,
                        };

                        let rust_type = self.param_to_rust_type(param);
                        let param_info = ParameterInfo {
                            name,
                            original_name,
                            rust_type,
                            required: param.required,
                            description: param.description.clone(),
                        };

                        match location.as_str() {
                            "path" => path_params.push(param_info),
                            "query" => query_params.push(param_info),
                            _ => {}
                        }
                    }

                    // Extract request body type - use serde_json::Value for complex union types
                    let request_body_type = operation
                        .request_body
                        .as_ref()
                        .and_then(|rb| rb.content.get("application/json"))
                        .and_then(|mt| mt.schema.as_ref())
                        .and_then(|s| {
                            // Check if this is a union type (anyOf/oneOf without properties)
                            // For $ref schemas, look up the definition in components
                            let schema_to_check = if s.ref_path.is_some() && s.properties.is_none()
                                && s.any_of.is_none() && s.one_of.is_none() {
                                // It's a $ref, look up the actual schema
                                s.ref_path.as_ref()
                                    .and_then(|ref_path| {
                                        let type_name = ref_path.trim_start_matches("#/components/schemas/");
                                        spec.components.as_ref()
                                            .and_then(|c| c.schemas.as_ref())
                                            .and_then(|schemas| schemas.get(type_name))
                                    })
                            } else {
                                Some(s)
                            };

                            schema_to_check.map(|schema| {
                                // Use serde_json::Value for union types (anyOf/oneOf without properties)
                                let is_union_type = (schema.any_of.is_some() || schema.one_of.is_some())
                                    && schema.properties.is_none();
                                if is_union_type {
                                    "serde_json::Value".to_string()
                                } else {
                                    self.extract_type_name_from_ref(schema).unwrap_or_else(|| "serde_json::Value".to_string())
                                }
                            })
                        });

                    // Extract response type
                    let response_type = self.extract_response_type(&operation.responses, spec);

                    // Extract placeholder names from path in order (e.g., ["project"] from "/projects/{project}")
                    let path_placeholders: Vec<String> = placeholder_re
                        .captures_iter(path)
                        .map(|cap| cap[2].to_string())
                        .collect();

                    endpoints.push(ApiEndpoint {
                        path: path.clone(),
                        method: method_name.to_uppercase(),
                        operation_id: operation.operation_id.clone(),
                        summary: operation.summary.clone(),
                        path_params,
                        query_params,
                        request_body_type,
                        response_type,
                        base_url: Some(base_url.clone()),
                        path_placeholders,
                    });
                }
            }
        }

        // Extract from GCP Discovery Document format (resources -> methods)
        if let Some(resources) = &spec.resources {
            self.extract_gcp_endpoints(resources, "", &mut endpoints, Some(&base_url));
        }

        // Filter out endpoints where path placeholders don't match path_params
        // (OpenAPI spec may have {account_id} in path but not declare it as a parameter)
        endpoints.retain(|ep| {
            ep.path_placeholders.len() == ep.path_params.len()
        });

        // Deduplicate endpoints by method + path (some specs have duplicates with different operation_ids)
        let mut seen = std::collections::HashSet::new();
        endpoints.retain(|ep| {
            let key = format!("{}:{}", ep.method, ep.path);
            seen.insert(key)
        });

        Ok(endpoints)
    }

    /// Extract endpoints from GCP Discovery Document resources recursively.
    fn extract_gcp_endpoints(
        &self,
        resources: &BTreeMap<String, Resource>,
        parent_path: &str,
        endpoints: &mut Vec<ApiEndpoint>,
        base_url: Option<&str>,
    ) {
        // Regex to extract placeholders like {sitesId} or {+name} from paths
        let placeholder_re = Regex::new(r"\{(\+)?([^}]+)\}").unwrap();

        for (_resource_name, resource) in resources {
            // Extract methods from this resource
            if let Some(methods) = &resource.methods {
                for (_method_name, method) in methods {
                    // Use flatPath for the actual request URL (path is a template like "v1/{+name}")
                    let path = method.flat_path.as_deref().unwrap_or_else(|| method.path.as_deref().unwrap_or(""));

                    // Extract placeholder names from flatPath in order (e.g., ["sitesId"] from "v1/sites/{sitesId}")
                    let path_placeholders: Vec<String> = placeholder_re
                        .captures_iter(path)
                        .map(|cap| cap[2].to_string())
                        .collect();

                    // Extract all parameters first
                    let mut all_params: BTreeMap<String, ParameterInfo> = BTreeMap::new();

                    if let Some(parameters) = &method.parameters {
                        for (param_name, param) in parameters {
                            let rust_type = self.gcp_param_to_rust_type(param);
                            // Sanitize parameter names: replace dots, dashes, and special chars with underscores
                            let sanitized_name = param_name
                                .replace('.', "_")
                                .replace('-', "_")
                                .replace('<', "_")
                                .replace('>', "_")
                                .replace('[', "_")
                                .replace(']', "_")
                                .replace('~', "_");
                            let param_info = ParameterInfo {
                                name: sanitized_name,
                                original_name: param_name.clone(),
                                rust_type,
                                required: param.required,
                                description: param.description.clone(),
                            };
                            all_params.insert(param_name.clone(), param_info);
                        }
                    }

                    // Now build path_params and query_params in the correct order
                    // For path params, use parameterOrder if available to get correct ordering
                    let mut path_params = Vec::new();
                    let mut query_params = Vec::new();

                    // First, add path parameters in parameterOrder sequence (for GCP, this maps placeholders to param names)
                    if let Some(param_order) = &method.parameter_order {
                        for param_name in param_order {
                            if let Some(param_info) = all_params.get(param_name) {
                                path_params.push(param_info.clone());
                            }
                        }
                    } else {
                        // Fallback: extract path params from their location
                        for (param_name, param_info) in &all_params {
                            // Check if this param is in path_placeholders (indicates it's a path param)
                            if path_placeholders.contains(param_name) {
                                path_params.push(param_info.clone());
                            }
                        }
                    }

                    // Then add remaining params as query params
                    for (param_name, param_info) in &all_params {
                        if !path_params.iter().any(|p| p.name == param_info.name) {
                            query_params.push(param_info.clone());
                        }
                    }

                    // Extract request body type
                    let request_body_type = method
                        .request_body
                        .as_ref()
                        .and_then(|rb| rb.ref_path.as_ref())
                        .and_then(|ref_path| self.extract_type_name_from_gcp_ref(ref_path));

                    // Extract response type
                    let response_type = method
                        .response
                        .as_ref()
                        .and_then(|resp| resp.ref_path.as_ref())
                        .and_then(|ref_path| self.extract_type_name_from_gcp_ref(ref_path));

                    endpoints.push(ApiEndpoint {
                        path: path.to_string(),
                        method: method.http_method.as_deref().unwrap_or("GET").to_uppercase(),
                        operation_id: method.id.clone(),
                        summary: method.description.clone(),
                        path_params,
                        query_params,
                        request_body_type,
                        response_type,
                        base_url: base_url.map(String::from),
                        path_placeholders,
                    });
                }
            }

            // Recurse into nested resources
            if let Some(nested) = &resource.resources {
                self.extract_gcp_endpoints(nested, parent_path, endpoints, base_url);
            }
        }
    }

    fn gcp_param_to_rust_type(&self, param: &GcpParameter) -> String {
        let param_type = param.param_type.as_deref().unwrap_or("string");
        let format = param.format.as_deref();

        match (param_type, format) {
            ("integer", Some("int64")) => "i64".to_string(),
            ("integer", Some("int32")) => "i32".to_string(),
            ("integer", _) => "i32".to_string(),
            ("number", Some("float")) => "f32".to_string(),
            ("number", Some("double")) => "f64".to_string(),
            ("number", _) => "f64".to_string(),
            ("boolean", _) => "bool".to_string(),
            ("string", Some("date-time")) => "String".to_string(),
            ("string", Some("date")) => "String".to_string(),
            ("string", _) => "String".to_string(),
            ("array", _) => "Vec<String>".to_string(),
            _ => "String".to_string(),
        }
    }

    fn extract_type_name_from_gcp_ref(&self, ref_path: &str) -> Option<String> {
        // Extract type name from GCP ref like "GoogleCloudRunV2Service"
        // Use the exact name that gen_resource_types generates (no stripping)
        Some(
            ref_path
                .split('.')
                .map(|part| {
                    let mut chars = part.chars();
                    chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()
                        + chars.as_str()
                })
                .collect::<String>(),
        )
    }

    fn param_to_rust_type(&self, param: &Parameter) -> String {
        let schema = param.schema.as_ref();
        // Handle type being either a string or array of strings (e.g., ["string", "null"])
        let param_type = schema
            .and_then(|s| s.param_type.as_ref())
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s)
                } else if let Some(arr) = v.as_array() {
                    // Take the first non-null type from the array
                    arr.iter()
                        .filter_map(|item| item.as_str())
                        .find(|t| *t != "null")
                } else {
                    None
                }
            })
            .unwrap_or("string");
        let format = schema.and_then(|s| s.format.as_deref());

        match (param_type, format) {
            ("integer", Some("int64")) => "i64".to_string(),
            ("integer", Some("int32")) => "i32".to_string(),
            ("integer", _) => "i32".to_string(),
            ("number", Some("float")) => "f32".to_string(),
            ("number", Some("double")) => "f64".to_string(),
            ("number", _) => "f64".to_string(),
            ("boolean", _) => "bool".to_string(),
            ("string", Some("date-time")) => "String".to_string(),
            ("string", Some("date")) => "String".to_string(),
            ("string", _) => "String".to_string(),
            ("array", _) => "Vec<String>".to_string(),
            _ => "String".to_string(),
        }
    }

    fn extract_type_name_from_ref(&self, schema: &SchemaRef) -> Option<String> {
        schema.ref_path.as_ref().and_then(|ref_path| {
            // Extract type name from #/components/schemas/ServiceName
            ref_path.split('/').last().map(|s| {
                // Convert to proper PascalCase to match gen_resource_types output
                // Split on dots, hyphens, @, AND underscores
                // e.g., treasury.transaction -> TreasuryTransaction
                // e.g., Custom-pages -> CustomPages
                // e.g., iam_response_collection_accounts -> IamResponseCollectionAccounts
                // e.g., @cf_ai4bharat... -> CfAi4bharat
                s.split(|c| c == '.' || c == '-' || c == '@' || c == '_')
                    .map(|part| {
                        let mut chars = part.chars();
                        chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()
                            + chars.as_str()
                    })
                    .collect::<String>()
            })
        })
    }

    fn extract_response_type(&self, responses: &BTreeMap<String, Response>, spec: &OpenApiSpec) -> Option<String> {
        // Look for 200, 201, 204 responses
        for status in &["200", "201", "202", "204"] {
            if let Some(response) = responses.get(*status) {
                if let Some(content) = &response.content {
                    if let Some(media) = content.get("application/json") {
                        if let Some(schema) = &media.schema {
                            // Check if this is a generatable type
                            // For $ref schemas, look up the definition in components
                            let schema_to_check = if schema.ref_path.is_some()
                                && schema.properties.is_none()
                                && schema.any_of.is_none()
                                && schema.one_of.is_none() {
                                // It's a $ref, look up the actual schema
                                schema.ref_path.as_ref()
                                    .and_then(|ref_path| {
                                        let type_name = ref_path.trim_start_matches("#/components/schemas/");
                                        spec.components.as_ref()
                                            .and_then(|c| c.schemas.as_ref())
                                            .and_then(|schemas| schemas.get(type_name))
                                    })
                            } else {
                                Some(schema)
                            };

                            // Check if the type is generatable (has properties or simple structure)
                            let is_generatable = schema_to_check.map_or(true, |s| {
                                // Skip types that are only allOf/anyOf/oneOf without properties
                                // unless they're simple wrappers
                                s.properties.is_some()
                                || (s.all_of.is_none() && s.any_of.is_none() && s.one_of.is_none())
                            });

                            if is_generatable {
                                return self.extract_type_name_from_ref(schema);
                            } else {
                                return Some("serde_json::Value".to_string());
                            }
                        }
                    }
                }
                // 204 No Content
                if *status == "204" {
                    return Some("()".to_string());
                }
            }
        }
        None
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

        // Function signature - take owned types, use .as_str() internally
        write!(out, "pub fn {}_builder(\n    client: &SimpleHttpClient,", fn_name)?;

        // Path parameters - owned String
        for param in &endpoint.path_params {
            writeln!(out)?;
            write!(out, "    {}: String,", self.escape_keyword(&param.name))?;
        }

        // Query parameters (optional) - owned String types
        for param in &endpoint.query_params {
            writeln!(out)?;
            write!(out, "    {}: Option<{}>,", self.escape_keyword(&param.name), param.rust_type)?;
        }

        // Request body
        if let Some(body_type) = &endpoint.request_body_type {
            writeln!(out)?;
            write!(out, "    body: &{}", body_type)?;
        }

        writeln!(out)?;
        writeln!(out, ") -> Result<ClientRequestBuilder<SystemDnsResolver>, ApiError> {{")?;

        // Build URL
        writeln!(out)?;
        writeln!(out, "    // Build URL")?;

        // Build URL format string by replacing placeholders with {}
        // Replace placeholders in order of appearance, passing params positionally
        let mut url_format = endpoint.path.clone();

        // For each placeholder in order, replace its first occurrence with {}
        // and track which param to pass for it
        let mut params_to_pass: Vec<&str> = Vec::new();

        for placeholder in endpoint.path_placeholders.iter() {
            let placeholder_pattern = format!("{{{}}}", placeholder);
            if let Some(pos) = url_format.find(&placeholder_pattern) {
                let before = &url_format[..pos];
                let after = &url_format[pos + placeholder_pattern.len()..];
                url_format = format!("{}{{}}{}", before, after);

                // Find the matching param and add it to the list
                if let Some(param) = endpoint.path_params.iter()
                    .find(|p| &p.name == placeholder || &p.original_name == placeholder)
                {
                    params_to_pass.push(&param.name);
                }
            }
        }

        let base_url = endpoint.base_url.as_deref().unwrap_or("https://api.example.com");
        writeln!(out, "    let url = format!(")?;
        writeln!(out, "        \"{}{}\",", base_url, url_format)?;
        // Pass path parameters in the order they appear in the path
        for param_name in &params_to_pass {
            writeln!(out, "        {}.as_str(),", self.escape_keyword(param_name))?;
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

                // Check if this is a Vec type - handle array params differently
                if rust_type.starts_with("Vec<") {
                    // For array params: if let Some(vals) = param { for val in vals { ... } }
                    writeln!(out, "    if let Some(vals) = {} {{", self.escape_keyword(param_name))?;
                    writeln!(out, "        for val in vals {{")?;
                    writeln!(out, "            query_parts.push(format!(\"{}={{}}\", val));", original_name)?;
                    writeln!(out, "        }}")?;
                    writeln!(out, "    }}")?;
                } else {
                    writeln!(out, "    if let Some(val) = {} {{", self.escape_keyword(param_name))?;
                    writeln!(out, "        query_parts.push(format!(\"{}={{}}\", val));", original_name)?;
                    writeln!(out, "    }}")?;
                }
            }
            writeln!(out)?;
            writeln!(out, "    let url_with_query = if query_parts.is_empty() {{")?;
            writeln!(out, "        url")?;
            writeln!(out, "    }} else {{")?;
            writeln!(out, "        format!(\"{{}}?{{}}\", url, query_parts.join(\"&\"))")?;
            writeln!(out, "    }};")?;
            writeln!(out)?;
            let method_lower = endpoint.method.to_lowercase();
            writeln!(out, "    let builder = client.{}(&url_with_query)", method_lower)?;
        } else {
            let method_lower = endpoint.method.to_lowercase();
            writeln!(out, "    let builder = client.{}(&url)", method_lower)?;
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
        let struct_name = format!("{}Args", self.to_pascal_case(&fn_name));

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
                writeln!(out, "    pub {}: String,", self.escape_keyword(&param.name))?;
            }

            for param in &endpoint.query_params {
                let rust_type = if param.rust_type == "String" {
                    "String".to_string()
                } else {
                    param.rust_type.clone()
                };
                writeln!(out, "    /// Query parameter: {}", param.name)?;
                writeln!(out, "    pub {}: Option<{}>,", self.escape_keyword(&param.name), rust_type)?;
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

        // Call builder then execute - pass owned values, cloning as needed
        writeln!(out)?;
        write!(out, "    let builder = {}_builder(client", fn_name)?;
        for param in &endpoint.path_params {
            write!(out, ", args.{}.clone()", self.escape_keyword(&param.name))?;
        }
        for param in &endpoint.query_params {
            // Clone all query params since builder takes owned types
            write!(out, ", args.{}.clone()", self.escape_keyword(&param.name))?;
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
                .replace('-', "_")
                .replace('.', "_")
                .replace('@', "_")
                .replace(':', "_")
                .replace('<', "_")
                .replace('>', "_")
                .replace('[', "_")
                .replace(']', "_");
            return self.to_snake_case(&clean_id);
        }

        // Generate from method + path
        let path_part = endpoint
            .path
            .trim_matches('/')
            .replace('/', "_")
            .replace('{', "")
            .replace('}', "")
            .replace('-', "_")  // Replace dashes in path
            .replace('.', "_") // Replace dots in path
            .replace('@', "_") // Replace @ in path
            .replace(':', "_") // Replace : in path
            .replace('<', "_") // Replace < in path
            .replace('>', "_") // Replace > in path
            .replace('[', "_") // Replace [ in path
            .replace(']', "_"); // Replace ] in path
        format!("{}_{}", endpoint.method.to_lowercase(), path_part)
    }

    fn to_snake_case(&self, s: &str) -> String {
        // First replace special characters with underscores
        let normalized = s
            .replace('-', "_")
            .replace('.', "_")
            .replace('@', "_")
            .replace(':', "_")
            .replace('<', "_")
            .replace('>', "_")
            .replace('[', "_")
            .replace(']', "_");

        let mut result = String::new();
        let mut prev_was_upper = false;

        for (i, c) in normalized.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 && !prev_was_upper {
                    result.push('_');
                }
                result.push(c.to_lowercase().next().unwrap());
                prev_was_upper = true;
            } else if c.is_numeric() {
                result.push(c);
                prev_was_upper = false;
            } else if c == '_' {
                result.push('_');
                prev_was_upper = false;
            } else {
                result.push(c);
                prev_was_upper = false;
            }
        }

        result
    }


    fn label_to_feature_name(label: &str) -> String {
        label.split('/').next().unwrap_or(label).replace('-', "_")
    }

    fn to_pascal_case(&self, s: &str) -> String {
        // First normalize special characters to underscores
        let normalized = s
            .replace('-', "_")
            .replace('.', "_")
            .replace('@', "_")
            .replace(':', "_")
            .replace('<', "_")
            .replace('>', "_")
            .replace('[', "_")
            .replace(']', "_");

        normalized.split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(c) => {
                        let mut result = c.to_uppercase().to_string();
                        result.extend(chars);
                        result
                    }
                    None => String::new(),
                }
            })
            .collect()
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
                if path.extension().map_or(false, |ext| ext == "rs") {
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
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
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
