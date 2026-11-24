//! The JSON Web Token (JWT) provider, with the time from the injected clock.

use std::sync::Arc;

use foundation_auth::client_auth::{AuthMethod, ClientAuth, JwtAuth, Principal};
use foundation_auth::shared::jwt::{
    JwtAlgorithm, JwtSigningKey, JwtVerifier, JwtVerifierConfig, PublicKeySource,
};
use foundation_compact::Duration;
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::{valtron_test, SimulatedClock};
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::Extensions;

use super::common;

const ISSUER: &str = "https://id.example.com";
const AUDIENCE: &str = "agent-rpc";

fn verifier(signing: &JwtSigningKey) -> JwtVerifier {
    JwtVerifier::from_config(JwtVerifierConfig {
        allowed_algorithms: vec![JwtAlgorithm::EdDSA],
        issuer: Some(ISSUER.to_string()),
        audience: Some(AUDIENCE.to_string()),
        key_id: None,
        public_key: PublicKeySource::Pem(signing.public_key_pem().expect("public key")),
    })
    .expect("the verifier builds")
}

fn now_unix(ctx: &Ctx) -> u64 {
    u64::try_from(foundation_compact::time::unix_seconds(
        ctx.clock().wall_now(),
    ))
    .unwrap_or(0)
}

async fn authenticate(
    auth: &JwtAuth,
    token: &str,
) -> Result<Option<foundation_auth::client_auth::AuthInfo>, foundation_auth::client_auth::AuthError>
{
    let headers = common::bearer_headers(token);
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    auth.authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
}

#[valtron_test]
async fn a_valid_jwt_is_accepted() {
    let ctx = Ctx::host();
    let signing = JwtSigningKey::generate_ed25519();
    let exp = now_unix(&ctx) + 3600;
    let token = signing
        .sign_claims(
            &serde_json::json!({
                "sub": "client-7",
                "iss": ISSUER,
                "aud": AUDIENCE,
                "exp": exp,
            }),
            &ctx,
        )
        .expect("sign");
    let auth = JwtAuth::new(verifier(&signing), &ctx);

    let info = authenticate(&auth, &token)
        .await
        .expect("no error")
        .expect("accepted");
    let principal = info.artifacts.get::<Principal>().expect("a principal");
    assert_eq!(principal.subject, "client-7");
    assert!(matches!(principal.method, AuthMethod::Jwt { .. }));
}

#[valtron_test]
async fn an_expired_jwt_is_refused() {
    let ctx = Ctx::host();
    let signing = JwtSigningKey::generate_ed25519();
    let exp = now_unix(&ctx) + 1;
    let token = signing
        .sign_claims(
            &serde_json::json!({
                "sub": "client-7",
                "iss": ISSUER,
                "aud": AUDIENCE,
                "exp": exp,
            }),
            &ctx,
        )
        .expect("sign");

    // A simulated clock two hours after the expiry.
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(exp + 7200));
    let expired_ctx = Ctx::fixed(Arc::clone(&clock) as Arc<dyn Clock>);
    let auth = JwtAuth::new(verifier(&signing), &expired_ctx);

    let error = authenticate(&auth, &token).await.expect_err("expired");
    assert!(matches!(
        error,
        foundation_auth::client_auth::AuthError::Invalid(_)
    ));
}

#[valtron_test]
async fn a_token_of_another_shape_is_skipped() {
    let ctx = Ctx::host();
    let signing = JwtSigningKey::generate_ed25519();
    let auth = JwtAuth::new(verifier(&signing), &ctx);

    let result = authenticate(&auth, "not-a-jwt").await.expect("no error");
    assert!(result.is_none());
}

#[valtron_test]
async fn missing_credentials_are_skipped() {
    let ctx = Ctx::host();
    let signing = JwtSigningKey::generate_ed25519();
    let auth = JwtAuth::new(verifier(&signing), &ctx);

    let headers = foundation_netio::shared::http::SimpleHeaders::new();
    let connection = ConnectionContext::default();
    let extensions = Extensions::new();
    let result = auth
        .authenticate(common::call(&headers, &connection, &extensions, "/t/Call"))
        .await
        .expect("no error");
    assert!(result.is_none());
}
