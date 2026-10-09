use std::collections::BTreeMap;

use foundation_openapi::{EndpointInfo, OperationEffect, OperationType, OperationTypeClassifier};

#[test]
fn test_classify_create_operations() {
    let endpoint = EndpointInfo {
        operation_id: "createInstance".to_string(),
        method: "POST".to_string(),
        path: "/v1/instances".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Create,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Create
    );

    let endpoint = EndpointInfo {
        operation_id: "insertRow".to_string(),
        method: "POST".to_string(),
        path: "/v1/rows".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Create,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Create
    );
}

#[test]
fn test_classify_read_operations() {
    let endpoint = EndpointInfo {
        operation_id: "getInstance".to_string(),
        method: "GET".to_string(),
        path: "/v1/instances/{id}".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Read,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Read
    );

    let endpoint = EndpointInfo {
        operation_id: "listInstances".to_string(),
        method: "GET".to_string(),
        path: "/v1/instances".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Read,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Read
    );
}

#[test]
fn test_classify_update_operations() {
    let endpoint = EndpointInfo {
        operation_id: "updateInstance".to_string(),
        method: "PATCH".to_string(),
        path: "/v1/instances/{id}".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Update,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Update
    );
}

#[test]
fn test_classify_delete_operations() {
    let endpoint = EndpointInfo {
        operation_id: "deleteInstance".to_string(),
        method: "DELETE".to_string(),
        path: "/v1/instances/{id}".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Delete,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Delete
    );
}

#[test]
fn test_classify_mutating_actions() {
    let endpoint = EndpointInfo {
        operation_id: "startInstance".to_string(),
        method: "POST".to_string(),
        path: "/v1/instances/{id}:start".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Action(OperationEffect::Mutating),
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Action(OperationEffect::Mutating)
    );

    let endpoint = EndpointInfo {
        operation_id: "cancelOperation".to_string(),
        method: "POST".to_string(),
        path: "/v1/operations/{id}:cancel".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Action(OperationEffect::Mutating),
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Action(OperationEffect::Mutating)
    );
}

#[test]
fn test_classify_readonly_actions() {
    let endpoint = EndpointInfo {
        operation_id: "testIamPermissions".to_string(),
        method: "POST".to_string(),
        path: "/v1/resources/{id}:testIamPermissions".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Action(OperationEffect::ReadOnly),
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Action(OperationEffect::ReadOnly)
    );
}

#[test]
fn test_classify_by_method_fallback() {
    let endpoint = EndpointInfo {
        operation_id: "v1Compute".to_string(),
        method: "GET".to_string(),
        path: "/v1/compute".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Read,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Read
    );

    let endpoint = EndpointInfo {
        operation_id: "v1Compute".to_string(),
        method: "DELETE".to_string(),
        path: "/v1/compute/{id}".to_string(),
        path_params: vec!["id".to_string()],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        deprecated: false,
        operation_type: OperationType::Delete,
    };
    assert_eq!(
        OperationTypeClassifier::classify(&endpoint),
        OperationType::Delete
    );
}
