//! The signed-secret provider over the trust store and the simulated clock.

use std::sync::Arc;

use foundation_auth::client_auth::{
    AccessPolicy, AuthMethod, ClientAuth, Principal, SignedSecretAuth,
};
use foundation_auth::secret::{
    SecretDomain, SecretIssuer, SecretRequest, SecretVerifier, SigningKey, VerifyPolicy,
};
use foundation_auth::trust::{TrustedClient, TrustedClients};
use foundation_compact::Duration;
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::Extensions;

use super::common;

const T0: u64 = 1_800_000_000;

async fn fixture() -> (
    Ctx,
    Arc<foundation_core::valtron::SimulatedClock>,
    SecretIssuer,
    Arc<TrustedClients>,
) {
    let (ctx, clock) = common::ctx_at(T0);
    let issuer = SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        SigningKey::from_seed(common::TEST_SEED, None),
        ctx.clone(),
        Duration::from_hours(2),
    );
    let trust = common::trusted(&ctx).await;
    (ctx, clock, issuer, trust)
}

fn make_auth(ctx: &Ctx, issuer: &SecretIssuer, trust: Arc<TrustedClients>) -> SignedSecretAuth {
    let verifier = SecretVerifier::new(
        SecretDomain::AGENT_RPC,
        vec![issuer.signing_key().verifying_key().clone()],
        ctx.clone(),
        VerifyPolicy::access(common::AUDIENCE),
    );
    SignedSecretAuth::new(verifier, trust, AccessPolicy::default())
}

/// Enroll a client-key client under `client_id`, without a certificate.
async fn enroll(trust: &TrustedClients, client_id: &str) {
    trust
        .record_enrollment(
            TrustedClient {
                fingerprint_sha256: format!("fp-{client_id}"),
                client_id: Some(client_id.to_string()),
                client_name: "browser".to_string(),
                enrolled_at: T0,
                expires_at: T0 + 86_400,
                last_seen_at: None,
                revoked_at: None,
            },
            "",
        )
        .await
        .expect("enroll the client");
}

#[valtron_test]
async fn valid_access_secret_is_accepted() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access(
            Duration::from_hours(1),
            common::AUDIENCE,
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let info = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error")
        .expect("accepted");

    let principal = info.artifacts.get::<Principal>().expect("a principal");
    assert_eq!(principal.subject, secret.id());
    assert!(matches!(principal.method, AuthMethod::SignedSecret { .. }));
}

#[valtron_test]
async fn access_secret_with_a_subject_keeps_the_client_id() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    enroll(&trust, "client-1").await;
    let secret = issuer
        .issue(&SecretRequest::access_for(
            Duration::from_hours(1),
            common::AUDIENCE,
            "client-1",
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let info = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error")
        .expect("accepted");
    let principal = info.artifacts.get::<Principal>().expect("a principal");
    assert_eq!(principal.subject, "client-1");
}

#[valtron_test]
async fn access_secret_for_a_revoked_client_is_refused() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    enroll(&trust, "client-1").await;
    trust
        .revoke_client("fp-client-1")
        .await
        .expect("revoke the client");
    let secret = issuer
        .issue(&SecretRequest::access_for(
            Duration::from_hours(1),
            common::AUDIENCE,
            "client-1",
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("a revoked client is refused");
    assert_eq!(error, foundation_auth::client_auth::AuthError::Revoked);
}

#[valtron_test]
async fn access_secret_for_an_unknown_client_is_refused() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access_for(
            Duration::from_hours(1),
            common::AUDIENCE,
            "not-enrolled",
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("an unknown client is refused");
    assert!(matches!(
        error,
        foundation_auth::client_auth::AuthError::Invalid(_)
    ));
}

#[valtron_test]
async fn missing_credentials_are_skipped() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let auth = make_auth(&ctx, &issuer, trust);
    let headers = foundation_netio::shared::http::SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_none());
}

#[valtron_test]
async fn jwt_shaped_credentials_are_skipped() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let auth = make_auth(&ctx, &issuer, trust);
    let headers = common::bearer_headers("header.payload.signature");
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_none());
}

#[valtron_test]
async fn expired_access_secret_is_refused() {
    let (ctx, clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access(
            Duration::from_hours(1),
            common::AUDIENCE,
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);
    // Move past the window plus the 5-minute skew.
    clock.advance_wall(Duration::from_secs(3600 + 301));

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("expired");
    assert_eq!(error, foundation_auth::client_auth::AuthError::Expired);
}

#[valtron_test]
async fn revoked_secret_is_refused() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access(
            Duration::from_hours(1),
            common::AUDIENCE,
        ))
        .expect("issue");
    trust
        .revoke_secret(secret.id().as_str(), secret.valid_until())
        .await
        .expect("revoke");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("revoked");
    assert_eq!(error, foundation_auth::client_auth::AuthError::Revoked);
}

#[valtron_test]
async fn spent_secret_is_refused() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access(Duration::from_hours(1), common::AUDIENCE).with_max_uses(1))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, Arc::clone(&trust));

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    auth.authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error")
        .expect("the first use is accepted");

    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("spent");
    assert_eq!(error, foundation_auth::client_auth::AuthError::Spent);
}

#[valtron_test]
async fn enrollment_secret_is_not_a_bearer_credential() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::enroll(Duration::from_hours(1)))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("wrong purpose");
    assert!(matches!(
        error,
        foundation_auth::client_auth::AuthError::Invalid(_)
    ));
}

#[valtron_test]
async fn secret_for_another_host_is_refused() {
    let (ctx, _clock, issuer, trust) = fixture().await;
    let secret = issuer
        .issue(&SecretRequest::access(
            Duration::from_hours(1),
            "https://other.example.com/rpc",
        ))
        .expect("issue");
    let auth = make_auth(&ctx, &issuer, trust);

    let headers = common::bearer_headers(secret.encode().as_str());
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let error = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect_err("wrong audience");
    assert!(matches!(
        error,
        foundation_auth::client_auth::AuthError::Invalid(_)
    ));
}
