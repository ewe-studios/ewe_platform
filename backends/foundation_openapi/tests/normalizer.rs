use std::sync::Arc;

use foundation_openapi::{OpenApiSpec, SpecProcessor};

#[test]
fn normalizes_simple_spec() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test API", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/v1/projects": {
                "get": {
                    "operationId": "listProjects",
                    "responses": {
                        "200": {
                            "description": "Success"
                        }
                    }
                }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let processor = SpecProcessor::new(Arc::new(spec));
    let normalized = processor.normalize();

    assert_eq!(normalized.metadata.total_endpoints, 1);
    assert_eq!(normalized.metadata.api_title, "Test API");
    assert_eq!(normalized.metadata.api_version, "1.0.0");
}

#[test]
fn to_normalized_json_produces_valid_json() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {}
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let processor = SpecProcessor::new(Arc::new(spec));
    let json_output = processor.to_normalized_json();

    assert!(json_output.is_ok());
    let json_str = json_output.unwrap();
    assert!(json_str.contains("\"endpoints\""));
    assert!(json_str.contains("\"metadata\""));
}
