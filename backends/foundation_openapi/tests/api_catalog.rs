use foundation_openapi::{
    escape_rust_keyword, operation_id_to_fn_name, path_to_fn_suffix, sanitize_doc_comment,
    sanitize_field_name, sanitize_identifier, to_pascal_case, to_pascal_case_from_any,
    to_snake_case,
};

#[test]
fn test_to_pascal_case() {
    assert_eq!(
        to_pascal_case("getV1ComputeServices"),
        "GetV1ComputeServices"
    );
    assert_eq!(
        to_pascal_case("reports_activities_list"),
        "ReportsActivitiesList"
    );
    assert_eq!(to_pascal_case("admin.channels.stop"), "AdminChannelsStop");
}

#[test]
fn test_to_snake_case() {
    assert_eq!(
        to_snake_case("getV1ComputeServices"),
        "get_v1_compute_services"
    );
    assert_eq!(
        to_snake_case("ReportsActivitiesList"),
        "reports_activities_list"
    );
    assert_eq!(to_snake_case("CloudSQL"), "cloud_sql");
    assert_eq!(
        to_snake_case("AlloydbProjectsLocationsClustersRestoreFromCloudSQL"),
        "alloydb_projects_locations_clusters_restore_from_cloud_sql"
    );
    assert_eq!(to_snake_case("SBOM"), "sbom");
    assert_eq!(to_snake_case("ExportSBOM"), "export_sbom");
    assert_eq!(to_snake_case("OAuth"), "o_auth");
    assert_eq!(to_snake_case("FinishOAuthFlow"), "finish_o_auth_flow");
    assert_eq!(to_snake_case("StartOAuthFlow"), "start_o_auth_flow");
    assert_eq!(to_snake_case("Oauth"), "oauth");
    assert_eq!(to_snake_case("FinishOauthFlow"), "finish_oauth_flow");
    assert_eq!(
        to_snake_case("agents.a2a.v1.getCard"),
        "agents_a2a_v1_get_card"
    );
    assert_eq!(
        to_snake_case("agents_a2a_v1_getCard"),
        "agents_a2a_v1_get_card"
    );
    assert_eq!(
        to_snake_case("containeranalysis_projects_resources_exportSBOM"),
        "containeranalysis_projects_resources_export_sbom"
    );
}

#[test]
fn test_sanitize_identifier() {
    assert_eq!(
        sanitize_identifier("admin.channels.stop"),
        "admin_channels_stop"
    );
    assert_eq!(sanitize_identifier("foo-bar@baz"), "foo_bar_baz");
    assert_eq!(sanitize_identifier("test:id<ok>"), "test_id_ok_");
    assert_eq!(sanitize_identifier("a[b]c(d)e"), "a_b_c_d_e");
    assert_eq!(sanitize_identifier("QUERY PLAN"), "QUERY_PLAN");
    assert_eq!(
        to_snake_case(&sanitize_identifier("QUERY PLAN")),
        "query_plan"
    );
}

#[test]
fn test_path_to_fn_suffix() {
    assert_eq!(
        path_to_fn_suffix("/projects/{project}/services"),
        "projects_project_services"
    );
    assert_eq!(path_to_fn_suffix("/tools/{id}:execute"), "tools_id_execute");
    assert_eq!(path_to_fn_suffix("/"), "");
}

#[test]
fn test_operation_id_to_fn_name() {
    assert_eq!(
        operation_id_to_fn_name(Some("CloudKmsProjectsGet"), "GET", "/projects"),
        "cloud_kms_projects_get"
    );
    assert_eq!(
        operation_id_to_fn_name(None, "GET", "/projects/{id}"),
        "get_projects_id"
    );
}

#[test]
fn test_escape_rust_keyword() {
    assert_eq!(escape_rust_keyword("type"), "type_rs");
    assert_eq!(escape_rust_keyword("async"), "async_rs");
    assert_eq!(escape_rust_keyword("normal"), "normal");
}

#[test]
fn test_sanitize_field_name() {
    assert_eq!(sanitize_field_name("camelCase"), "camel_case");
    assert_eq!(sanitize_field_name("123"), "field_123");
    assert_eq!(sanitize_field_name("async"), "async_");
    assert_eq!(sanitize_field_name("QUERY PLAN"), "query_plan");
}

#[test]
fn test_to_pascal_case_from_any() {
    assert_eq!(
        to_pascal_case_from_any("getV1ComputeServices"),
        "GetV1ComputeServices"
    );
    assert_eq!(
        to_pascal_case_from_any("admin.channels.stop"),
        "AdminChannelsStop"
    );
    assert_eq!(to_pascal_case_from_any(""), "Unknown");
}

#[test]
fn test_sanitize_doc_comment() {
    let doc = sanitize_doc_comment("See https://cloud.google.com for details", true);
    assert!(doc.contains("<https://cloud.google.com>"));

    let doc = sanitize_doc_comment("Returns a String or i32", true);
    assert!(doc.contains("`String`"));
    assert!(doc.contains("`i32`"));

    let doc = sanitize_doc_comment("<code>example</code> value", true);
    assert!(doc.contains("example"));
    assert!(!doc.contains("<code>"));

    let doc = sanitize_doc_comment("<b>bold</b> text", true);
    assert_eq!(doc, "bold text");

    let doc = sanitize_doc_comment("timestamp('2020-01-01')", true);
    assert!(doc.contains("''2020-01-01''"));

    assert_eq!(sanitize_doc_comment("", true), "");

    let doc = sanitize_doc_comment("First line\nSecond line", false);
    assert_eq!(doc, "First line");

    let doc = sanitize_doc_comment("First line\nSecond line", true);
    assert_eq!(doc, "First line Second line");

    let doc = sanitize_doc_comment("a < b", true);
    assert!(doc.contains("&lt;"));

    let doc = sanitize_doc_comment("Use returnPartialSuccess for partial results", true);
    assert!(doc.contains("`returnPartialSuccess`"));
}
