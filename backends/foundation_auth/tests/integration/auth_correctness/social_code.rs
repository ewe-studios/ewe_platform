//! A social-login authorization code is bound to the client that asked for it.
//!
//! The upstream provider is not reachable in a test, so the test starts at
//! the end of the flow: a `ToS` code whose pending login is a social login
//! for client A. Accepting it makes the authorization code (a 302 to the
//! client's redirect URI), exactly as `social_callback` does.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::models::{OAuthClient, PendingLogin};
use foundation_auth::server::services::{create_client_record, TosService};
use foundation_auth::server::storage;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::DataValue;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::integration::tos_login_gate::{config, start, Env};

/// A GET route of the `IdP` itself: the test client follows the 302 there.
const REDIRECT: &str = "http://testserver/auth/v1/templates/config";

fn new_client(env: &Env, name: &str) -> (OAuthClient, String) {
    let (client, secret) = create_client_record(
        name,
        vec![REDIRECT.into()],
        vec!["authorization_code".into()],
        vec!["openid".into()],
        false,
        env.storage.clock.as_ref(),
    );
    let query_store = Arc::clone(&env.storage.query_store);
    let stored = client.clone();
    async move { storage::store_oauth_client(query_store.as_ref(), &stored).await }
        .block()
        .expect("store client");
    (client, secret)
}

/// The authorization code the social flow makes for `client_id`.
fn social_code(env: &Env, user_id: &str, client_id: &str) -> String {
    env.seed_tos("v1", 1000);
    let (user_id_again, client_id) = (user_id.to_string(), client_id.to_string());
    let service = TosService::new(Arc::clone(&env.storage), Arc::clone(&env.storage.clock));
    let user_id = user_id.to_string();
    let pending = PendingLogin {
        auth_method: "social".into(),
        client_id: Some(client_id.clone()),
        redirect_uri: Some(REDIRECT.into()),
        state: Some("app-state".into()),
        nonce: None,
    };
    let tos_code = async move {
        service
            .gate(&user_id, pending, Duration::from_secs(60))
            .await
            .unwrap()
            .expect("a ToS code")
            .code
    }
    .block();

    // The 302 goes to REDIRECT, which the client follows (200). The code is
    // read from the store.
    let reply = env.accept_code(&tos_code, "v1", None);
    assert_eq!(reply.status, 200, "{}", reply.body);
    let query_store = Arc::clone(&env.storage.query_store);
    let user_id = user_id_again;
    async move {
        let mut rows = query_store
            .query(
                "SELECT code, client_id FROM authorization_codes WHERE user_id = ?",
                &[DataValue::Text(user_id)],
            )
            .await
            .unwrap();
        let row = futures_lite::StreamExt::next(&mut rows)
            .await
            .expect("an authorization code row")
            .unwrap();
        let stored_client: String = row.get_by_name("client_id").unwrap();
        assert_eq!(stored_client, client_id, "the code names its client");
        row.get_by_name::<String>("code").unwrap()
    }
    .block()
}

fn exchange(env: &Env, client_id: &str, secret: &str, code: &str) -> (usize, serde_json::Value) {
    let form = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&client_id={client_id}&client_secret={secret}",
        urlencoding::encode(REDIRECT)
    );
    let request = PreparedRequestBuilder::post("http://testserver/idp/token")
        .unwrap()
        .body_text(form)
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build();
    let (status, _, body) = env.client.send(request).unwrap().into_parts();
    let text = match body {
        foundation_netio::shared::http::SendSafeBody::Text(s) => s,
        foundation_netio::shared::http::SendSafeBody::Bytes(b) => {
            String::from_utf8_lossy(&b).to_string()
        }
        other => String::from_utf8_lossy(
            &foundation_netio::shared::client::body_reader::try_collect_bytes(other).unwrap(),
        )
        .to_string(),
    };
    (
        status.into_usize(),
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
    )
}

#[valtron_test(threads = 8, timeout = 60000)]
fn social_code_is_bound_to_its_client() {
    let env = start(config().with_require_pkce(false));
    let user = env.register("social@example.com");
    let (client_a, secret_a) = new_client(&env, "A");
    let (client_b, secret_b) = new_client(&env, "B");

    // A code made for A cannot be redeemed by B, even with B's own secret.
    let code = social_code(&env, &user, &client_a.id);
    let (status, body) = exchange(&env, &client_b.id, &secret_b, &code);
    assert_eq!(status, 400, "B must not redeem A's code: {body}");

    // A redeems it (the failed attempt by B did not use it up).
    let (status, body) = exchange(&env, &client_a.id, &secret_a, &code);
    assert_eq!(status, 200, "A redeems its code: {body}");
    assert!(body["access_token"].is_string());

    // And a code is single use.
    let (status, _) = exchange(&env, &client_a.id, &secret_a, &code);
    assert_eq!(status, 400);
}
