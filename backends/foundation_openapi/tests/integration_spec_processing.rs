//! Integration tests for processing real OpenAPI specs from cloud providers.
//!
//! These tests validate that the foundation_openapi crate can correctly process
//! OpenAPI specs from various cloud providers and generate normalized representations.
//!
//! Test fixtures are frozen snapshots copied from artefacts/cloud_providers/
//! to ensure tests are deterministic and don't change when upstream specs update.

use foundation_openapi::{normalize_spec, process_spec, NormalizedEndpoint};
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
    let normalized = normalize_spec(&spec_json).expect("Should normalize fly_io spec");

    assert_eq!(normalized.metadata.total_endpoints, 73);
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert_eq!(normalized.metadata.total_types, 126);

    // Validate POST /apps endpoint structure
    let post_apps = normalized
        .endpoints
        .get("/apps")
        .and_then(|m| m.get("POST"))
        .expect("POST /apps should exist");

    assert_eq!(post_apps.operation_id, "Apps_create");
    assert!(post_apps.request_type.is_some());
}

#[test]
fn processes_prisma_postgres_spec() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize prisma_postgres spec");

    assert_eq!(normalized.metadata.total_endpoints, 53);
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert_eq!(normalized.metadata.total_types, 132);

    // Validate GET /v1/compute-services endpoint structure
    let get_compute = normalized
        .endpoints
        .get("/v1/compute-services")
        .and_then(|m| m.get("GET"))
        .expect("GET /v1/compute-services should exist");

    assert_eq!(get_compute.operation_id, "getV1Compute-services");
    assert!(get_compute.response_type.is_some());
}

// =============================================================================
// GCP Discovery Document Specs (frozen fixtures)
// =============================================================================

#[test]
fn processes_gcp_abusiveexperiencereport_spec() {
    let spec_json = load_fixture("gcp_abusiveexperiencereport");
    let normalized = normalize_spec(&spec_json).expect("Should normalize GCP spec");

    assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
    assert_eq!(normalized.metadata.total_endpoints, 2);

    // Validate sites.get endpoint
    let sites_get = normalized
        .endpoints
        .get("v1/sites/{sitesId}")
        .and_then(|m| m.get("GET"))
        .expect("GET v1/sites/{{sitesId}} should exist");

    assert_eq!(sites_get.operation_id, "abusiveexperiencereport.sites.get");
    assert_eq!(sites_get.response_type, Some("SiteSummaryResponse".to_string()));
    assert_eq!(sites_get.path_params, vec!["name"]);
}

#[test]
fn processes_gcp_groupsmigration_spec() {
    let spec_json = load_fixture("gcp_groupsmigration");
    let normalized = normalize_spec(&spec_json).expect("Should normalize GCP spec");

    assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
    assert_eq!(normalized.metadata.total_endpoints, 1);

    // Validate at least one endpoint has proper GCP structure
    let mut found_valid = false;
    for (_path, methods) in &normalized.endpoints {
        for (_method, endpoint) in methods {
            if endpoint.operation_id.contains("groupsmigration") {
                found_valid = true;
                break;
            }
        }
    }
    assert!(found_valid, "Should have groupsmigration endpoint");
}

// =============================================================================
// Normalized Output Validation
// =============================================================================

#[test]
fn normalized_output_contains_endpoint_details() {
    let spec_json = load_fixture("fly_io");
    let normalized = normalize_spec(&spec_json).expect("Should normalize fly_io spec");

    // All endpoints must have non-empty operation_id and method
    for (_path, methods) in &normalized.endpoints {
        for (method, endpoint) in methods {
            assert!(!endpoint.operation_id.is_empty());
            assert!(!method.is_empty());
        }
    }
}

#[test]
fn normalized_output_serializes_to_json() {
    let spec_json = load_fixture("prisma_postgres");
    let processor = process_spec(&spec_json).expect("Should parse spec");
    let json_output = processor.to_normalized_json().expect("Should serialize to JSON");

    // Verify it's valid JSON with expected structure
    let parsed: serde_json::Value = serde_json::from_str(&json_output).expect("Output should be valid JSON");

    assert!(parsed["endpoints"].is_object());
    assert!(parsed["types"].is_object());
    assert!(parsed["metadata"]["total_endpoints"].is_number());
    assert_eq!(parsed["metadata"]["total_endpoints"].as_u64(), Some(53));
}

#[test]
fn type_definitions_include_required_fields() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    // Validate all type definitions have required fields
    for (_name, type_def) in &normalized.types {
        assert!(!type_def.name.is_empty());
        let type_json = serde_json::to_string(&type_def).expect("Should serialize type");
        assert!(type_json.contains("\"name\""));
        assert!(type_json.contains("\"kind\""));
    }
}

#[test]
fn endpoint_has_request_and_response_types() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    // Find endpoint with both request and response types and validate them
    let post_compute = normalized
        .endpoints
        .get("/v1/compute-services")
        .and_then(|m| m.get("POST"))
        .expect("POST /v1/compute-services should exist");

    let req_type = post_compute.request_type.as_ref().expect("Should have request_type");
    let resp_type = post_compute.response_type.as_ref().expect("Should have response_type");

    assert!(!req_type.starts_with('#'));
    assert!(!resp_type.starts_with('#'));
}

/// Validates complete endpoint structure extraction with all fields.
#[test]
fn endpoint_extracted_with_full_structure() {
    let spec_json = load_fixture("prisma_postgres");
    let normalized = normalize_spec(&spec_json).expect("Should normalize spec");

    let endpoint = normalized
        .endpoints
        .get("/v1/compute-services")
        .and_then(|m| m.get("POST"))
        .expect("POST /v1/compute-services should exist");

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

    assert_eq!(endpoint.operation_id, expected.operation_id);
    assert_eq!(endpoint.request_type, expected.request_type);
    assert_eq!(endpoint.response_type, expected.response_type);
    assert_eq!(endpoint.error_types, expected.error_types);
    assert_eq!(endpoint.success_codes, expected.success_codes);
    assert_eq!(endpoint.path_params, expected.path_params);
    assert_eq!(endpoint.query_params, expected.query_params);
}

/// Validates GCP Discovery format endpoint extraction.
#[test]
fn gcp_endpoint_extracted_with_full_structure() {
    let spec_json = load_fixture("gcp_abusiveexperiencereport");
    let normalized = normalize_spec(&spec_json).expect("Should normalize GCP spec");

    let endpoint = normalized
        .endpoints
        .get("v1/sites/{sitesId}")
        .and_then(|m| m.get("GET"))
        .expect("GET v1/sites/{{sitesId}} should exist");

    // GCP uses dotted notation: resource.method
    assert!(endpoint.operation_id.contains('.'));
    assert_eq!(endpoint.operation_id, "abusiveexperiencereport.sites.get");

    assert!(endpoint.response_type.is_some());
    assert!(!endpoint.success_codes.is_empty());
    assert_eq!(endpoint.path_params.len(), 1);
}
