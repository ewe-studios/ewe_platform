//! `ClientService` tests.

use foundation_auth::server::services::client_service::create_client_record;
use foundation_core::valtron::{Clock, MonotonicClock};
use std::sync::Arc;

fn test_clock() -> Arc<dyn Clock> {
    Arc::new(MonotonicClock::new())
}

#[test]
fn test_create_client_record() {
    let (client, secret) = create_client_record(
        "My App",
        vec!["https://app.example.com/cb".into()],
        vec!["authorization_code".into()],
        vec!["openid".into()],
        false,
        test_clock().as_ref(),
    );

    assert_ne!(client.id, "");
    assert_eq!(client.name, "My App");
    assert_ne!(secret, "");
    assert!(client.verify_secret(&secret));
}
