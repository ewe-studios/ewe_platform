use foundation_openapi::{EndpointInfo, OperationType};
use std::collections::BTreeMap;

#[test]
fn extracts_path_params() {
    let params = EndpointInfo::extract_path_params("/v1/projects/{projectId}");
    assert_eq!(params, vec!["projectId".to_string()]);

    let params = EndpointInfo::extract_path_params("/v1/folders/{folderId}/files/{fileId}");
    assert_eq!(params, vec!["folderId".to_string(), "fileId".to_string()]);
}

#[test]
fn converts_to_pascal_case() {
    assert_eq!(
        EndpointInfo::to_pascal_case("getV1Projects"),
        "GetV1Projects"
    );
    assert_eq!(
        EndpointInfo::to_pascal_case("get_v1_projects"),
        "GetV1Projects"
    );
    assert_eq!(
        EndpointInfo::to_pascal_case("treasury.transaction"),
        "TreasuryTransaction"
    );
    assert_eq!(EndpointInfo::to_pascal_case("Custom-pages"), "CustomPages");
}

#[test]
fn converts_to_snake_case() {
    assert_eq!(
        EndpointInfo::to_snake_case("getV1Projects"),
        "get_v1_projects"
    );
    assert_eq!(
        EndpointInfo::to_snake_case("GetV1Projects"),
        "get_v1_projects"
    );
}

#[test]
fn generates_args_struct_name() {
    let endpoint = EndpointInfo {
        operation_id: "getV1Projects".to_string(),
        method: "GET".to_string(),
        path: "/v1/projects".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        operation_type: OperationType::Read,
        deprecated: false,
    };
    assert_eq!(endpoint.args_struct_name(), "GetV1ProjectsArgs");
}

#[test]
fn generates_fn_name() {
    let endpoint = EndpointInfo {
        operation_id: "getV1Projects".to_string(),
        method: "GET".to_string(),
        path: "/v1/projects".to_string(),
        path_params: vec![],
        query_params: vec![],
        request_type: None,
        response_type: None,
        error_types: BTreeMap::new(),
        success_codes: vec![],
        base_url: None,
        summary: None,
        operation_type: OperationType::Read,
        deprecated: false,
    };
    assert_eq!(endpoint.fn_name(), "get_v1_projects");
}
