use foundation_cedar::CedarRequest;

#[test]
fn test_build_request() {
    let req = CedarRequest::builder()
        .principal_str("User", "alice")
        .unwrap()
        .action_str("Action", "view")
        .unwrap()
        .resource_str("Album", "trip")
        .unwrap()
        .build();
    assert!(req.is_ok());
}

#[test]
fn test_build_request_missing_principal() {
    let req = CedarRequest::builder()
        .action_str("Action", "view")
        .unwrap()
        .resource_str("Album", "trip")
        .unwrap()
        .build();
    assert!(req.is_err());
}
