use foundation_cedar::{CedarEngine, CedarError, CedarRequest};

const TEST_SCHEMA: &str = r"
entity User;
entity Album;
action view appliesTo {
  principal: [User],
  resource: [Album]
};
";

const TEST_POLICY: &str = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
);
"#;

#[test]
fn test_engine_from_str() {
    let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY);
    assert!(engine.is_ok());
}

#[test]
fn test_engine_allows_alice() {
    let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY).unwrap();
    let req = CedarRequest::builder()
        .principal_str("User", "alice")
        .unwrap()
        .action_str("Action", "view")
        .unwrap()
        .resource_str("Album", "trip")
        .unwrap()
        .build()
        .unwrap();

    let response = engine.is_authorized(&req);
    assert!(response.is_allowed());
    assert_ne!(response.reasons(), [] as [cedar_policy::PolicyId; 0]);
}

#[test]
fn test_engine_denies_bob() {
    let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY).unwrap();
    let req = CedarRequest::builder()
        .principal_str("User", "bob")
        .unwrap()
        .action_str("Action", "view")
        .unwrap()
        .resource_str("Album", "trip")
        .unwrap()
        .build()
        .unwrap();

    let response = engine.is_authorized(&req);
    assert!(!response.is_allowed());
}

#[test]
fn test_engine_invalid_schema() {
    let result = CedarEngine::from_str("invalid{{{", TEST_POLICY);
    assert!(result.is_err());
}

#[test]
fn test_engine_invalid_policy() {
    let result = CedarEngine::from_str(TEST_SCHEMA, "invalid{{{");
    assert!(result.is_err());
}

#[test]
fn test_engine_validation_error() {
    let bad_policy = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
) when { principal.nonexistent_attr > 5 };
"#;
    let result = CedarEngine::from_str(TEST_SCHEMA, bad_policy);
    assert!(result.is_err());
    if let Err(CedarError::Validation(errors)) = result {
        assert_ne!(errors, [] as [std::string::String; 0]);
    }
}
