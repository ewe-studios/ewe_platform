//! The provider that reads the principal a Worker forwards to a Durable Object.

use foundation_auth::client_auth::{
    AuthMethod, ClientAuth, ForwardedPrincipalAuth, Principal, FORWARDED_PRINCIPAL_HEADER,
};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeader, SimpleHeaders};

use super::common;

#[valtron_test]
async fn a_forwarded_principal_is_accepted() {
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::custom(FORWARDED_PRINCIPAL_HEADER),
        vec!["client-9".to_string()],
    );
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let info = ForwardedPrincipalAuth::new()
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error")
        .expect("accepted");
    let principal = info.artifacts.get::<Principal>().expect("a principal");
    assert_eq!(principal.subject, "client-9");
    assert!(matches!(principal.method, AuthMethod::Forwarded));
}

#[valtron_test]
async fn a_missing_header_is_skipped() {
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = ForwardedPrincipalAuth::new()
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_none());
}
