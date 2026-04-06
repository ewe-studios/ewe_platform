//! Prisma Postgres `` `OpenAPI` `` spec fetcher and normalizer.
//!
//! WHY: Prisma Postgres publishes an OpenAPI 3.1.0 spec at `/v1/doc` with two
//! quirks the generators cannot consume directly:
//! 1. No `servers` field — the base URL must be inferred from the fetch URL.
//! 2. All request/response schemas are defined inline in path operations
//!    rather than in `components/schemas` with `$ref` pointers.
//!
//! WHAT: Fetches the raw spec, then normalizes it into canonical OpenAPI 3.x
//! form so the type and client generators produce correct Rust code.
//!
//! HOW: After download, `normalize_prisma_spec` walks every path operation,
//! lifts inline schemas into `components/schemas`, replaces them with `$ref`,
//! and injects the `servers` array.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard::normalize;
use foundation_core::valtron::{from_future, StreamIterator, StreamIteratorExt};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Prisma Postgres API base URL.
pub const BASE_URL: &str = "https://api.prisma.io";

/// Prisma Postgres API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.prisma.io/v1/doc";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "prisma_postgres";

/// Fetch the Prisma Postgres `` `OpenAPI` `` spec and normalize it.
///
/// After fetching the raw spec, applies `normalize_prisma_spec` to extract
/// inline schemas and add the missing `servers` field before writing to disk.
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
pub fn fetch_prisma_postgres_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let output_dir = output_dir.clone();

    let future = async move {
        info!("Fetching Prisma Postgres OpenAPI spec from {}", SPEC_URL);

        // Ensure output directory exists
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to create output directory: {e}"
            )))
        })?;

        // Fetch using curl
        let output_path = output_dir.join("openapi.json");
        let output = Command::new("curl")
            .args(["-s", "-o"])
            .arg(&output_path)
            .arg(SPEC_URL)
            .output()
            .map_err(|e| {
                DeploymentError::ProcessFailed {
                    command: format!("curl -o {} {}", output_path.display(), SPEC_URL),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("curl execution failed: {e}"),
                }
            })?;

        if !output.status.success() {
            return Err(DeploymentError::ProcessFailed {
                command: format!("curl -o {} {}", output_path.display(), SPEC_URL),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Read, normalize, and write back
        let content = std::fs::read_to_string(&output_path).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to read fetched spec: {e}"
            )))
        })?;

        let mut spec: Value = serde_json::from_str(&content).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: "prisma_postgres openapi.json".to_string(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        normalize_prisma_spec(&mut spec);

        let normalized = serde_json::to_string_pretty(&spec).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: "prisma_postgres openapi.json".to_string(),
                reason: format!("Serialization failed: {e}"),
            }
        })?;

        std::fs::write(&output_path, normalized).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to write normalized spec: {e}"
            )))
        })?;

        info!("Successfully fetched and normalized Prisma Postgres spec to {:?}", output_path);
        Ok(output_path)
    };

    let task = from_future(future);
    let stream = foundation_core::valtron::execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}

/// Normalize a Prisma Postgres OpenAPI 3.1.0 spec into canonical form.
///
/// 1. Adds `servers: [{"url": "https://api.prisma.io"}]` if missing.
/// 2. Walks every path operation and extracts inline request/response schemas
///    into `components/schemas` with `$ref` pointers.
/// 3. Normalizes OpenAPI 3.1.0 nullable types (`["string", "null"]` → `type: "string"`).
pub fn normalize_prisma_spec(spec: &mut Value) {
    normalize::ensure_servers(spec, BASE_URL);

    let mut extracted_schemas = Map::new();

    // Extract a shared error schema once (all error responses share the same shape)
    extract_error_schema(spec, &mut extracted_schemas);

    // Walk all paths and extract inline schemas
    if let Some(paths) = spec.get("paths").cloned() {
        if let Some(paths_obj) = paths.as_object() {
            for (path, path_item) in paths_obj {
                let base_name = normalize::path_to_type_name(path);
                if let Some(item_obj) = path_item.as_object() {
                    for (method, operation) in item_obj {
                        if !["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                            continue;
                        }
                        let method_prefix = capitalize_first(method);
                        extract_operation_schemas(
                            spec,
                            path,
                            method,
                            operation,
                            &base_name,
                            &method_prefix,
                            &mut extracted_schemas,
                        );
                    }
                }
            }
        }
    }

    // Merge extracted schemas into components/schemas
    if !extracted_schemas.is_empty() {
        let components = spec
            .as_object_mut()
            .unwrap()
            .entry("components")
            .or_insert_with(|| Value::Object(Map::new()));
        let schemas = components
            .as_object_mut()
            .unwrap()
            .entry("schemas")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(schemas_obj) = schemas.as_object_mut() {
            for (name, schema) in extracted_schemas {
                schemas_obj.insert(name, schema);
            }
        }
    }
}

/// Extract the shared error response schema (`PrismaApiError`).
///
/// Prisma Postgres error responses all share the same structure:
/// ```json
/// { "error": { "code": "string", "hint": "string", "message": "string" } }
/// ```
fn extract_error_schema(spec: &Value, schemas: &mut Map<String, Value>) {
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        for (_path, path_item) in paths {
            if let Some(item_obj) = path_item.as_object() {
                for (_method, operation) in item_obj {
                    for (code, resp) in operation
                        .get("responses")
                        .and_then(|r| r.as_object())
                        .into_iter()
                        .flatten()
                    {
                        if code.starts_with('2') {
                            continue;
                        }
                        let schema = resp
                            .pointer("/content/application~1json/schema");
                        if let Some(schema) = schema {
                            if schema.get("properties").and_then(|p| p.get("error")).is_some() {
                                let mut error_schema = schema.clone();
                                normalize::normalize_nullable_types(&mut error_schema);
                                error_schema["type"] = Value::String("object".to_string());
                                // Extract the inner error object
                                if let Some(error_inner) = error_schema
                                    .get("properties")
                                    .and_then(|p| p.get("error"))
                                    .cloned()
                                {
                                    let mut inner = error_inner;
                                    inner["type"] = Value::String("object".to_string());
                                    schemas.insert("PrismaApiErrorDetail".to_string(), inner);
                                    if let Some(props) = error_schema
                                        .get_mut("properties")
                                        .and_then(|p| p.as_object_mut())
                                    {
                                        props.insert(
                                            "error".to_string(),
                                            serde_json::json!({"$ref": "#/components/schemas/PrismaApiErrorDetail"}),
                                        );
                                    }
                                }
                                schemas.insert("PrismaApiError".to_string(), error_schema);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Extract inline schemas from a single path operation.
///
/// Handles:
/// - Request body inline schemas → `{BaseName}{Method}Request`
/// - 2xx response inline schemas → `{BaseName}` for the data resource,
///   `{BaseName}{Method}Response` for the envelope
fn extract_operation_schemas(
    spec: &mut Value,
    path: &str,
    method: &str,
    operation: &Value,
    base_name: &str,
    method_prefix: &str,
    schemas: &mut Map<String, Value>,
) {
    // Request body
    let rb_schema = operation
        .pointer("/requestBody/content/application~1json/schema")
        .cloned();
    if let Some(mut rb) = rb_schema {
        if rb.get("properties").is_some() {
            let req_name = format!("{base_name}{method_prefix}Request");
            normalize::normalize_nullable_types(&mut rb);
            rb["type"] = Value::String("object".to_string());
            if let Some(props) = rb.get_mut("properties").and_then(|p| p.as_object_mut()) {
                normalize::extract_inline_schemas(props, &req_name, schemas);
            }
            schemas.insert(req_name.clone(), rb);
            // Replace inline with $ref
            if let Some(op) = spec.pointer_mut(&format!(
                "/paths/{}/{}",
                json_pointer_escape(path),
                method
            )) {
                op["requestBody"]["content"]["application/json"]["schema"] =
                    serde_json::json!({"$ref": format!("#/components/schemas/{req_name}")});
            }
        }
    }

    // 2xx responses
    let response_codes: Vec<String> = operation
        .get("responses")
        .and_then(|r| r.as_object())
        .map(|obj| {
            obj.keys()
                .filter(|k| k.starts_with('2'))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    for code in response_codes {
        let resp_schema = operation
            .pointer(&format!(
                "/responses/{code}/content/application~1json/schema"
            ))
            .cloned();
        let Some(schema) = resp_schema else {
            continue;
        };
        if schema.get("properties").is_none() {
            continue;
        }

        let data_prop = schema
            .get("properties")
            .and_then(|p| p.get("data"))
            .cloned();

        let resp_name = format!("{base_name}{method_prefix}Response");

        match data_prop {
            // data is an array of objects → extract the item type
            Some(ref data) if data.get("type").and_then(|t| t.as_str()) == Some("array")
                && data.get("items").and_then(|i| i.get("properties")).is_some() =>
            {
                let mut item_schema = data["items"].clone();
                normalize::normalize_nullable_types(&mut item_schema);
                item_schema["type"] = Value::String("object".to_string());
                if let Some(props) = item_schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    normalize::extract_inline_schemas(props, base_name, schemas);
                }
                schemas.insert(base_name.to_string(), item_schema);

                // Build response envelope
                let mut envelope = schema.clone();
                normalize::normalize_nullable_types(&mut envelope);
                envelope["type"] = Value::String("object".to_string());
                envelope["properties"]["data"] = serde_json::json!({
                    "type": "array",
                    "items": {"$ref": format!("#/components/schemas/{base_name}")}
                });
                schemas.insert(resp_name.clone(), envelope);
            }
            // data is an inline object → extract it
            Some(ref data) if data.get("properties").is_some() => {
                let mut data_schema = data.clone();
                normalize::normalize_nullable_types(&mut data_schema);
                data_schema["type"] = Value::String("object".to_string());
                if let Some(props) = data_schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    normalize::extract_inline_schemas(props, base_name, schemas);
                }
                schemas.insert(base_name.to_string(), data_schema);

                // Build response envelope
                let mut envelope = schema.clone();
                normalize::normalize_nullable_types(&mut envelope);
                envelope["type"] = Value::String("object".to_string());
                envelope["properties"]["data"] =
                    serde_json::json!({"$ref": format!("#/components/schemas/{base_name}")});
                schemas.insert(resp_name.clone(), envelope);
            }
            // No data field or data is a primitive → extract whole response
            _ => {
                let mut resp_schema = schema.clone();
                normalize::normalize_nullable_types(&mut resp_schema);
                resp_schema["type"] = Value::String("object".to_string());
                if let Some(props) = resp_schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    normalize::extract_inline_schemas(props, &resp_name, schemas);
                }
                schemas.insert(resp_name.clone(), resp_schema);
            }
        }

        // Replace inline schema with $ref in the spec
        if let Some(op) = spec.pointer_mut(&format!(
            "/paths/{}/{}",
            json_pointer_escape(path),
            method
        )) {
            op["responses"][&code]["content"]["application/json"]["schema"] =
                serde_json::json!({"$ref": format!("#/components/schemas/{resp_name}")});
        }

        // Replace error responses with $ref to shared error schema
        if schemas.contains_key("PrismaApiError") {
            if let Some(op) = spec.pointer_mut(&format!(
                "/paths/{}/{}",
                json_pointer_escape(path),
                method
            )) {
                if let Some(responses) = op.get_mut("responses").and_then(|r| r.as_object_mut()) {
                    for (err_code, err_resp) in responses.iter_mut() {
                        if err_code.starts_with('2') {
                            continue;
                        }
                        if let Some(content) = err_resp
                            .get_mut("content")
                            .and_then(|c| c.get_mut("application/json"))
                        {
                            content["schema"] =
                                serde_json::json!({"$ref": "#/components/schemas/PrismaApiError"});
                        }
                    }
                }
            }
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => {
            let mut result = c.to_uppercase().to_string();
            result.extend(chars);
            result
        }
        None => String::new(),
    }
}

fn json_pointer_escape(path: &str) -> String {
    path.replace('~', "~0").replace('/', "~1")
}

/// Process a fetched Prisma Postgres spec.
///
/// # Returns
///
/// Returns a `ProcessedSpec` with extracted endpoints and metadata.
#[must_use]
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert_eq!(PROVIDER_NAME, "prisma_postgres");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("prisma.io"));
    }

    #[test]
    fn normalize_adds_servers() {
        let mut spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Test", "version": "v1"},
            "components": {"schemas": {}},
            "paths": {}
        });
        normalize_prisma_spec(&mut spec);
        assert_eq!(spec["servers"][0]["url"], BASE_URL);
    }

    #[test]
    fn normalize_extracts_inline_response_schemas() {
        let mut spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Test", "version": "v1"},
            "components": {"schemas": {}},
            "paths": {
                "/v1/databases": {
                    "get": {
                        "operationId": "getV1Databases",
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "data": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "id": {"type": "string"},
                                                            "name": {"type": "string"},
                                                            "status": {"type": "string"}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        normalize_prisma_spec(&mut spec);

        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("Databases"), "Should extract Databases resource type");
        assert!(schemas.contains_key("DatabasesGetResponse"), "Should extract response envelope");

        let db_schema = &schemas["Databases"];
        assert_eq!(db_schema["properties"]["id"]["type"], "string");
        assert_eq!(db_schema["properties"]["name"]["type"], "string");

        // The response should now use $ref
        let resp_schema = &spec["paths"]["/v1/databases"]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
        assert!(resp_schema.get("$ref").is_some(), "Should replace inline with $ref");
    }

    #[test]
    fn normalize_extracts_request_body_schemas() {
        let mut spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Test", "version": "v1"},
            "components": {"schemas": {}},
            "paths": {
                "/v1/projects": {
                    "post": {
                        "operationId": "postV1Projects",
                        "requestBody": {
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["name"],
                                        "properties": {
                                            "name": {"type": "string"},
                                            "region": {"type": "string"}
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {}
                    }
                }
            }
        });

        normalize_prisma_spec(&mut spec);

        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("ProjectsPostRequest"));

        let req_schema = &schemas["ProjectsPostRequest"];
        assert_eq!(req_schema["properties"]["name"]["type"], "string");
    }

    #[test]
    fn normalize_handles_nullable_types() {
        let mut spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Test", "version": "v1"},
            "components": {"schemas": {}},
            "paths": {
                "/v1/items": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "data": {
                                                    "type": "object",
                                                    "properties": {
                                                        "id": {"type": "string"},
                                                        "label": {"type": ["string", "null"]}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        normalize_prisma_spec(&mut spec);

        let schemas = spec["components"]["schemas"].as_object().unwrap();
        let items = &schemas["Items"];
        assert_eq!(items["properties"]["label"]["type"], "string");
        assert_eq!(items["properties"]["label"]["nullable"], true);
    }

    #[test]
    fn normalize_extracts_nested_objects() {
        let mut spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Test", "version": "v1"},
            "components": {"schemas": {}},
            "paths": {
                "/v1/databases": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "data": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "id": {"type": "string"},
                                                            "project": {
                                                                "type": "object",
                                                                "properties": {
                                                                    "id": {"type": "string"},
                                                                    "name": {"type": "string"}
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        normalize_prisma_spec(&mut spec);

        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("DatabasesProject"), "Should extract nested project object");
        assert_eq!(schemas["DatabasesProject"]["properties"]["id"]["type"], "string");

        // The parent should have a $ref for project
        let db = &schemas["Databases"];
        assert!(db["properties"]["project"].get("$ref").is_some());
    }

    #[test]
    fn process_spec_extracts_version() {
        let spec = serde_json::json!({
            "info": { "version": "1.0.0", "title": "Prisma Postgres API" },
            "paths": {
                "/databases": {
                    "get": { "operationId": "listDatabases", "summary": "List databases" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("1.0.0".to_string()));
        assert!(processed.endpoints.is_some());
        assert!(!processed.content_hash.is_empty());
    }
}
