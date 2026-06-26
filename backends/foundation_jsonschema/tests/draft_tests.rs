use foundation_jsonschema::Draft;
use serde_json::json;

#[test]
fn test_draft_detect_draft202012() {
    let schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema"});
    assert_eq!(Draft::detect(&schema), Some(Draft::Draft202012));
}

#[test]
fn test_draft_detect_draft201909() {
    let schema = json!({"$schema": "https://json-schema.org/draft/2019-09/schema"});
    assert_eq!(Draft::detect(&schema), Some(Draft::Draft201909));
}

#[test]
fn test_draft_detect_draft7() {
    let schema = json!({"$schema": "http://json-schema.org/draft-07/schema#"});
    assert_eq!(Draft::detect(&schema), Some(Draft::Draft7));
}

#[test]
fn test_draft_detect_draft6() {
    let schema = json!({"$schema": "http://json-schema.org/draft-06/schema#"});
    assert_eq!(Draft::detect(&schema), Some(Draft::Draft6));
}

#[test]
fn test_draft_detect_draft4() {
    let schema = json!({"$schema": "http://json-schema.org/draft-04/schema#"});
    assert_eq!(Draft::detect(&schema), Some(Draft::Draft4));
}

#[test]
fn test_draft_detect_absent() {
    let schema = json!({"type": "string"});
    assert_eq!(Draft::detect(&schema), None);
}

#[test]
fn test_draft_detect_unknown() {
    let schema = json!({"$schema": "http://example.com/unknown"});
    assert_eq!(Draft::detect(&schema), None);
}

#[test]
fn test_draft_from_schema_uri_no_trailing_hash() {
    assert_eq!(
        Draft::from_schema_uri("http://json-schema.org/draft-07/schema"),
        Some(Draft::Draft7)
    );
}

#[test]
fn test_draft_from_schema_uri_with_hash() {
    assert_eq!(
        Draft::from_schema_uri("http://json-schema.org/draft-07/schema#"),
        Some(Draft::Draft7)
    );
}

#[test]
fn test_draft_from_schema_uri_https() {
    assert_eq!(
        Draft::from_schema_uri("https://json-schema.org/draft-07/schema"),
        Some(Draft::Draft7)
    );
}

#[test]
fn test_draft_from_schema_uri_unknown() {
    assert_eq!(Draft::from_schema_uri("http://example.com/unknown"), None);
}

#[test]
fn test_draft_schema_uri() {
    assert_eq!(
        Draft::Draft4.schema_uri(),
        "http://json-schema.org/draft-04/schema#"
    );
    assert_eq!(
        Draft::Draft202012.schema_uri(),
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn test_draft_id_keyword() {
    assert_eq!(Draft::Draft4.id_keyword(), "id");
    assert_eq!(Draft::Draft6.id_keyword(), "$id");
    assert_eq!(Draft::Draft7.id_keyword(), "$id");
    assert_eq!(Draft::Draft201909.id_keyword(), "$id");
    assert_eq!(Draft::Draft202012.id_keyword(), "$id");
}

#[test]
fn test_draft_default() {
    assert_eq!(Draft::DEFAULT, Draft::Draft202012);
}
