//! WHY: Generates Rust type definitions from OpenAPI specs for cloud providers.
//!
//! WHAT: Reads OpenAPI specs from `artefacts/cloud_providers/`, parses resource definitions,
//! and generates Rust structs that represent cloud resources (e.g., GCP Compute Instance,
//! Cloudflare Worker, etc.).
//!
//! HOW: Uses `serde_json` to parse OpenAPI specs, extracts schema definitions from
//! `components/schemas`, maps OpenAPI types to Rust types, and generates Rust source files
//! using string templates.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

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

// ---------------------------------------------------------------------------
// OpenAPI spec structures
// ---------------------------------------------------------------------------

/// WHY: Minimal OpenAPI spec deserialization for resource extraction.
///
/// WHAT: Captures only the fields needed for code generation.
///
/// HOW: Uses serde for flexible JSON parsing.
#[derive(Debug, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: Info,
    #[serde(default)]
    pub paths: BTreeMap<String, PathItem>,
    #[serde(default)]
    pub components: Components,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    pub title: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Default)]
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
}

#[derive(Debug, Deserialize, Default)]
pub struct Operation {
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<RequestBody>,
    #[serde(default)]
    pub responses: BTreeMap<String, Response>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RequestBody {
    pub content: BTreeMap<String, MediaType>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Response {
    pub content: Option<BTreeMap<String, MediaType>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MediaType {
    pub schema: Option<Schema>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Components {
    #[serde(default)]
    pub schemas: BTreeMap<String, Schema>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Schema {
    #[serde(default)]
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
    #[serde(default)]
    #[serde(rename = "$ref")]
    pub ref_path: Option<String>,
    #[serde(default)]
    pub any_of: Option<Vec<Schema>>,
    #[serde(default)]
    pub one_of: Option<Vec<Schema>>,
    #[serde(default)]
    pub all_of: Option<Vec<Schema>>,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub example: Option<Value>,
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
    /// Field name (Rust-safe identifier)
    pub name: String,
    /// Field type (Rust type string)
    pub ty: String,
    /// Whether the field is required
    pub required: bool,
    /// Description
    pub description: Option<String>,
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
    pub fn generate_for_provider(&self, provider: &str) -> Result<(), GenResourceError> {
        let spec_path = self.artefacts_dir.join(provider).join("openapi.json");
        // Output to provider's resources directory as resources.rs
        let output_path = self.output_dir.join(provider).join("resources.rs");

        // Create provider directory if it doesn't exist
        std::fs::create_dir_all(&output_path.parent().unwrap()).ok();

        tracing::info!("Generating resource types for {provider}...");

        // Read and parse spec
        let spec_content =
            std::fs::read_to_string(&spec_path).map_err(|e| GenResourceError::ReadFile {
                path: spec_path.display().to_string(),
                source: e,
            })?;

        let spec: Value =
            serde_json::from_str(&spec_content).map_err(|e| GenResourceError::Json {
                path: spec_path.display().to_string(),
                source: e,
            })?;

        // Extract resource definitions
        let resources = self.extract_resources(&spec)?;

        tracing::info!("  Extracted {} resource types", resources.len());

        // Generate Rust code
        let rust_code = self.generate_rust(provider, &resources)?;

        // Write output
        std::fs::write(&output_path, rust_code).map_err(|e| GenResourceError::WriteFile {
            path: output_path.display().to_string(),
            source: e,
        })?;

        // Run rustfmt
        let _ = Command::new("rustfmt").arg(&output_path).output();

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

    /// Extract resource definitions from OpenAPI spec.
    fn extract_resources(&self, spec: &Value) -> Result<Vec<ResourceDef>, GenResourceError> {
        let mut resources = Vec::new();

        // Extract schemas from components/schemas
        if let Some(schemas) = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
        {
            for (schema_name, schema_value) in schemas {
                if let Some(resource) = self.extract_resource(schema_name, schema_value) {
                    resources.push(resource);
                }
            }
        }

        Ok(resources)
    }

    /// Extract a single resource from a schema.
    fn extract_resource(&self, schema_name: &str, schema_value: &Value) -> Option<ResourceDef> {
        let schema: Schema = serde_json::from_value(schema_value.clone()).ok()?;

        // Only process object types with properties
        if schema.schema_type.as_deref() != Some("object") {
            return None;
        }

        let properties = schema.properties.as_ref()?;
        if properties.is_empty() {
            return None;
        }

        let rust_name = self.openapi_to_rust_name(schema_name);
        let description = schema.description.clone();

        let mut fields = Vec::new();
        for (field_name, field_schema) in properties {
            let field_ty = self.schema_to_rust_type(field_schema);
            let required = schema.required.contains(field_name);

            fields.push(FieldDef {
                name: self.openapi_to_rust_name(field_name),
                ty: field_ty,
                required,
                description: field_schema.description.clone(),
            });
        }

        Some(ResourceDef {
            name: rust_name,
            schema_name: schema_name.to_string(),
            description,
            fields,
            is_root: false, // Could be determined by analyzing path references
        })
    }

    /// Convert OpenAPI schema type to Rust type.
    fn schema_to_rust_type(&self, schema: &Schema) -> String {
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
                    let inner_ty = self.schema_to_rust_type(items);
                    format!("Vec<{inner_ty}>")
                } else {
                    "Vec<Value>".to_string()
                }
            }
            Some("object") => "serde_json::Value".to_string(),
            Some(null) if null == "null" => "Option<serde_json::Value>".to_string(),
            None => {
                // Could be a reference or complex type
                if let Some(ref_path) = &schema.ref_path {
                    let ref_name = ref_path.trim_start_matches("#/components/schemas/");
                    self.openapi_to_rust_name(ref_name)
                } else if schema.any_of.is_some()
                    || schema.one_of.is_some()
                    || schema.all_of.is_some()
                {
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

    /// Convert OpenAPI identifier to Rust-safe identifier.
    fn openapi_to_rust_name(&self, name: &str) -> String {
        // Convert kebab-case and camelCase to snake_case
        let result = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();

        // Remove consecutive underscores
        let result = result
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");

        // Handle Rust keywords
        match result.as_str() {
            "type" => "type_".to_string(),
            "ref" => "ref_".to_string(),
            "mod" => "mod_".to_string(),
            "struct" => "struct_".to_string(),
            "enum" => "enum_".to_string(),
            "impl" => "impl_".to_string(),
            "trait" => "trait_".to_string(),
            "fn" => "fn_".to_string(),
            "let" => "let_".to_string(),
            "const" => "const_".to_string(),
            "static" => "static_".to_string(),
            "async" => "async_".to_string(),
            "await" => "await_".to_string(),
            "loop" => "loop_".to_string(),
            "match" => "match_".to_string(),
            "move" => "move_".to_string(),
            "mut" => "mut_".to_string(),
            "pub" => "pub_".to_string(),
            "return" => "return_".to_string(),
            "self" => "self_".to_string(),
            "Self" => "Self_".to_string(),
            "use" => "use_".to_string(),
            "where" => "where_".to_string(),
            "while" => "while_".to_string(),
            "dyn" => "dyn_".to_string(),
            "abstract" => "abstract_".to_string(),
            "become" => "become_".to_string(),
            "box" => "box_".to_string(),
            "do" => "do_".to_string(),
            "final" => "final_".to_string(),
            "macro" => "macro_".to_string(),
            "override" => "override_".to_string(),
            "priv" => "priv_".to_string(),
            "typeof" => "typeof_".to_string(),
            "unsized" => "unsized_".to_string(),
            "virtual" => "virtual_".to_string(),
            "yield" => "yield_".to_string(),
            _ => result,
        }
    }

    /// Generate Rust source code from resource definitions.
    fn generate_rust(
        &self,
        provider: &str,
        resources: &[ResourceDef],
    ) -> Result<String, GenResourceError> {
        let mut out = String::with_capacity(256 * 1024);

        let provider_display = match provider {
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

        writeln!(
            out,
            r#"//! Auto-generated resource types for {provider_display}.
//!
//! This file is generated by `cargo run --bin ewe_platform gen_resource_types`.
//! DO NOT EDIT MANUALLY.
//!
//! Generated from OpenAPI spec in `artefacts/cloud_providers/{provider}/openapi.json`.

#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(dead_code)]
#![allow(unused_imports)]

use serde::{{Deserialize, Serialize}};
use serde_json::Value;

"#
        )
        .map_err(|e| GenResourceError::WriteFile {
            path: format!("generated code for {provider}"),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;

        // Generate structs for each resource
        for resource in resources {
            self.generate_struct(&mut out, resource)?;
        }

        Ok(out)
    }

    /// Generate a single struct definition.
    fn generate_struct(
        &self,
        out: &mut String,
        resource: &ResourceDef,
    ) -> Result<(), GenResourceError> {
        // Write doc comment
        if let Some(desc) = &resource.description {
            writeln!(out, "/// {desc}").map_err(|e| GenResourceError::WriteFile {
                path: format!("struct {}", resource.name),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?;
        } else {
            writeln!(out, "/// {} resource type.", resource.name).map_err(|e| {
                GenResourceError::WriteFile {
                    path: format!("struct {}", resource.name),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                }
            })?;
        }
        writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").map_err(|e| {
            GenResourceError::WriteFile {
                path: format!("struct {}", resource.name),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            }
        })?;
        writeln!(out, "pub struct {} {{", resource.name).map_err(|e| {
            GenResourceError::WriteFile {
                path: format!("struct {}", resource.name),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            }
        })?;

        // Write fields
        for field in &resource.fields {
            if let Some(desc) = &field.description {
                writeln!(out, "    /// {desc}").map_err(|e| GenResourceError::WriteFile {
                    path: format!("field {}.{}", resource.name, field.name),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                })?;
            }
            let field_ty = if field.required {
                field.ty.clone()
            } else {
                format!("Option<{}>", field.ty)
            };
            writeln!(out, "    pub {}: {},", field.name, field_ty).map_err(|e| {
                GenResourceError::WriteFile {
                    path: format!("field {}.{}", resource.name, field.name),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                }
            })?;
        }

        writeln!(out, "}}\n").map_err(|e| GenResourceError::WriteFile {
            path: format!("struct {}", resource.name),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CLI integration
// ---------------------------------------------------------------------------

/// Register the `gen_resource_types` subcommand.
pub fn register(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(
        clap::Command::new("gen_resource_types")
            .about("Generate Rust resource types from OpenAPI specs")
            .arg(
                clap::Arg::new("provider")
                    .long("provider")
                    .short('p')
                    .help("Generate types for only this provider (default: all)")
                    .value_name("PROVIDER"),
            )
            .arg(
                clap::Arg::new("output-dir")
                    .long("output-dir")
                    .help("Output directory for generated files")
                    .value_name("DIR")
                    .default_value("backends/foundation_deployment/src/providers/resources"),
            ),
    )
}

/// Run the `gen_resource_types` command.
pub fn run(matches: &clap::ArgMatches) -> Result<(), BoxedError> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let artefacts_dir = PathBuf::from("artefacts/cloud_providers");
    let output_dir = matches
        .get_one::<String>("output-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/src/providers/resources"));

    // Create output directory
    std::fs::create_dir_all(&output_dir).map_err(|e| GenResourceError::WriteFile {
        path: output_dir.display().to_string(),
        source: e,
    })?;

    let generator = ResourceGenerator::new(artefacts_dir, output_dir.clone());

    if let Some(provider) = matches.get_one::<String>("provider") {
        generator.generate_for_provider(provider)?;
    } else {
        generator.generate_all()?;
    }

    tracing::info!("Resource type generation complete!");
    tracing::info!("Output directory: {}", output_dir.display());

    Ok(())
}
