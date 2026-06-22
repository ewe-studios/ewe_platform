use foundation_deployment::json_schema::{
    extract_path_params, extract_type_name_from_ref, normalize_type_name,
};

#[test]
fn extracts_type_name_from_openapi_ref() {
    assert_eq!(
        extract_type_name_from_ref("#/components/schemas/ServiceName"),
        Some("ServiceName".to_string())
    );
}

#[test]
fn extracts_type_name_from_gcp_ref() {
    assert_eq!(
        extract_type_name_from_ref("GoogleCloudRunV2Service"),
        Some("GoogleCloudRunV2Service".to_string())
    );
}

#[test]
fn normalizes_dotted_names() {
    assert_eq!(
        normalize_type_name("treasury.transaction"),
        "TreasuryTransaction"
    );
}

#[test]
fn normalizes_hyphenated_names() {
    assert_eq!(normalize_type_name("Custom-pages"), "CustomPages");
}

#[test]
fn normalizes_snake_case_names() {
    assert_eq!(
        normalize_type_name("iam_response_collection"),
        "IamResponseCollection"
    );
}

#[test]
fn extracts_path_params() {
    let params = extract_path_params("/v1/projects/{projectId}");
    assert_eq!(params, vec!["projectId".to_string()]);

    let params = extract_path_params("/v1/folders/{folderId}/files/{fileId}");
    assert_eq!(params, vec!["folderId".to_string(), "fileId".to_string()]);
}
