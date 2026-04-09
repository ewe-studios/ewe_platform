//! Integration tests for processing real OpenAPI specs from cloud providers.
//!
//! These tests validate that the foundation_openapi crate can correctly process
//! OpenAPI specs from various cloud providers and generate normalized representations.
//!
//! Test fixtures are frozen snapshots copied from artefacts/cloud_providers/
//! to ensure tests are deterministic and don't change when upstream specs update.

use foundation_openapi::{process_spec, normalize_spec, NormalizedEndpoint};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// Get the base path to the fixtures directory.
fn fixtures_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
        .join("tests/fixtures")
        .canonicalize()
        .expect("tests/fixtures directory should exist")
}

/// Load a frozen test fixture.
fn load_fixture(name: &str) -> String {
    let path = fixtures_path().join(format!("{name}.json"));
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture at {:?}: {}", path, e))
}

// =============================================================================
// Standard OpenAPI 3.x Specs (frozen fixtures)
// =============================================================================

#[test]
fn processes_fly_io_spec() {
    let spec_json = load_fixture("fly_io");
    let processor = process_spec(&spec_json).expect("Should parse fly_io spec");
    let normalized = processor.normalize();

    // Exact endpoint count from frozen snapshot (verified: 2026-04-09)
    assert_eq!(normalized.metadata.total_endpoints, 73,
        "fly_io should have exactly 73 endpoints");
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert!(normalized.metadata.total_types > 0, "fly_io should have type definitions");

    // Validate specific endpoint structure
    let apps_endpoint = normalized.endpoints.get("/apps");
    assert!(apps_endpoint.is_some(), "/apps endpoint should exist");

    let apps_methods = apps_endpoint.unwrap();
    let post_apps = apps_methods.get("POST");
    assert!(post_apps.is_some(), "POST /apps should exist");

    let post_apps_info = post_apps.unwrap();
    assert_eq!(post_apps_info.operation_id, "Apps_create",
        "POST /apps operation_id should be 'Apps_create'");

    println!("fly_io: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

#[test]
fn processes_prisma_postgres_spec() {
    let spec_json = load_fixture("prisma_postgres");
    let processor = process_spec(&spec_json).expect("Should parse prisma_postgres spec");
    let normalized = processor.normalize();

    // Exact endpoint count from frozen snapshot (verified: 2026-04-09)
    assert_eq!(normalized.metadata.total_endpoints, 53,
        "prisma_postgres should have exactly 53 endpoints");
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert!(normalized.metadata.total_types > 0, "prisma_postgres should have type definitions");

    // Validate specific endpoint structure
    let compute_services_endpoint = normalized.endpoints.get("/v1/compute-services");
    assert!(compute_services_endpoint.is_some(), "/v1/compute-services endpoint should exist");

    let get_compute_services = compute_services_endpoint.unwrap().get("GET");
    assert!(get_compute_services.is_some(), "GET /v1/compute-services should exist");

    let get_compute_services_info = get_compute_services.unwrap();
    assert_eq!(get_compute_services_info.operation_id, "getV1Compute-services",
        "GET /v1/compute-services operation_id should be 'getV1Compute-services'");
    assert!(get_compute_services_info.response_type.is_some(),
        "GET /v1/compute-services should have a response type");

    println!("prisma_postgres: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

// =============================================================================
// GCP Discovery Document Specs (frozen fixtures)
// =============================================================================

#[test]
fn processes_gcp_abusiveexperiencereport_spec() {
    let spec_json = load_fixture("gcp_abusiveexperiencereport");
    let processor = process_spec(&spec_json).expect("Should parse GCP abusiveexperiencereport spec");
    let normalized = processor.normalize();

    assert_eq!(normalized.metadata.spec_format, "gcp_discovery");

    // Validate endpoint structure exists
    assert!(normalized.metadata.total_endpoints > 0,
        "GCP abusiveexperiencereport should have endpoints");

    // Validate at least one endpoint has proper structure
    let mut found_valid_endpoint = false;
    for (_path, methods) in &normalized.endpoints {
        for (_method, endpoint) in methods {
            if !endpoint.operation_id.is_empty() {
                found_valid_endpoint = true;
                break;
            }
        }
    }
    assert!(found_valid_endpoint, "Should have at least one endpoint with operation_id");

    println!("gcp/abusiveexperiencereport: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

#[test]
fn processes_gcp_groupsmigration_spec() {
    let spec_json = load_fixture("gcp_groupsmigration");
    let processor = process_spec(&spec_json).expect("Should parse GCP groupsmigration spec");
    let normalized = processor.normalize();

    assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
    assert!(normalized.metadata.total_endpoints > 0,
        "GCP groupsmigration should have endpoints");

    println!("gcp/groupsmigration: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

// =============================================================================
// Normalized Output Validation
// =============================================================================

#[test]
fn normalized_output_contains_endpoint_details() {
    let spec_json = load_fixture("fly_io");
    let normalized = normalize_spec(&spec_json).expect("Should normalize fly_io spec");

    // Verify endpoint structure for all endpoints
    for (path, methods) in &normalized.endpoints {
        for (method, endpoint) in methods {
            assert!(!endpoint.operation_id.is_empty(),
                "Operation ID should not be empty for {} {}", method, path);
            assert!(!method.is_empty(), "Method should not be empty");
        }
    }

    println!("Validated {} endpoint paths in normalized output", normalized.endpoints.len());
}

#[test]
fn normalized_output_serializes_to_json() {
    let spec_json = load_fixture("prisma_postgres");
    let processor = process_spec(&spec_json).expect("Should parse spec");
    let json_output = processor.to_normalized_json();

    assert!(json_output.is_ok(), "Should serialize to JSON");
    let json_str = json_output.unwrap();

    // Verify JSON structure
    assert!(json_str.contains("\"endpoints\""), "JSON should contain endpoints key");
    assert!(json_str.contains("\"types\""), "JSON should contain types key");
    assert!(json_str.contains("\"metadata\""), "JSON should contain metadata key");
    assert!(json_str.contains("\"total_endpoints\""), "JSON should contain total_endpoints");
    assert!(json_str.contains("\"total_types\""), "JSON should contain total_types");

    // Verify it's valid JSON by parsing it back
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .expect("Output should be valid JSON");

    assert!(parsed["endpoints"].is_object());
    assert!(parsed["metadata"]["total_endpoints"].is_number());
}

#[test]
fn type_definitions_include_required_fields() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    for (name, type_def) in &normalized.types {
        assert!(!type_def.name.is_empty(), "Type name should not be empty");
        // Serialize the whole type definition to verify it's valid
        let type_json = serde_json::to_string(&type_def)
            .unwrap_or_else(|e| panic!("Failed to serialize type definition for {}: {}", name, e));
        assert!(type_json.contains("\"name\""), "Type definition should have name field");
        assert!(type_json.contains("\"kind\""), "Type definition should have kind field");
    }

    println!("Validated {} type definitions", normalized.types.len());
}

#[test]
fn endpoint_has_request_and_response_types() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    // Find an endpoint with both request and response types
    let mut found_complete_endpoint = false;
    for (_path, methods) in &normalized.endpoints {
        for (_method, endpoint) in methods {
            if endpoint.request_type.is_some() && endpoint.response_type.is_some() {
                found_complete_endpoint = true;

                // Validate the types are properly named (PascalCase, no $ref remnants)
                if let Some(ref req_type) = endpoint.request_type {
                    assert!(!req_type.starts_with("#"),
                        "Request type should not contain $ref: {}", req_type);
                }
                if let Some(ref resp_type) = endpoint.response_type {
                    assert!(!resp_type.starts_with("#"),
                        "Response type should not contain $ref: {}", resp_type);
                }
            }
        }
    }

    assert!(found_complete_endpoint,
        "Should have at least one endpoint with both request_type and response_type");
}

/// Validates complete endpoint structure extraction with all fields.
#[test]
fn endpoint_extracted_with_full_structure() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    // Find POST /v1/compute-services endpoint
    let compute_services_post = normalized.endpoints
        .get("/v1/compute-services")
        .and_then(|methods| methods.get("POST"));

    assert!(compute_services_post.is_some(),
        "POST /v1/compute-services should exist");

    let endpoint = compute_services_post.unwrap();

    // Build expected endpoint structure
    let mut expected_error_types = BTreeMap::new();
    expected_error_types.insert("401".to_string(), "PrismaApiError".to_string());
    expected_error_types.insert("403".to_string(), "PrismaApiError".to_string());
    expected_error_types.insert("404".to_string(), "PrismaApiError".to_string());
    expected_error_types.insert("422".to_string(), "PrismaApiError".to_string());
    expected_error_types.insert("429".to_string(), "PrismaApiError".to_string());

    let expected = NormalizedEndpoint {
        operation_id: "postV1Compute-services".to_string(),
        request_type: Some("ComputeservicesPostRequest".to_string()),
        response_type: Some("ComputeservicesPostResponse".to_string()),
        error_types: expected_error_types,
        success_codes: vec!["201".to_string()],
        path_params: vec![],
        query_params: vec![],
    };

    assert_eq!(endpoint.operation_id, expected.operation_id,
        "operation_id should match");
    assert_eq!(endpoint.request_type, expected.request_type,
        "request_type should match");
    assert_eq!(endpoint.response_type, expected.response_type,
        "response_type should match");
    assert_eq!(endpoint.error_types, expected.error_types,
        "error_types should match");
    assert_eq!(endpoint.success_codes, expected.success_codes,
        "success_codes should match");
    assert_eq!(endpoint.path_params, expected.path_params,
        "path_params should match");
    assert_eq!(endpoint.query_params, expected.query_params,
        "query_params should match");
}

/// Validates GCP Discovery format endpoint extraction.
#[test]
fn gcp_endpoint_extracted_with_full_structure() {
    let spec_json = load_fixture("gcp_abusiveexperiencereport");
    let normalized = normalize_spec(&spec_json).expect("Should normalize GCP spec");

    // GCP Discovery format should have endpoints from resources
    assert!(!normalized.endpoints.is_empty(),
        "GCP spec should have endpoints");

    // Find and validate first endpoint with complete structure
    let mut found_valid_endpoint = false;
    for (path, methods) in &normalized.endpoints {
        for (method, endpoint) in methods {
            if !endpoint.operation_id.is_empty() {
                found_valid_endpoint = true;

                // Validate GCP-specific structure (dotted notation: resource.method)
                assert!(endpoint.operation_id.contains('.'),
                    "GCP operation_id should use dotted notation (resource.method): {}", endpoint.operation_id);

                // Validate endpoint has response type
                assert!(endpoint.response_type.is_some(),
                    "GCP endpoint should have response_type: {} {}", method, path);

                // Validate success codes were extracted
                assert!(!endpoint.success_codes.is_empty(),
                    "GCP endpoint should have success_codes: {} {}", method, path);

                // Validate path params match the path template
                let path_param_count = path.matches('{').count();
                assert_eq!(endpoint.path_params.len(), path_param_count,
                    "path_params count should match {{}} placeholders in path: {} {}", method, path);

                break;
            }
        }
        if found_valid_endpoint {
            break;
        }
    }

    assert!(found_valid_endpoint,
        "Should have at least one endpoint with operation_id");
}
