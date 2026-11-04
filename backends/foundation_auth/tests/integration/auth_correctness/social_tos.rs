//! A social login that needs a `ToS` acceptance ends with a redirect to the
//! `ToS` page (code in the fragment), not with JSON on the browser callback.
//!
//! The upstream provider is a local test server: it serves the token and the
//! user profile.

use foundation_auth::server::models::OAuthClient;
use foundation_auth::server::storage;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::DataValue;
use foundation_testing::http::{HttpResponse, TestHttpServer};

use crate::integration::tos_login_gate::{config, start, Env};

const TOS_PAGE: &str = "https://app.example/terms";

fn add_provider(env: &Env, upstream_base: &str) {
    let query_store = std::sync::Arc::clone(&env.storage.query_store);
    let (token_url, userinfo_url, authorize_url) = (
        format!("{upstream_base}/token"),
        format!("{upstream_base}/userinfo"),
        format!("{upstream_base}/authorize"),
    );
    async move {
        query_store
            .execute(
                "INSERT INTO upstream_providers (id, name, provider_type, client_id, \
                 authorization_url, token_url, userinfo_url) VALUES (?, ?, 'oauth2', ?, ?, ?, ?)",
                &[
                    DataValue::Text("fake".into()),
                    DataValue::Text("Fake".into()),
                    DataValue::Text("upstream-client".into()),
                    DataValue::Text(authorize_url),
                    DataValue::Text(token_url),
                    DataValue::Text(userinfo_url),
                ],
            )
            .await
            .expect("insert provider");
    }
    .block();
}

/// The OAuth client the app logs in with (`client-1`, one registered redirect).
fn register_client(env: &Env) {
    let client = OAuthClient {
        id: "client-1".into(),
        name: "App".into(),
        client_secret_hash: "x".into(),
        redirect_uris: vec!["https://app.example/callback".into()],
        grant_types: vec!["authorization_code".into()],
        scopes: vec!["openid".into()],
        is_public: true,
        created_at: 1,
    };
    let query_store = std::sync::Arc::clone(&env.storage.query_store);
    async move { storage::store_oauth_client(query_store.as_ref(), &client).await }
        .block()
        .expect("store client");
}

fn upstream() -> TestHttpServer {
    TestHttpServer::with_response(|req| {
        if req.path.url.contains("/token") {
            HttpResponse::ok(
                r#"{"access_token":"at-1","token_type":"Bearer","expires_in":3600}"#,
            )
        } else {
            HttpResponse::ok(
                r#"{"sub":"up-1","email":"social-tos@example.com","email_verified":true,"name":"S"}"#,
            )
        }
    })
    .close_after_response(true)
}

/// Start the social flow and return the upstream `state`.
fn social_state(env: &Env) -> String {
    let (status, headers) = env.raw_get(&format!(
        "/idp/authorize?provider=fake&client_id=client-1&state=app-state&redirect_uri={}",
        urlencoding::encode("https://app.example/callback")
    ));
    assert_eq!(status, 302, "{headers:?}");
    let location = &headers
        .iter()
        .find(|(k, _)| k == "location")
        .expect("Location")
        .1;
    location
        .split('&')
        .chain(location.split('?'))
        .find_map(|p| p.strip_prefix("state="))
        .expect("state in the upstream URL")
        .to_string()
}

#[valtron_test(threads = 8, timeout = 60000)]
fn social_callback_with_unaccepted_tos_redirects() {
    let upstream = upstream();
    let env = start(config().with_tos_page_url(TOS_PAGE));
    add_provider(&env, upstream.base_url().trim_end_matches('/'));
    register_client(&env);
    env.seed_tos("v1", 1000);
    let state = social_state(&env);

    let (status, headers) = env.raw_get(&format!(
        "/idp/callback?provider=fake&code=abc&state={state}"
    ));

    assert_eq!(status, 302, "{headers:?}");
    let location = &headers
        .iter()
        .find(|(k, _)| k == "location")
        .expect("Location")
        .1;
    assert!(
        location.starts_with(&format!("{TOS_PAGE}#tos_await_code=")),
        "{location}"
    );
    assert!(location.contains("&tos_version=v1"), "{location}");
    assert!(
        !location.contains('?'),
        "the code is in the fragment only: {location}"
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn without_a_tos_page_the_social_callback_answers_206() {
    let upstream = upstream();
    let env = start(config());
    add_provider(&env, upstream.base_url().trim_end_matches('/'));
    register_client(&env);
    env.seed_tos("v1", 1000);
    let state = social_state(&env);

    let (status, _) = env.raw_get(&format!(
        "/idp/callback?provider=fake&code=abc&state={state}"
    ));

    assert_eq!(status, 206);
}
