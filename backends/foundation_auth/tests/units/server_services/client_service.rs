//! ClientService tests.

use foundation_auth::server::services::client_service::create_client_record;

#[test]
fn test_create_client_record() {
    let (client, secret) = create_client_record(
        "My App",
        vec!["https://app.example.com/cb".into()],
        vec!["authorization_code".into()],
        vec!["openid".into()],
        false,
    );

    assert!(!client.id.is_empty());
    assert_eq!(client.name, "My App");
    assert!(!secret.is_empty());
    assert!(client.verify_secret(&secret));
}
