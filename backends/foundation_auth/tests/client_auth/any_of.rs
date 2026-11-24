//! The composite provider: the first `Ok(Some)` wins, and an `Err` stops.

use std::sync::Arc;

use foundation_auth::client_auth::{AnyOf, AuthError, ClientAuth};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

use super::common::{self, Fixed, Outcome};

#[valtron_test]
async fn the_first_accepting_provider_wins() {
    let providers: Vec<Arc<dyn ClientAuth>> = vec![
        Arc::new(Fixed::new("skip", Outcome::Skip)),
        Arc::new(Fixed::new("accept", Outcome::Accept)),
        Arc::new(Fixed::new(
            "later",
            Outcome::Fail(AuthError::Invalid("must not run".to_string())),
        )),
    ];
    let any_of = AnyOf::new(providers);
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = any_of
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_some());
}

#[valtron_test]
async fn an_error_stops_the_chain() {
    let providers: Vec<Arc<dyn ClientAuth>> = vec![
        Arc::new(Fixed::new("fail", Outcome::Fail(AuthError::Expired))),
        Arc::new(Fixed::new("accept", Outcome::Accept)),
    ];
    let any_of = AnyOf::new(providers);
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = any_of
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("the chain stops");
    assert_eq!(error, AuthError::Expired);
}

#[valtron_test]
async fn every_provider_skipping_gives_none() {
    let providers: Vec<Arc<dyn ClientAuth>> = vec![
        Arc::new(Fixed::new("a", Outcome::Skip)),
        Arc::new(Fixed::new("b", Outcome::Skip)),
    ];
    let any_of = AnyOf::new(providers);
    let headers = SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = any_of
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_none());
}
