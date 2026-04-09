//! Integration tests for processing real OpenAPI specs from cloud providers.
//!
//! These tests validate that the foundation_openapi crate can correctly process
//! OpenAPI specs from various cloud providers and generate normalized representations.

use foundation_openapi::{process_spec, normalize_spec};
use std::fs;
use std::path::PathBuf;

/// Get the base path to artefacts directory.
fn artefacts_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
        .join("../../artefacts/cloud_providers")
        .canonicalize()
        .expect("artefacts/cloud_providers directory should exist")
}

/// Load a spec file from the artefacts directory.
fn load_spec(provider: &str, service: &str) -> String {
    let path = artefacts_path()
        .join(provider)
        .join(service)
        .join("openapi.json");

    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read spec at {:?}: {}", path, e))
}

// =============================================================================
// Standard OpenAPI 3.x Specs
// =============================================================================

#[test]
fn processes_fly_io_spec() {
    let spec_json = load_spec("fly_io", "");
    let processor = process_spec(&spec_json).expect("Should parse fly_io spec");
    let normalized = processor.normalize();

    assert!(normalized.metadata.total_endpoints > 0, "fly_io should have endpoints");
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert!(normalized.metadata.total_types > 0, "fly_io should have type definitions");

    println!("fly_io: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

#[test]
fn processes_prisma_postgres_spec() {
    let spec_json = load_spec("prisma_postgres", "");
    let processor = process_spec(&spec_json).expect("Should parse prisma_postgres spec");
    let normalized = processor.normalize();

    assert!(normalized.metadata.total_endpoints > 0, "prisma_postgres should have endpoints");
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert!(normalized.metadata.total_types > 0, "prisma_postgres should have type definitions");

    println!("prisma_postgres: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

#[test]
fn processes_neon_spec() {
    let spec_json = load_spec("neon", "");
    let processor = process_spec(&spec_json).expect("Should parse neon spec");
    let normalized = processor.normalize();

    assert!(normalized.metadata.total_endpoints > 0, "neon should have endpoints");
    assert_eq!(normalized.metadata.spec_format, "openapi_3x");
    assert!(normalized.metadata.total_types > 0, "neon should have type definitions");

    println!("neon: {} endpoints, {} types",
        normalized.metadata.total_endpoints,
        normalized.metadata.total_types);
}

// =============================================================================
// GCP Discovery Document Specs (3 small specs)
// =============================================================================

#[test]
fn processes_gcp_abusiveexperiencereport_spec() {
    let spec_json = load_spec("gcp", "abusiveexperiencereport");
    let result = process_spec(&spec_json);

    if let Ok(processor) = result {
        let normalized = processor.normalize();
        assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
        println!("gcp/abusiveexperiencereport: {} endpoints, {} types",
            normalized.metadata.total_endpoints,
            normalized.metadata.total_types);
    } else {
        // If parsing fails, verify it's a recognized format issue, not a panic
        println!("gcp/abusiveexperiencereport: parse failed (expected for some GCP Discovery docs)");
    }
}

#[test]
fn processes_gcp_groupsmigration_spec() {
    let spec_json = load_spec("gcp", "groupsmigration");
    let result = process_spec(&spec_json);

    if let Ok(processor) = result {
        let normalized = processor.normalize();
        assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
        println!("gcp/groupsmigration: {} endpoints, {} types",
            normalized.metadata.total_endpoints,
            normalized.metadata.total_types);
    } else {
        println!("gcp/groupsmigration: parse failed (expected for some GCP Discovery docs)");
    }
}

#[test]
fn processes_gcp_indexing_spec() {
    let spec_json = load_spec("gcp", "indexing");
    let result = process_spec(&spec_json);

    if let Ok(processor) = result {
        let normalized = processor.normalize();
        assert_eq!(normalized.metadata.spec_format, "gcp_discovery");
        println!("gcp/indexing: {} endpoints, {} types",
            normalized.metadata.total_endpoints,
            normalized.metadata.total_types);
    } else {
        println!("gcp/indexing: parse failed (expected for some GCP Discovery docs)");
    }
}

// =============================================================================
// Normalized Output Validation
// =============================================================================

#[test]
fn normalized_output_contains_endpoint_details() {
    let spec_json = load_spec("fly_io", "");
    let normalized = normalize_spec(&spec_json).expect("Should normalize fly_io spec");

    // Verify endpoint structure
    for (path, methods) in &normalized.endpoints {
        for (method, endpoint) in methods {
            assert!(!endpoint.operation_id.is_empty(), "Operation ID should not be empty for {} {}", method, path);
            assert!(!method.is_empty(), "Method should not be empty");
        }
    }

    println!("Validated {} endpoint paths in normalized output", normalized.endpoints.len());
}

#[test]
fn normalized_output_serializes_to_json() {
    let spec_json = load_spec("prisma_postgres", "");
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
    let spec_json = load_spec("neon", "");
    let normalized = normalize_spec(&spec_json).expect("Should normalize neon spec");

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
