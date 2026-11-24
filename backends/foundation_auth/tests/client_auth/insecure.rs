//! The plaintext provider for development: loopback only by default.

use std::net::SocketAddr;

use foundation_auth::client_auth::{AuthError, AuthMethod, ClientAuth, InsecureNoAuth, Principal};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

use super::common;

fn connection(peer: Option<SocketAddr>) -> ConnectionContext {
    ConnectionContext {
        peer_addr: peer,
        ..Default::default()
    }
}

async fn authenticate(
    auth: &InsecureNoAuth,
    peer: Option<SocketAddr>,
) -> Result<Option<foundation_auth::client_auth::AuthInfo>, AuthError> {
    let headers = SimpleHeaders::new();
    let connection = connection(peer);
    let extensions = Extensions::new();
    auth.authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
}

#[valtron_test]
async fn a_loopback_peer_is_accepted() {
    let auth = InsecureNoAuth::new();
    let peer = SocketAddr::from(([127, 0, 0, 1], 4444));
    let info = authenticate(&auth, Some(peer))
        .await
        .expect("no error")
        .expect("accepted");
    let principal = info.artifacts.get::<Principal>().expect("a principal");
    assert!(matches!(principal.method, AuthMethod::Insecure));
}

#[valtron_test]
async fn a_non_loopback_peer_is_refused_by_default() {
    let auth = InsecureNoAuth::new();
    let peer = SocketAddr::from(([198, 51, 100, 7], 4444));
    let error = authenticate(&auth, Some(peer)).await.expect_err("refused");
    assert!(matches!(error, AuthError::Forbidden(_)));
}

#[valtron_test]
async fn a_non_loopback_peer_is_accepted_when_allowed() {
    let auth = InsecureNoAuth::new().with_allow_non_loopback(true);
    let peer = SocketAddr::from(([198, 51, 100, 7], 4444));
    assert!(authenticate(&auth, Some(peer))
        .await
        .expect("no error")
        .is_some());
}

#[valtron_test]
async fn no_peer_address_is_refused_unless_allowed() {
    let refused = InsecureNoAuth::new();
    assert!(matches!(
        authenticate(&refused, None).await,
        Err(AuthError::Forbidden(_))
    ));

    let allowed = InsecureNoAuth::new().with_allow_non_loopback(true);
    assert!(authenticate(&allowed, None)
        .await
        .expect("no error")
        .is_some());
}
