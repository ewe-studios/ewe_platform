use foundation_openapi::{EndpointExtractor, EndpointInfo, OpenApiSpec};
use std::sync::Arc;

#[test]
fn extracts_from_simple_openapi() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/v1/projects": {
                "get": {
                    "operationId": "listProjects",
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProjectsResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "ProjectsResponse": {
                    "type": "object",
                    "properties": {
                        "items": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "listProjects");
    assert_eq!(ep.method, "GET");
    assert_eq!(ep.path, "/v1/projects");
    assert!(ep.response_type.is_some());
}

#[test]
fn extracts_path_params_from_template() {
    let params = EndpointInfo::extract_path_params("/v1/projects/{projectId}");
    assert_eq!(params, vec!["projectId".to_string()]);
}

#[test]
fn extracts_query_params_from_operation() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/apps": {
                "get": {
                    "operationId": "listApps",
                    "parameters": [
                        {
                            "name": "org_slug",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "app_role",
                            "in": "query",
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AppsResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "AppsResponse": {
                    "type": "object"
                }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "listApps");
    assert_eq!(
        ep.query_params,
        vec!["org_slug".to_string(), "app_role".to_string()]
    );
    assert!(ep.path_params.is_empty());
}

#[test]
fn extracts_path_params_from_operation() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/apps/{app_name}/ip_assignments": {
                "get": {
                    "operationId": "listIpAssignments",
                    "parameters": [
                        {
                            "name": "app_name",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/IpAssignmentsResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "IpAssignmentsResponse": {
                    "type": "object"
                }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "listIpAssignments");
    assert_eq!(ep.path_params, vec!["app_name".to_string()]);
    assert!(ep.query_params.is_empty());
}

#[test]
fn extracts_request_body_type() {
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/apps": {
                "post": {
                    "operationId": "createApp",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateAppRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AppResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "CreateAppRequest": { "type": "object" },
                "AppResponse": { "type": "object" }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "createApp");
    assert_eq!(ep.request_type, Some("CreateAppRequest".to_string()));
    assert!(ep.response_type.is_some());
    assert_eq!(
        ep.response_type.as_ref().unwrap().as_rust_type(),
        "AppResponse"
    );
}

#[test]
fn extracts_array_request_body_type() {
    // Test that array request bodies are wrapped in Vec<>
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/items": {
                "put": {
                    "operationId": "bulkUpdateItems",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/BulkUpdateBody" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/UpdateResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "BulkUpdateBody": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" }
                        }
                    }
                },
                "UpdateResponse": { "type": "object" }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "bulkUpdateItems");
    // Array of inline objects should become Vec<serde_json::Value>
    assert_eq!(ep.request_type, Some("Vec<serde_json::Value>".to_string()));
}

#[test]
fn extracts_array_request_body_with_ref_items() {
    // Test that array request bodies with $ref items are wrapped in Vec<ItemType>
    let spec_json = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/secrets": {
                "post": {
                    "operationId": "bulkCreateSecrets",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateSecretsBody" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Success"
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "CreateSecretsBody": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/SecretInput"
                    }
                },
                "SecretInput": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "value": { "type": "string" }
                    }
                }
            }
        }
    });

    let spec: OpenApiSpec = serde_json::from_value(spec_json).unwrap();
    let extractor = EndpointExtractor::new(Arc::new(spec));
    let endpoints = extractor.extract_all();

    assert_eq!(endpoints.len(), 1);
    let ep = &endpoints[0];
    assert_eq!(ep.operation_id, "bulkCreateSecrets");
    // Array of named refs should become Vec<SecretInput>
    assert_eq!(ep.request_type, Some("Vec<SecretInput>".to_string()));
}
