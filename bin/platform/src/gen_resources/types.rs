//! WHY: Generates Rust type definitions from OpenAPI specs for cloud providers.
//!
//! WHAT: Reads OpenAPI specs from `artefacts/cloud_providers/`, uses foundation_openapi
//! for parsing and normalization, then generates Rust structs representing cloud resources.
//!
//! HOW: Uses foundation_openapi for all OpenAPI processing (parsing, type resolution,
//! endpoint extraction), then generates Rust source files using string templates.
//!
//! Feature Flags:
//! - Provider-level: `gcp`, `cloudflare`, etc. in provider/mod.rs
//! - API-level: `gcp_compute`, `gcp_run`, etc. in each {api}.rs file
//! - Dependencies: Cross-API type references tracked and both flags included

use regex::Regex;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Import from foundation_openapi to avoid duplication
use foundation_openapi::spec::Schema;

/// Endpoint info for codegen - simplified from foundation_openapi::EndpointInfo.
#[derive(Debug, Clone)]
struct CodegenEndpoint {
    operation_id: String,
    response_type: Option<String>,
    path_params: Vec<String>,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// WHY: Provides structured error reporting for resource generation failures.
///
/// WHAT: Covers file I/O, JSON parsing, and code generation errors.
///
/// HOW: Uses `derive_more` for Display, manual From implementations to avoid conflicts.
#[derive(Debug, derive_more::Display)]
pub enum GenResourceError {
    /// Could not read input spec file.
    #[display("failed to read {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    /// Could not parse JSON spec.
    #[display("json parse error for {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },

    /// Could not write generated file.
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    /// Schema type not supported.
    #[display("unsupported schema type {type_name} for {schema}")]
    UnsupportedType { type_name: String, schema: String },
}

impl std::error::Error for GenResourceError {}

impl From<std::io::Error> for GenResourceError {
    fn from(e: std::io::Error) -> Self {
        // Default to ReadFile for generic io::Error conversions
        // For specific cases, use explicit variants
        GenResourceError::ReadFile {
            path: String::new(),
            source: e,
        }
    }
}

impl From<serde_json::Error> for GenResourceError {
    fn from(e: serde_json::Error) -> Self {
        GenResourceError::Json {
            path: String::new(),
            source: e,
        }
    }
}

impl From<std::fmt::Error> for GenResourceError {
    fn from(e: std::fmt::Error) -> Self {
        GenResourceError::WriteFile {
            path: String::new(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        }
    }
}

// ---------------------------------------------------------------------------
// Intermediate representation for generated types
// ---------------------------------------------------------------------------

/// WHY: Normalized representation of a resource type.
///
/// WHAT: Captures the structure needed for Rust codegen.
///
/// HOW: Built from OpenAPI schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDef {
    /// Resource name (Rust-safe identifier)
    pub name: String,
    /// Original OpenAPI schema name
    pub schema_name: String,
    /// Description
    pub description: Option<String>,
    /// Fields
    pub fields: Vec<FieldDef>,
    /// Whether this is a root resource (top-level API resource)
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    /// Field name (Rust-safe snake_case identifier)
    pub name: String,
    /// Original field name from the spec (for serde rename)
    pub original_name: String,
    /// Field type (Rust type string)
    pub ty: String,
    /// Whether the field is required
    pub required: bool,
    /// Description
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Doc comment sanitization
// ---------------------------------------------------------------------------

/// Helper function to convert fmt::Error to GenResourceError.
fn write_fmt_error(_e: std::fmt::Error) -> GenResourceError {
    GenResourceError::WriteFile {
        path: "generated mod.rs".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::Other, "fmt error"),
    }
}

/// Sanitize a description string for use as a rustdoc comment.
///
/// This function:
/// 1. Converts HTML tags like `<code>` to backticks
/// 2. Wraps code-like patterns in backticks (paths, types, values)
/// 3. Converts bare URLs to angle-bracket links
/// 4. Escapes stray angle brackets not in URLs or backticks
/// 5. Truncates to first line for field-level comments (if `first_line_only` is true)
fn sanitize_doc_comment(description: &str, first_line_only: bool) -> String {
    if description.is_empty() {
        return String::new();
    }

    let mut result = description.to_string();

    // 1. Strip ALL backticks to avoid issues with rustdoc parsing
    // We don't re-add backticks because unbalanced backticks or backticks
    // containing special characters (like single quotes) break rustdoc
    result = result.replace('`', "");

    // 2. Escape single quotes to prevent them being interpreted as character literals
    // This is necessary for examples like: timestamp('2020-10-01T00:00:00Z')
    result = result.replace('\'', "''");

    // 3. Escape angle brackets using HTML entities to prevent them being
    // interpreted as generics in rustdoc
    result = result.replace('<', "&lt;").replace('>', "&gt;");

    // 4. Convert HTML <code> tags to plain text (no backticks)
    let code_tag_re = Regex::new(r"<code>([^<]+)</code>").unwrap();
    result = code_tag_re.replace_all(&result, "$1").to_string();

    // 5. Remove other HTML tags but keep content
    let html_tag_re = Regex::new(r"</?[^>]+>").unwrap();
    result = html_tag_re.replace_all(&result, "").to_string();

    // 6. For field comments, use only first line
    if first_line_only {
        if let Some(first) = result.lines().next() {
            result = first.to_string();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Resource type generator
// ---------------------------------------------------------------------------

/// WHY: Orchestrates resource type generation from OpenAPI specs.
///
/// WHAT: Reads specs, extracts schemas, generates Rust code.
///
/// HOW: Multi-pass approach: parse, normalize, generate.
pub struct ResourceGenerator {
    artefacts_dir: PathBuf,
    output_dir: PathBuf,
}

impl ResourceGenerator {
    pub fn new(artefacts_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            artefacts_dir,
            output_dir,
        }
    }

    /// Generate resource types for all providers.
    pub fn generate_all(&self) -> Result<(), GenResourceError> {
        let providers = self.discover_providers()?;

        for provider in &providers {
            self.generate_for_provider(provider)?;
        }

        Ok(())
    }

    /// Generate resource types for a single provider.
    ///
    /// If the provider directory has subdirectories with `openapi.json` files
    /// (e.g. `gcp/compute/openapi.json`), generates one file per sub-API.
    /// Otherwise generates a single `mod.rs` from the top-level spec.
    pub fn generate_for_provider(&self, provider: &str) -> Result<(), GenResourceError> {
        tracing::info!("Generating for provider: {}", provider);
        let provider_dir = self.artefacts_dir.join(provider);
        tracing::info!("  Provider dir: {}", provider_dir.display());
        tracing::info!("  Artefacts dir: {}", self.artefacts_dir.display());
        tracing::info!("  Output dir: {}", self.output_dir.display());

        // Discover sub-API directories (e.g. gcp/compute/, gcp/run/)
        let sub_apis = self.discover_sub_apis(&provider_dir);
        tracing::info!("  Found {} sub-APIs", sub_apis.len());

        // Output goes to providers/{provider}/resources/
        // self.output_dir is already backends/foundation_deployment/src/providers
        let provider_output_dir = self.output_dir.join(provider).join("resources");
        std::fs::create_dir_all(&provider_output_dir)?;
        tracing::info!("  Output dir: {}", provider_output_dir.display());

        if sub_apis.is_empty() {
            tracing::info!("  Single spec mode");
            // Single spec: provider/openapi.json -> resources/mod.rs
            self.generate_from_spec_simple(provider, &provider_dir.join("openapi.json"), &provider_output_dir.join("mod.rs"))?;
        } else {
            tracing::info!("  Multi-API mode");
            // Per-API specs: provider/{api}/openapi.json -> resources/{api}.rs
            for api_name in &sub_apis {
                let spec_path = provider_dir.join(api_name).join("openapi.json");
                let output_path = provider_output_dir.join(format!("{api_name}.rs"));
                let label = format!("{provider}/{api_name}");
                self.generate_from_spec_simple(&label, &spec_path, &output_path)?;
            }
            // Generate mod.rs for multi-API provider
            self.generate_mod_rs_simple(provider, &sub_apis)?;
        }

        Ok(())
    }

    /// Discover sub-API directories that contain their own `openapi.json`.
    fn discover_sub_apis(&self, provider_dir: &std::path::Path) -> Vec<String> {
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

    /// Generate resource types from a single spec file.
    fn generate_from_spec(
        &self,
        label: &str,
        spec_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Result<(), GenResourceError> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        tracing::info!("Generating resource types for {label}...");

        let spec_content =
            std::fs::read_to_string(spec_path).map_err(|e| GenResourceError::ReadFile {
                path: spec_path.display().to_string(),
                source: e,
            })?;

        let spec: Value =
            serde_json::from_str(&spec_content).map_err(|e| GenResourceError::Json {
                path: spec_path.display().to_string(),
                source: e,
            })?;

        // Collect the set of schema names that are object types with properties,
        // so we can validate $ref targets during type resolution.
        let object_schemas = self.collect_object_schema_names(&spec);

        let resources = self.extract_resources(&spec, &object_schemas)?;

        tracing::info!("  Extracted {} resource types", resources.len());

        // Dedup pass: detect PascalCase name collisions and append numeric suffix
        let resources = Self::dedup_type_names(resources);

        // Extract endpoint information for ResourceIdentifier generation
        let endpoints = self.extract_endpoints(&spec_content);

        // Note: We no longer filter out 'trivial' types (single serde_json::Value field)
        // because types like GoogleCloudAiplatformV1ContentMap have semantic meaning
        // even with one `additionalProperties` field that references another type.

        let rust_code = self.generate_rust(label, &resources, &endpoints)?;

        std::fs::write(output_path, rust_code).map_err(|e| GenResourceError::WriteFile {
            path: output_path.display().to_string(),
            source: e,
        })?;

        let _ = Command::new("rustfmt").arg(output_path).output();

        tracing::info!("  Generated: {}", output_path.display());
        Ok(())
    }

    /// Discover providers with specs in artefacts directory.
    fn discover_providers(&self) -> Result<Vec<String>, GenResourceError> {
        let mut providers = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.artefacts_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let spec_path = entry.path().join("openapi.json");
                if spec_path.exists() {
                    providers.push(name);
                }
            }
        }

        Ok(providers)
    }

    /// Collect schema names that resolve to object types with properties.
    ///
    /// Used to validate `$ref` targets: if a ref points to a non-object schema,
    /// the field should fall back to `serde_json::Value`.
    fn collect_object_schema_names(&self, spec: &Value) -> BTreeSet<String> {
        let mut names = BTreeSet::new();

        let schemas_maps: Vec<&serde_json::Map<String, Value>> = {
            let mut maps = Vec::new();
            if let Some(s) = spec
                .get("components")
                .and_then(|c| c.get("schemas"))
                .and_then(|s| s.as_object())
            {
                maps.push(s);
            }
            if let Some(s) = spec.get("schemas").and_then(|s| s.as_object()) {
                maps.push(s);
            }
            // Consolidated format
            if maps.is_empty() {
                if let Some(obj) = spec.as_object() {
                    for (_api_name, api_spec) in obj {
                        if let Some(s) = api_spec
                            .get("components")
                            .and_then(|c| c.get("schemas"))
                            .and_then(|s| s.as_object())
                        {
                            maps.push(s);
                        } else if let Some(s) =
                            api_spec.get("schemas").and_then(|s| s.as_object())
                        {
                            maps.push(s);
                        }
                    }
                }
            }
            maps
        };

        for schemas in schemas_maps {
            for (name, value) in schemas {
                // Include all object types and composition types (allOf/oneOf/anyOf)
                // Composition types are always included regardless of base type
                let has_composition = value.get("allOf").is_some()
                    || value.get("oneOf").is_some()
                    || value.get("anyOf").is_some();
                let is_object = value.get("type").and_then(|t| t.as_str()) == Some("object");

                // Also include response types (they may be allOf-refs-only but are still needed)
                let is_response = name.to_lowercase().contains("response");

                if is_object || has_composition || is_response {
                    names.insert(name.clone());
                }
            }
        }

        names
    }

    /// Deduplicate type names by appending numeric suffixes on collision.
    fn dedup_type_names(mut resources: Vec<ResourceDef>) -> Vec<ResourceDef> {
        let mut seen: HashMap<String, usize> = HashMap::new();

        for resource in &mut resources {
            let count = seen.entry(resource.name.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                resource.name = format!("{}{}", resource.name, count);
            }
        }

        resources
    }

    /// Extract resource definitions from OpenAPI spec.
    ///
    /// Handles multiple formats:
    /// - Standard OpenAPI 3.x: `components/schemas`
    /// - GCP Discovery: `schemas` at top level
    /// - Consolidated: object where each value is an API spec (GCP multi-API format)
    fn extract_resources(
        &self,
        spec: &Value,
        object_schemas: &BTreeSet<String>,
    ) -> Result<Vec<ResourceDef>, GenResourceError> {
        let mut resources = Vec::new();

        // Try standard OpenAPI: components/schemas
        if let Some(schemas) = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
        {
            for (schema_name, schema_value) in schemas {
                if let Some(resource) =
                    self.extract_resource(schema_name, schema_value, object_schemas)
                {
                    resources.push(resource);
                }
            }
            return Ok(resources);
        }

        // Try GCP Discovery format: top-level `schemas`
        if let Some(schemas) = spec.get("schemas").and_then(|s| s.as_object()) {
            for (schema_name, schema_value) in schemas {
                if let Some(resource) =
                    self.extract_resource(schema_name, schema_value, object_schemas)
                {
                    resources.push(resource);
                }
            }
            return Ok(resources);
        }

        // Try consolidated format: each top-level key is an API spec
        if let Some(obj) = spec.as_object() {
            for (_api_name, api_spec) in obj {
                // Each entry might be an OpenAPI spec or a Discovery doc
                if let Some(schemas) = api_spec
                    .get("components")
                    .and_then(|c| c.get("schemas"))
                    .and_then(|s| s.as_object())
                {
                    for (schema_name, schema_value) in schemas {
                        if let Some(resource) =
                            self.extract_resource(schema_name, schema_value, object_schemas)
                        {
                            resources.push(resource);
                        }
                    }
                } else if let Some(schemas) = api_spec.get("schemas").and_then(|s| s.as_object()) {
                    for (schema_name, schema_value) in schemas {
                        if let Some(resource) =
                            self.extract_resource(schema_name, schema_value, object_schemas)
                        {
                            resources.push(resource);
                        }
                    }
                }
            }
        }

        Ok(resources)
    }

    /// Extract endpoint information using foundation_openapi.
    ///
    /// This builds a mapping of response types to their endpoint parameters,
    /// which is used to generate ResourceIdentifier implementations.
    fn extract_endpoints(&self, spec_content: &str) -> Vec<CodegenEndpoint> {
        // Use foundation_openapi for all endpoint extraction
        let Ok(processor) = foundation_openapi::process_spec(spec_content) else {
            tracing::warn!("Failed to parse spec for endpoint extraction");
            return Vec::new();
        };

        // Get endpoints from foundation_openapi
        let endpoints = processor.endpoints();

        // Convert to CodegenEndpoint format for codegen
        endpoints
            .into_iter()
            .map(|ep| {
                let response_type = ep.response_type.map(|rt| rt.as_rust_type().to_string());
                CodegenEndpoint {
                    operation_id: ep.operation_id,
                    response_type,
                    path_params: ep.path_params,
                }
            })
            .collect()
    }

    /// Extract a single resource from a schema.
    fn extract_resource(
        &self,
        schema_name: &str,
        schema_value: &Value,
        object_schemas: &BTreeSet<String>,
    ) -> Option<ResourceDef> {
        let schema: Schema = serde_json::from_value(schema_value.clone()).ok()?;

        // Check if this is a response or request type (these should always be included)
        let is_response_wrapper = schema_name.to_lowercase().contains("api_response") || schema_name.to_lowercase().contains("response");
        let is_request_type = schema_name.to_lowercase().contains("request");

        // Only process object types (including allOf/oneOf/anyOf which are implicitly objects)
        // OR response/request types which are always needed
        let is_object = schema.schema_type.as_deref() == Some("object")
            || schema.all_of.is_some()
            || schema.one_of.is_some()
            || schema.any_of.is_some();
        if !is_object && !is_response_wrapper && !is_request_type {
            return None;
        }

        let has_own_properties = schema.properties.as_ref().map_or(false, |p| !p.is_empty());
        let is_ref_only = schema.ref_path.is_some() && !has_own_properties;
        let has_composition = schema.one_of.is_some() || schema.any_of.is_some();
        let has_allof = schema.all_of.is_some();

        // Skip simple ref-only types (not composition, not response/request, not allOf)
        if is_ref_only && !is_response_wrapper && !is_request_type && !has_composition && !has_allof {
            return None;
        }

        // Merge properties from allOf members if present
        let mut properties = schema.properties.unwrap_or_default();
        if let Some(allof) = &schema.all_of {
            for member in allof {
                if let Some(member_props) = &member.properties {
                    for (key, value) in member_props {
                        properties.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
            }
        }

        let rust_name = self.to_pascal_case(schema_name);
        // Rename types that conflict with Rust built-ins and keywords
        let rust_name = match rust_name.as_str() {
            "Option" => "ApiOption".to_string(),
            "Value" => "ApiValue".to_string(),
            "Result" => "ApiResult".to_string(),
            "Ok" => "ApiOk".to_string(),
            "Err" => "ApiErr".to_string(),
            "Some" => "ApiSome".to_string(),
            "None" => "ApiNone".to_string(),
            "Box" => "ApiBox".to_string(),
            "Vec" => "ApiVec".to_string(),
            "String" => "ApiString".to_string(),
            _ => rust_name,
        };
        let description = schema.description.clone();

        let mut fields = Vec::new();
        let mut seen_field_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (field_name, field_schema) in properties {
            let snake_case_name = self.to_snake_case(&field_name);

            // Skip duplicate fields (same snake_case name)
            if seen_field_names.contains(&snake_case_name) {
                continue;
            }
            seen_field_names.insert(snake_case_name.clone());

            let field_ty = self.schema_to_rust_type(&field_schema, object_schemas);
            let required = schema.required.contains(&field_name);

            // Build description, appending enum TODO comment if applicable
            let description = {
                let mut desc = field_schema.description.clone();
                if let Some(enum_vals) = &field_schema.enum_values {
                    let vals: Vec<String> = enum_vals
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => format!("\"{s}\""),
                            other => other.to_string(),
                        })
                        .collect();
                    let todo = format!("TODO: enum values: [{}]", vals.join(", "));
                    desc = Some(match desc {
                        Some(d) => format!("{d} // {todo}"),
                        None => todo,
                    });
                }
                desc
            };

            fields.push(FieldDef {
                name: snake_case_name,
                original_name: field_name.clone(),
                ty: field_ty,
                required,
                description,
            });
        }

        // For schemas with no fields, generate a single value: serde_json::Value field
        // This handles composition types (oneOf/anyOf/allOf) and empty object types
        // Users can work with the raw JSON value for complex cases

        Some(ResourceDef {
            name: rust_name,
            schema_name: schema_name.to_string(),
            description,
            fields,
            is_root: false, // Could be determined by analyzing path references
        })
    }

    /// Convert OpenAPI schema type to Rust type.
    ///
    /// `object_schemas` contains schema names that are known to be object types
    /// with properties, used to validate `$ref` targets.
    fn schema_to_rust_type(&self, schema: &Schema, object_schemas: &BTreeSet<String>) -> String {
        match schema.schema_type.as_deref() {
            Some("string") => "String".to_string(),
            Some("integer") => match schema.format.as_deref() {
                Some("int32") => "i32".to_string(),
                Some("int64") => "i64".to_string(),
                _ => "i64".to_string(),
            },
            Some("number") => match schema.format.as_deref() {
                Some("float") => "f32".to_string(),
                Some("double") => "f64".to_string(),
                _ => "f64".to_string(),
            },
            Some("boolean") => "bool".to_string(),
            Some("array") => {
                if let Some(items) = &schema.items {
                    let inner_ty = self.schema_to_rust_type(items, object_schemas);
                    format!("::std::vec::Vec<{inner_ty}>")
                } else {
                    "::std::vec::Vec<serde_json::Value>".to_string()
                }
            }
            Some("object") => "serde_json::Value".to_string(),
            Some(null) if null == "null" => "::core::option::Option<serde_json::Value>".to_string(),
            None => {
                // Could be a reference or complex type
                if let Some(ref_path) = &schema.ref_path {
                    self.resolve_ref(ref_path, object_schemas)
                } else if let Some(all_of) = &schema.all_of {
                    // allOf with a single $ref: use the referenced type
                    if all_of.len() == 1 {
                        if let Some(ref_path) = &all_of[0].ref_path {
                            return self.resolve_ref(ref_path, object_schemas);
                        }
                    }
                    "serde_json::Value".to_string()
                } else if schema.any_of.is_some() || schema.one_of.is_some() {
                    "serde_json::Value".to_string()
                } else {
                    "serde_json::Value".to_string()
                }
            }
            Some(other) => {
                // Unknown type, use Value
                tracing::warn!("Unknown schema type: {other}, using Value");
                "serde_json::Value".to_string()
            }
        }
    }

    /// Resolve a `$ref` path to a Rust type name.
    ///
    /// If the referenced schema is a known object type, returns the PascalCase name.
    /// Otherwise falls back to `serde_json::Value`.
    fn resolve_ref(&self, ref_path: &str, object_schemas: &BTreeSet<String>) -> String {
        let ref_name = ref_path.trim_start_matches("#/components/schemas/");
        // Also handle GCP Discovery refs like "#/schemas/Foo"
        let ref_name = ref_name.trim_start_matches("#/schemas/");
        if object_schemas.contains(ref_name) {
            let ty = self.to_pascal_case(ref_name);
            // Rename types that conflict with Rust built-ins
            match ty.as_str() {
                "Option" => "ApiOption".to_string(),
                "Value" => "ApiValue".to_string(),
                "Result" => "ApiResult".to_string(),
                "Ok" => "ApiOk".to_string(),
                "Err" => "ApiErr".to_string(),
                "Some" => "ApiSome".to_string(),
                "None" => "ApiNone".to_string(),
                "Box" => "ApiBox".to_string(),
                "Vec" => "ApiVec".to_string(),
                "String" => "ApiString".to_string(),
                _ => ty,
            }
        } else {
            "serde_json::Value".to_string()
        }
    }

    /// Convert OpenAPI identifier to snake_case Rust field name.
    fn to_snake_case(&self, name: &str) -> String {
        // Split on non-alphanumeric, camelCase boundaries, and underscores
        let mut parts = Vec::new();
        let mut current = String::new();

        let chars: Vec<char> = name.chars().collect();
        for i in 0..chars.len() {
            let c = chars[i];
            if !c.is_alphanumeric() {
                // Non-alphanumeric = word boundary
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else if c.is_uppercase() {
                // camelCase boundary: start new word on uppercase
                if !current.is_empty() {
                    // Check if this is an acronym (e.g., "URL" in "getURLPath")
                    let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
                    if next_is_lower || current.chars().last().map_or(false, |p| p.is_lowercase()) {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                current.push(c.to_ascii_lowercase());
            } else {
                current.push(c.to_ascii_lowercase());
            }
        }
        if !current.is_empty() {
            parts.push(current);
        }

        let result = parts.join("_");

        // Handle Rust keywords and numeric starting characters
        let escaped = Self::escape_keyword(&result);

        // Prepend "field_" if starts with digit (more semantic than just "_")
        // e.g., "1" -> "field_1", "24" -> "field_24"
        if escaped.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            format!("field_{}", escaped)
        } else {
            escaped
        }
    }

    /// Convert OpenAPI identifier to PascalCase Rust type name.
    fn to_pascal_case(&self, name: &str) -> String {
        // First get snake_case parts
        let snake = self.to_snake_case(name);
        let pascal: String = snake
            .split('_')
            .filter(|s| !s.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        let upper: String = first.to_uppercase().collect();
                        upper + chars.as_str()
                    }
                    None => String::new(),
                }
            })
            .collect();

        if pascal.is_empty() {
            "Unknown".to_string()
        } else {
            pascal
        }
    }

    /// Escape Rust keywords by appending underscore.
    fn escape_keyword(name: &str) -> String {
        match name {
            // Strict keywords
            "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
            | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
            | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
            | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "async" | "await" | "dyn"
            // Reserved/weak keywords
            | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override" | "priv"
            | "typeof" | "unsized" | "virtual" | "yield" => format!("{name}_"),
            _ => name.to_string(),
        }
    }

    /// Generate Rust source code from resource definitions.
    fn generate_rust(
        &self,
        provider: &str,
        resources: &[ResourceDef],
        endpoints: &[CodegenEndpoint],
    ) -> Result<String, GenResourceError> {
        let mut out = String::with_capacity(256 * 1024);

        // Extract base provider name for display (handle "gcp/compute" labels)
        let base_provider = provider.split('/').next().unwrap_or(provider);
        let provider_display = match base_provider {
            "gcp" => "Google Cloud Platform",
            "cloudflare" => "Cloudflare",
            "fly-io" => "Fly.io",
            "neon" => "Neon",
            "supabase" => "Supabase",
            "stripe" => "Stripe",
            "mongodb-atlas" => "MongoDB Atlas",
            "prisma-postgres" => "Prisma Postgres",
            "planetscale" => "PlanetScale",
            other => other,
        };
        // If there's a sub-API, include it in the display
        let provider_display = if provider.contains('/') {
            let sub_api = provider.split('/').nth(1).unwrap_or("");
            format!("{provider_display} - {sub_api}")
        } else {
            provider_display.to_string()
        };

        writeln!(
            out,
            "//! Auto-generated resource types for {provider_display}.\n\
             //!\n\
             //! This file is generated by `cargo run --bin ewe_platform gen_resource_types`.\n\
             //! DO NOT EDIT MANUALLY.\n\
             //!\n\
             //! Generated from OpenAPI spec in `artefacts/cloud_providers/{provider}/openapi.json`.\n\
             \n\
             #![allow(clippy::too_many_lines)]\n\
             #![allow(clippy::cognitive_complexity)]\n\
             #![allow(dead_code)]\n\
             #![allow(unused_imports)]\n\
             \n\
             use serde::{{Deserialize, Serialize}};\n\
             use foundation_macros::JsonHash;\n\
             use super::*;\n\
             "
        )
        .map_err(|e| GenResourceError::WriteFile {
            path: format!("generated code for {provider}"),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;

        // Generate structs for each resource
        for resource in resources {
            self.generate_struct(&mut out, resource)?;
        }

        // Generate ResourceIdentifier implementations for response types
        self.generate_resource_identifier_impls(&mut out, endpoints, provider)?;

        Ok(out)
    }

    /// Generate ResourceIdentifier implementations for response types.
    ///
    /// For each endpoint with a response type, generates an impl block that
    /// computes the resource ID from path parameters.
    fn generate_resource_identifier_impls(
        &self,
        out: &mut String,
        endpoints: &[CodegenEndpoint],
        provider: &str,
    ) -> Result<(), GenResourceError> {
        // Build provider prefix for resource kinds
        let provider_prefix = provider.replace('-', "_");

        // Deduplicate by response type - we only want one ResourceIdentifier impl per type
        let mut seen_response_types: std::collections::HashSet<String> = std::collections::HashSet::new();

        writeln!(out)?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// ResourceIdentifier implementations")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        for endpoint in endpoints {
            // Skip endpoints without response types (e.g., 204 No Content)
            let response_type = match &endpoint.response_type {
                Some(t) => t,
                None => continue,
            };

            // Skip if we already generated an impl for this type
            if !seen_response_types.insert(response_type.clone()) {
                continue;
            }

            // Generate Args type name from operation_id
            let args_type = format!("{}Args", self.to_pascal_case(&endpoint.operation_id));

            // Build resource kind: e.g., "prisma_postgres::ComputeservicesGetResponse"
            let resource_kind = format!("{}::{}", provider_prefix, response_type);

            writeln!(out, "/// ResourceIdentifier implementation for {}.", response_type)?;
            writeln!(out, "///")?;
            writeln!(out, "/// WHY: Enables automatic state tracking via StoreStateIdentifierTask.")?;
            writeln!(out, "///")?;
            writeln!(out, "/// HOW: Computes resource ID from input path parameters.")?;
            writeln!(out, "impl ResourceIdentifier<{}> for {} {{", args_type, response_type)?;
            writeln!(out, "    fn generate_resource_id(&self, input: &{}) -> String {{", args_type)?;

            // Generate resource ID from path parameters
            if !endpoint.path_params.is_empty() {
                // Build format string: "provider::type/{} /{}" (one {} per param)
                write!(out, "        format!(\"{}", resource_kind)?;
                for _ in 0..endpoint.path_params.len() {
                    write!(out, "/{{}}")?;
                }
                write!(out, "\"")?;
                for param in &endpoint.path_params {
                    let param_name = self.to_snake_case(param);
                    write!(out, ", input.{}", param_name)?;
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
            writeln!(out, "        \"{}\"", provider.split('/').next().unwrap_or(provider))?;
            writeln!(out, "    }}")?;
            writeln!(out, "}}")?;
            writeln!(out)?;
        }

        Ok(())
    }

    /// Generate a single struct definition.
    fn generate_struct(
        &self,
        out: &mut String,
        resource: &ResourceDef,
    ) -> Result<(), GenResourceError> {
        self.generate_struct_with_box(out, resource, &std::collections::HashSet::new())
    }

    /// Generate a single struct definition, wrapping recursive type references in Box.
    fn generate_struct_with_box(
        &self,
        out: &mut String,
        resource: &ResourceDef,
        recursive_types: &std::collections::HashSet<String>,
    ) -> Result<(), GenResourceError> {
        // Write struct-level doc comment (full description, sanitized)
        // Split by newlines and write each line as a separate doc comment
        if let Some(desc) = &resource.description {
            let sanitized = sanitize_doc_comment(desc, false);
            let mut has_written_doc = false;
            for line in sanitized.lines() {
                let trimmed = line.trim();
                // Skip empty lines unless we've already written some doc content
                if trimmed.is_empty() {
                    if has_written_doc {
                        writeln!(out, "///").map_err(|e| GenResourceError::WriteFile {
                            path: format!("struct {}", resource.name),
                            source: std::io::Error::new(std::io::ErrorKind::Other, e),
                        })?;
                    }
                } else {
                    writeln!(out, "/// {line}").map_err(|e| GenResourceError::WriteFile {
                        path: format!("struct {}", resource.name),
                        source: std::io::Error::new(std::io::ErrorKind::Other, e),
                    })?;
                    has_written_doc = true;
                }
            }
        } else {
            writeln!(out, "/// {} resource type.", resource.name).map_err(|e| {
                GenResourceError::WriteFile {
                    path: format!("struct {}", resource.name),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                }
            })?;
        }
        writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]").map_err(|e| {
            GenResourceError::WriteFile {
                path: format!("struct {}", resource.name),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            }
        })?;
        // Deny unknown fields is too strict for evolving APIs; just generate the struct
        writeln!(out, "pub struct {} {{", resource.name).map_err(|e| {
            GenResourceError::WriteFile {
                path: format!("struct {}", resource.name),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            }
        })?;

        // For composition types with no fields, generate a single value: serde_json::Value field
        // instead of an empty struct
        if resource.fields.is_empty() {
            writeln!(out, "    pub value: serde_json::Value,").map_err(|e| {
                GenResourceError::WriteFile {
                    path: format!("struct {}", resource.name),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                }
            })?;
        } else {
            // Write fields
            for field in &resource.fields {
                if let Some(desc) = &field.description {
                    // Field-level comments: first line only, sanitized
                    let sanitized = sanitize_doc_comment(desc, true);
                    writeln!(out, "    /// {sanitized}").map_err(|e| GenResourceError::WriteFile {
                        path: format!("field {}.{}", resource.name, field.name),
                        source: std::io::Error::new(std::io::ErrorKind::Other, e),
                    })?;
                }
                // Add serde attributes
                if field.name != field.original_name && !field.required {
                    // Both rename and default for optional renamed fields
                    writeln!(
                        out,
                        "    #[serde(default, rename = \"{}\")]",
                        field.original_name
                    )
                    .map_err(|e| GenResourceError::WriteFile {
                        path: format!("field {}.{}", resource.name, field.name),
                        source: std::io::Error::new(std::io::ErrorKind::Other, e),
                    })?;
                } else if field.name != field.original_name {
                    // Rename only for required renamed fields
                    writeln!(out, "    #[serde(rename = \"{}\")]", field.original_name).map_err(
                        |e| GenResourceError::WriteFile {
                            path: format!("field {}.{}", resource.name, field.name),
                            source: std::io::Error::new(std::io::ErrorKind::Other, e),
                        },
                    )?;
                } else if !field.required {
                    // Default only for optional non-renamed fields
                    writeln!(out, "    #[serde(default)]").map_err(|e| {
                        GenResourceError::WriteFile {
                            path: format!("field {}.{}", resource.name, field.name),
                            source: std::io::Error::new(std::io::ErrorKind::Other, e),
                        }
                    })?;
                }

                // Wrap recursive type references in Box to break infinite size cycles
                let field_ty = self.wrap_recursive_type(&field.ty, field.required, recursive_types);
                writeln!(out, "    pub {}: {},", field.name, field_ty).map_err(|e| {
                    GenResourceError::WriteFile {
                        path: format!("field {}.{}", resource.name, field.name),
                        source: std::io::Error::new(std::io::ErrorKind::Other, e),
                    }
                })?;
            }
        }

        writeln!(out, "}}\n").map_err(|e| GenResourceError::WriteFile {
            path: format!("struct {}", resource.name),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;

        Ok(())
    }

    /// Wrap recursive type references in Box to break infinite size cycles.
    fn wrap_recursive_type(
        &self,
        ty: &str,
        required: bool,
        recursive_types: &std::collections::HashSet<String>,
    ) -> String {
        // Extract type names and check if any are recursive
        let type_names = extract_type_names(ty);
        let has_recursive = type_names.iter().any(|t| recursive_types.contains(t));

        if !has_recursive {
            // No recursion, return as-is
            if required {
                return ty.to_string();
            } else {
                return format!("::core::option::Option<{}>", ty);
            }
        }

        // Wrap recursive references in Box
        let result = ty.to_string();

        // Handle Vec<T> - wrap inner recursive types
        if result.starts_with("::std::vec::Vec<") || result.starts_with("Vec<") {
            let prefix = if result.starts_with("::std::vec::Vec<") { "::std::vec::Vec<" } else { "Vec<" };
            let suffix = result.strip_prefix(prefix).unwrap_or(&result);
            if let Some(inner) = suffix.strip_suffix('>') {
                let wrapped_inner = self.wrap_recursive_type(inner, true, recursive_types);
                return format!("{}{}>", prefix, wrapped_inner);
            }
        }

        // Handle Option<T> - wrap inner recursive types
        if result.starts_with("::core::option::Option<") || result.starts_with("Option<") {
            let prefix = if result.starts_with("::core::option::Option<") { "::core::option::Option<" } else { "Option<" };
            let suffix = result.strip_prefix(prefix).unwrap_or(&result);
            if let Some(inner) = suffix.strip_suffix('>') {
                let wrapped_inner = self.wrap_recursive_type(inner, true, recursive_types);
                return format!("{}{}>", prefix, wrapped_inner);
            }
        }

        // Handle HashMap/BTreeMap - wrap recursive values
        for map_prefix in ["::std::collections::HashMap<", "HashMap<", "::std::collections::BTreeMap<", "BTreeMap<"] {
            if result.starts_with(map_prefix) {
                let suffix = result.strip_prefix(map_prefix).unwrap_or(&result);
                if let Some(inner) = suffix.strip_suffix('>') {
                    // Maps have K, V - only wrap V
                    if let Some(comma_pos) = inner.find(',') {
                        let key = &inner[..comma_pos].trim();
                        let value = inner[comma_pos + 1..].trim();
                        let wrapped_value = self.wrap_recursive_type(value, true, recursive_types);
                        return format!("{}{}, {}>", map_prefix, key, wrapped_value);
                    }
                }
            }
        }

        // Simple type reference - wrap in Box if recursive
        if recursive_types.contains(ty) {
            if required {
                return format!("::std::boxed::Box<{}>", ty);
            } else {
                return format!("::core::option::Option<::std::boxed::Box<{}>>", ty);
            }
        }

        if required {
            ty.to_string()
        } else {
            format!("::core::option::Option<{}>", ty)
        }
    }

    /// Generate mod.rs for a resources directory.
    fn generate_mod_rs(
        &self,
        provider: &str,
        apis: &[String],
    ) -> Result<(), GenResourceError> {
        let mut out = String::new();

        let provider_display = match provider.split('/').next().unwrap_or(provider) {
            "gcp" => "Google Cloud Platform",
            "cloudflare" => "Cloudflare",
            "fly-io" => "Fly.io",
            "neon" => "Neon",
            "supabase" => "Supabase",
            "stripe" => "Stripe",
            "mongodb-atlas" => "MongoDB Atlas",
            "prisma-postgres" => "Prisma Postgres",
            "planetscale" => "PlanetScale",
            other => other,
        };

        writeln!(out, "//! Auto-generated resource types for {provider_display}.")
            .map_err(|e| write_fmt_error(e))?;
        writeln!(out, "//!")
            .map_err(|e| write_fmt_error(e))?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_resource_types`.")
            .map_err(|e| write_fmt_error(e))?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")
            .map_err(|e| write_fmt_error(e))?;
        writeln!(out)
            .map_err(|e| write_fmt_error(e))?;

        // Multi-API - declare submodules and re-exports
        // Each API is a file: resources/{api}.rs
        for api in apis {
            writeln!(out, "pub mod {api};")
                .map_err(|e| write_fmt_error(e))?;
        }
        writeln!(out)
            .map_err(|e| write_fmt_error(e))?;
        for api in apis {
            writeln!(out, "pub use {api}::*;")
                .map_err(|e| write_fmt_error(e))?;
        }

        // Output to providers/{provider}/resources/mod.rs
        let mod_path = self.output_dir.join(provider).join("resources").join("mod.rs");
        std::fs::write(&mod_path, out)?;
        Ok(())
    }

    /// Generate resource types from a single spec file with provider-level feature flag.
    fn generate_from_spec_simple(
        &self,
        label: &str,
        spec_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Result<(), GenResourceError> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        tracing::info!("Generating resource types for {label}...");

        let spec_content = std::fs::read_to_string(spec_path)
            .map_err(|e| GenResourceError::ReadFile { path: spec_path.display().to_string(), source: e })?;

        let spec: Value = serde_json::from_str(&spec_content)
            .map_err(|e| GenResourceError::Json { path: spec_path.display().to_string(), source: e })?;

        let object_schemas = self.collect_object_schema_names(&spec);
        let resources = self.extract_resources(&spec, &object_schemas)?;
        let resources = Self::dedup_type_names(resources);

        // Extract endpoint information for ResourceIdentifier generation
        let endpoints = self.extract_endpoints(&spec_content);

        let rust_code = self.generate_rust_simple(label, &resources, &endpoints)?;

        std::fs::write(output_path, rust_code)
            .map_err(|e| GenResourceError::WriteFile { path: output_path.display().to_string(), source: e })?;
        eprintln!("  Wrote output file");

        let _ = Command::new("rustfmt").arg(output_path).output();
        tracing::info!("  Generated: {}", output_path.display());
        Ok(())
    }

    /// Normalize provider name for feature flags (replace `-` with `_`).
    fn provider_to_feature_name(provider: &str) -> String {
        provider.replace('-', "_")
    }
    fn generate_mod_rs_simple(&self, provider: &str, apis: &[String]) -> Result<(), GenResourceError> {
        let mut out = String::new();
        let provider_display = Self::provider_display(provider);
        let feature_name = Self::provider_to_feature_name(provider);

        writeln!(out, "//! Auto-generated resource types for {provider_display}.").map_err(write_fmt_error)?;
        writeln!(out, "//!").map_err(write_fmt_error)?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_resource_types`.").map_err(write_fmt_error)?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.").map_err(write_fmt_error)?;
        writeln!(out, "//!").map_err(write_fmt_error)?;
        writeln!(out, "//! Feature flag: `{feature_name}`").map_err(write_fmt_error)?;
        writeln!(out).map_err(write_fmt_error)?;
        writeln!(out, "#![cfg(feature = \"{feature_name}\")]").map_err(write_fmt_error)?;
        writeln!(out).map_err(write_fmt_error)?;

        for api in apis { writeln!(out, "pub mod {api};").map_err(write_fmt_error)?; }
        writeln!(out).map_err(write_fmt_error)?;
        for api in apis { writeln!(out, "pub use {api}::*;").map_err(write_fmt_error)?; }

        std::fs::write(self.output_dir.join(provider).join("resources").join("mod.rs"), out)?;
        Ok(())
    }

    /// Generate Rust source code with provider-level feature flag.
    fn generate_rust_simple(&self, label: &str, resources: &[ResourceDef], endpoints: &[CodegenEndpoint]) -> Result<String, GenResourceError> {
        let mut out = String::with_capacity(256 * 1024);
        let provider = label.split('/').next().unwrap_or(label);
        let api_name = label.split('/').nth(1).unwrap_or("");
        let feature_name = Self::provider_to_feature_name(provider);
        let display = if !api_name.is_empty() {
            format!("{} - {}", Self::provider_display(provider), api_name)
        } else {
            Self::provider_display(provider).to_string()
        };

        writeln!(out,
            "//! Auto-generated resource types for {display}.\n\
             //!\n\
             //! This file is generated by `cargo run --bin ewe_platform gen_resource_types`.\n\
             //! DO NOT EDIT MANUALLY.\n\
             //!\n\
             //! Feature flag: `{feature_name}`\n\
             \n\
             #![cfg(feature = \"{feature_name}\")]\n\
             \n\
             use serde::{{Deserialize, Serialize}};\n\
             use foundation_macros::JsonHash;\n\
             use super::*;\n"
        ).map_err(|e| GenResourceError::WriteFile { path: format!("generated code for {label}"), source: std::io::Error::new(std::io::ErrorKind::Other, e) })?;

        // Topologically sort resources and detect recursive types
        let (sorted, recursive_types) = self.sort_resources_topo(resources);

        // Generate structs, wrapping recursive field references in Box
        for resource in sorted {
            self.generate_struct_with_box(&mut out, resource, &recursive_types)?;
        }

        // Generate ResourceIdentifier implementations for response types
        self.generate_resource_identifier_impls(&mut out, endpoints, provider)?;

        Ok(out)
    }

    /// Topologically sort resources so types with no dependencies come first.
    /// Types referenced by other types must be defined before the referencing type.
    /// Also detects cycles and marks recursive field references for Box wrapping.
    fn sort_resources_topo<'a>(&self, resources: &'a [ResourceDef]) -> (Vec<&'a ResourceDef>, std::collections::HashSet<String>) {
        use std::collections::{HashMap, HashSet, VecDeque};

        // Build a map of name -> resource and collect dependencies
        let name_to_idx: HashMap<&str, usize> = resources.iter().enumerate().map(|(i, r)| (r.name.as_str(), i)).collect();
        let mut deps: Vec<Vec<usize>> = Vec::with_capacity(resources.len());
        let mut in_degree: Vec<usize> = Vec::with_capacity(resources.len());

        for (i, resource) in resources.iter().enumerate() {
            let mut dep_idxs = Vec::new();
            let mut seen = HashSet::new();

            for field in &resource.fields {
                for ty_name in extract_type_names(&field.ty) {
                    if let Some(&dep_idx) = name_to_idx.get(ty_name.as_str()) {
                        if dep_idx != i && !seen.contains(&dep_idx) {
                            dep_idxs.push(dep_idx);
                            seen.insert(dep_idx);
                        }
                    }
                }
            }
            deps.push(dep_idxs);
            in_degree.push(0);
        }

        // Calculate in-degrees
        for d in &deps {
            for &dep in d {
                in_degree[dep] += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<usize> = in_degree.iter().enumerate().filter(|&(_, &d)| d == 0).map(|(i, _)| i).collect();
        let mut result = Vec::with_capacity(resources.len());

        while let Some(idx) = queue.pop_front() {
            result.push(&resources[idx]);
            for &dep in &deps[idx] {
                in_degree[dep] -= 1;
                if in_degree[dep] == 0 {
                    queue.push_back(dep);
                }
            }
        }

        // Detect cycles - types not in result are part of cycles
        let mut recursive_types: HashSet<String> = HashSet::new();
        if result.len() < resources.len() {
            // Find types involved in cycles
            for r in resources.iter() {
                if !result.iter().any(|&rr| rr.name == r.name) {
                    recursive_types.insert(r.name.clone());
                }
            }
            // Append remaining types (in cycle) to result
            for r in resources.iter() {
                if !result.iter().any(|&rr| rr.name == r.name) {
                    result.push(r);
                }
            }
        }

        // Also detect self-referential types (direct recursion)
        for resource in resources.iter() {
            for field in &resource.fields {
                for ty_name in extract_type_names(&field.ty) {
                    if ty_name == resource.name {
                        recursive_types.insert(resource.name.clone());
                    }
                }
            }
        }

        (result, recursive_types)
    }

    /// Get display name for a provider.
    fn provider_display(provider: &str) -> Cow<'static, str> {
        match provider {
            "gcp" => Cow::Borrowed("Google Cloud Platform"),
            "cloudflare" => Cow::Borrowed("Cloudflare"),
            "fly-io" => Cow::Borrowed("Fly.io"),
            "neon" => Cow::Borrowed("Neon"),
            "supabase" => Cow::Borrowed("Supabase"),
            "stripe" => Cow::Borrowed("Stripe"),
            "mongodb-atlas" => Cow::Borrowed("MongoDB Atlas"),
            "prisma-postgres" => Cow::Borrowed("Prisma Postgres"),
            "planetscale" => Cow::Borrowed("PlanetScale"),
            other => Cow::Owned(other.to_string()),
        }
    }
}

/// Extract Rust type names from a type string (e.g., "Option<Foo>" -> ["Foo"]).
fn extract_type_names(ty: &str) -> Vec<String> {
    let mut result = Vec::new();
    // Remove common wrappers
    let ty = ty
        .replace("::core::option::Option<", "")
        .replace("::std::vec::Vec<", "")
        .replace("Option<", "")
        .replace("Vec<", "")
        .replace("HashMap<", "")
        .replace("BTreeMap<", "")
        .replace("BTreeSet<", "")
        .replace("::serde_json::Value", "")
        .replace("String", "")
        .replace("bool", "")
        .replace("i32", "")
        .replace("i64", "")
        .replace("u32", "")
        .replace("u64", "")
        .replace("f32", "")
        .replace("f64", "")
        .replace("char", "");

    // Extract remaining identifiers (PascalCase type names)
    // They may be separated by commas or angle brackets
    let mut current = String::new();
    let mut depth: i32 = 0;
    for c in ty.chars() {
        match c {
            '<' | ',' | '>' | ' ' => {
                if !current.is_empty() && depth == 0 {
                    // Check if it looks like a type name (starts with uppercase)
                    if current.chars().next().map_or(false, |c| c.is_uppercase()) {
                        result.push(current.clone());
                    }
                    current.clear();
                }
                if c == '<' {
                    depth += 1;
                } else if c == '>' {
                    depth = depth.saturating_sub(1);
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() && current.chars().next().map_or(false, |c| c.is_uppercase()) {
        result.push(current);
    }

    result
}
