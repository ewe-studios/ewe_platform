//! Social login sends the authorization code to the redirect URI, so the URI
//! must be one the client registered.

use foundation_auth::server::models::OAuthClient;
use foundation_auth::server::storage;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};

use crate::integration::tos_login_gate::{config, start};

const REGISTERED: &str = "https://app.example/callback";

fn register_client(env: &crate::integration::tos_login_gate::Env) -> String {
    let client = OAuthClient {
        id: "client-1".into(),
        name: "App".into(),
        client_secret_hash: "x".into(),
        redirect_uris: vec![REGISTERED.into()],
        grant_types: vec!["authorization_code".into()],
        scopes: vec!["openid".into()],
        is_public: true,
        created_at: 1,
    };
    let query_store = std::sync::Arc::clone(&env.storage.query_store);
    async move { storage::store_oauth_client(query_store.as_ref(), &client).await }
        .block()
        .expect("store client");
    "client-1".into()
}

fn authorize(
    env: &crate::integration::tos_login_gate::Env,
    client: Option<&str>,
    redirect: &str,
) -> crate::integration::tos_login_gate::Reply {
    let client_param = client.map_or(String::new(), |c| format!("&client_id={c}"));
    let url = format!(
        "/idp/authorize?provider=nope&state=s&redirect_uri={}{client_param}",
        urlencoding::encode(redirect)
    );
    env.get(&url, None)
}

#[valtron_test(threads = 8, timeout = 60000)]
fn social_authorize_rejects_unregistered_redirect_uri() {
    let env = start(config());
    let client = register_client(&env);

    let evil = authorize(&env, Some(&client), "https://evil.example/steal");
    let prefix = authorize(&env, Some(&client), "https://app.example/callback/extra");
    let no_client = authorize(&env, None, REGISTERED);
    let unknown = authorize(&env, Some("nobody"), REGISTERED);

    for (name, reply) in [
        ("evil", &evil),
        ("prefix", &prefix),
        ("unknown client", &unknown),
    ] {
        assert_eq!(reply.status, 400, "{name}: {}", reply.body);
    }
    assert_eq!(evil.body["error_description"], "redirect_uri not allowed");
    assert_eq!(prefix.body["error_description"], "redirect_uri not allowed");
    assert_eq!(no_client.status, 400, "{}", no_client.body);

    // The registered URI passes this check; the request then stops at the
    // provider lookup (there is no provider "nope").
    let ok = authorize(&env, Some(&client), REGISTERED);
    assert_eq!(ok.status, 404, "{}", ok.body);
}
