//! Tests for the `analyzer` path-grouping helper.

use foundation_openapi::unified::analyzer::extract_group_name;

#[test]
fn test_extract_group_name() {
    assert_eq!(
        extract_group_name("/v1/projects/{project}/instances/{instance}"),
        "instances"
    );
    assert_eq!(
        extract_group_name("/v1/run/projects/{project}/locations/{location}/instances"),
        "instances"
    );
    assert_eq!(extract_group_name("/projects"), "projects");
    assert_eq!(extract_group_name("/"), "default");
}
