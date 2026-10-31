//! Streamed body-read tests for the auth HTTP clients.
//!
//! WHY: Feature 18, work items E9 and E10, changed the auth HTTP clients to
//! read response bodies as async streams. Each client checks the status code
//! first. Each client caps the error body at 64 kibibytes (KiB). These tests
//! check that behaviour over a real Hypertext Transfer Protocol (HTTP) server.
//!
//! WHAT: Success tests check that each client parses a valid JavaScript Object
//! Notation (JSON) body. Error tests check that an error body larger than
//! 64 KiB is cut at the cap. One parking test checks that a pending body read
//! does not block other work.
//!
//! HOW: `foundation_testing::http::TestHttpServer` serves canned responses.
//! The async client calls run on the valtron pool through `from_future`,
//! `execute`, and `collect_one`, inside `#[valtron_test]`.

use foundation_auth::shared::body::DEFAULT_MAX_ERROR_BODY;
use foundation_auth::shared::discovery::DiscoveryError;
use foundation_auth::shared::introspection::IntrospectionError;
use foundation_auth::shared::jwks::JwksError;
use foundation_auth::shared::userinfo::UserInfoError;
use foundation_auth::{
    DiscoveryClient, IntrospectionClient, JwksManager, ProviderMapping, ProviderType,
    UpstreamClientError, UpstreamOidcClient, UpstreamProvider, UserInfoClient,
};
use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_netio::HttpClientBuilder;
use foundation_testing::http::{HttpResponse, TestHttpServer};

/// The fixed prefix the clients put before a capped error body for a 500.
const HTTP_500_PREFIX: &str = "HTTP 500: ";

/// Drive an async future to completion on the valtron pool and return its output.
fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

/// Build a non-200 response with an explicit body.
fn error_response(status: u16, status_text: &str, body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        status_text: status_text.to_string(),
        headers: vec![("Content-Length".to_string(), body.len().to_string())],
        body: body.to_vec(),
    }
}

/// A body twice the size of the error cap.
fn oversized_body() -> Vec<u8> {
    vec![b'x'; 128 * 1024]
}

/// A minimal OAuth2 provider for the upstream client tests.
fn test_provider() -> UpstreamProvider {
    UpstreamProvider {
        id: "test".into(),
        name: "Test".into(),
        provider_type: ProviderType::Oauth2,
        client_id: "client-abc".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: Some("https://idp.example/authorize".into()),
        token_url: None,
        userinfo_url: None,
        discovery_url: None,
        scopes: vec!["openid".into(), "email".into()],
        is_active: true,
        mapping_config: ProviderMapping::default(),
        created_at: 0,
        updated_at: 0,
    }
}

/// A discovery document the discovery and parking tests serve.
const DISCOVERY_JSON: &str = r#"{"issuer":"https://idp.example","authorization_endpoint":"https://idp.example/authorize","token_endpoint":"https://idp.example/token","userinfo_endpoint":"https://idp.example/userinfo","jwks_uri":"https://idp.example/jwks","response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"]}"#;

// ===========================================================================
// E10 success tests: each shared client parses a valid JSON body.
// ===========================================================================

#[valtron_test]
fn discovery_parses_document() {
    let json = DISCOVERY_JSON;
    let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
        .close_after_response(true);

    let url = server.url("/.well-known/openid-configuration");
    let client = DiscoveryClient::with_client(HttpClientBuilder::new().build());

    let doc = drive(async move { client.fetch_from_url(&url).await }).expect("discovery fetch");

    assert_eq!(doc.issuer, "https://idp.example");
    assert_eq!(doc.token_endpoint, "https://idp.example/token");
    assert_eq!(
        doc.userinfo_endpoint.as_deref(),
        Some("https://idp.example/userinfo")
    );
}

#[valtron_test]
fn jwks_parses_key_set() {
    let json = r#"{"keys":[{"kty":"OKP","crv":"Ed25519","x":"Fb6xzYJqMxN0KqZ6F3h4YvGkHxT0zG0qHqZ3Yb1x4Yc","kid":"key-1","use":"sig","alg":"EdDSA"}]}"#;
    let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
        .close_after_response(true);

    let jwks_url = server.url("/jwks");
    let mut manager =
        JwksManager::with_client(HttpClientBuilder::new().build(), jwks_url, None);

    let (count, kid) = drive(async move {
        manager
            .fetch()
            .await
            .map(|jwks| (jwks.keys.len(), jwks.keys.first().map(|k| k.key_id.clone())))
    })
    .expect("jwks fetch");

    assert_eq!(count, 1);
    assert_eq!(kid.as_deref(), Some("key-1"));
}

#[valtron_test]
fn introspection_parses_result() {
    let json = r#"{"active":true,"scope":"openid","client_id":"client","sub":"user-1","username":"alice","token_type":"Bearer"}"#;
    let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
        .close_after_response(true);

    let url = server.url("/introspect");
    let client = IntrospectionClient::with_client(HttpClientBuilder::new().build());

    let result = drive(async move {
        client
            .introspect(&url, "tok", "client", "secret")
            .await
    })
    .expect("introspection");

    assert!(result.active);
    assert_eq!(result.sub.as_deref(), Some("user-1"));
    assert_eq!(result.client_id.as_deref(), Some("client"));
}

#[valtron_test]
fn userinfo_parses_profile() {
    let json = r#"{"sub":"user-1","name":"Alice","email":"alice@example.com","email_verified":true,"preferred_username":"alice"}"#;
    let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
        .close_after_response(true);

    let url = server.url("/userinfo");
    let client = UserInfoClient::with_client(HttpClientBuilder::new().build());

    let info = drive(async move { client.fetch(&url, "access-tok").await }).expect("userinfo fetch");

    assert_eq!(info.sub, "user-1");
    assert_eq!(info.email.as_deref(), Some("alice@example.com"));
    assert_eq!(info.email_verified, Some(true));
}

#[valtron_test]
fn upstream_token_exchange_parses_token() {
    let json = r#"{"access_token":"at-1","token_type":"Bearer","expires_in":3600,"refresh_token":"rt-1","scope":"openid","id_token":"idt-1"}"#;
    let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
        .close_after_response(true);

    let mut provider = test_provider();
    provider.token_url = Some(server.url("/token"));
    let mut client = UpstreamOidcClient::new(provider, "https://app.example/callback".into());
    client.configure().expect("configure");

    let token = drive(async move { client.exchange_code("code", Some("verifier"), None).await })
        .expect("token exchange");

    assert_eq!(token.access_token, "at-1");
    assert_eq!(token.refresh_token.as_deref(), Some("rt-1"));
    assert_eq!(token.id_token.as_deref(), Some("idt-1"));
}

// ===========================================================================
// E10 error tests: an error body larger than 64 KiB is cut at the cap.
// ===========================================================================

#[valtron_test]
fn discovery_error_body_is_capped() {
    let body = oversized_body();
    let original = body.clone();
    let server = TestHttpServer::with_response(move |_req| {
        error_response(500, "Internal Server Error", &body)
    })
    .close_after_response(true);

    let url = server.url("/.well-known/openid-configuration");
    let client = DiscoveryClient::with_client(HttpClientBuilder::new().build());

    let err = drive(async move { client.fetch_from_url(&url).await }).expect_err("a 500 is an error");

    let text = match err {
        DiscoveryError::FetchFailed(msg) => msg
            .strip_prefix(HTTP_500_PREFIX)
            .expect("the message carries the HTTP prefix")
            .to_string(),
        other => panic!("unexpected discovery error: {other:?}"),
    };
    assert!(text.len() <= DEFAULT_MAX_ERROR_BODY);
    assert!(original.starts_with(text.as_bytes()));
}

#[valtron_test]
fn jwks_error_body_is_capped() {
    let body = oversized_body();
    let original = body.clone();
    let server = TestHttpServer::with_response(move |_req| {
        error_response(500, "Internal Server Error", &body)
    })
    .close_after_response(true);

    let jwks_url = server.url("/jwks");
    let mut manager =
        JwksManager::with_client(HttpClientBuilder::new().build(), jwks_url, None);

    let err = drive(async move { manager.fetch().await.map(|_| ()) })
        .expect_err("a 500 is an error");

    let text = match err {
        JwksError::JwksFetchFailed(msg) => msg
            .strip_prefix(HTTP_500_PREFIX)
            .expect("the message carries the HTTP prefix")
            .to_string(),
        other => panic!("unexpected jwks error: {other:?}"),
    };
    assert!(text.len() <= DEFAULT_MAX_ERROR_BODY);
    assert!(original.starts_with(text.as_bytes()));
}

#[valtron_test]
fn introspection_error_body_is_capped() {
    let body = oversized_body();
    let original = body.clone();
    let server = TestHttpServer::with_response(move |_req| {
        error_response(401, "Unauthorized", &body)
    })
    .close_after_response(true);

    let url = server.url("/introspect");
    let client = IntrospectionClient::with_client(HttpClientBuilder::new().build());

    let err = drive(async move {
        client
            .introspect(&url, "tok", "client", "secret")
            .await
    })
    .expect_err("a 401 is an error");

    // A 401 carries the raw body with no HTTP prefix.
    let text = match err {
        IntrospectionError::Unauthorized(msg) => msg,
        other => panic!("unexpected introspection error: {other:?}"),
    };
    assert!(text.len() <= DEFAULT_MAX_ERROR_BODY);
    assert!(original.starts_with(text.as_bytes()));
}

#[valtron_test]
fn userinfo_error_body_is_capped() {
    let body = oversized_body();
    let original = body.clone();
    let server = TestHttpServer::with_response(move |_req| {
        error_response(500, "Internal Server Error", &body)
    })
    .close_after_response(true);

    let url = server.url("/userinfo");
    let client = UserInfoClient::with_client(HttpClientBuilder::new().build());

    let err = drive(async move { client.fetch(&url, "tok").await }).expect_err("a 500 is an error");

    let text = match err {
        UserInfoError::FetchFailed(msg) => msg
            .strip_prefix(HTTP_500_PREFIX)
            .expect("the message carries the HTTP prefix")
            .to_string(),
        other => panic!("unexpected userinfo error: {other:?}"),
    };
    assert!(text.len() <= DEFAULT_MAX_ERROR_BODY);
    assert!(original.starts_with(text.as_bytes()));
}

#[valtron_test]
fn upstream_token_exchange_error_body_is_capped() {
    let body = oversized_body();
    let original = body.clone();
    let server = TestHttpServer::with_response(move |_req| {
        error_response(500, "Internal Server Error", &body)
    })
    .close_after_response(true);

    let mut provider = test_provider();
    provider.token_url = Some(server.url("/token"));
    let mut client = UpstreamOidcClient::new(provider, "https://app.example/callback".into());
    client.configure().expect("configure");

    let err = drive(async move { client.exchange_code("code", None, None).await })
        .expect_err("a 500 is an error");

    let text = match err {
        UpstreamClientError::TokenExchange(msg) => msg
            .strip_prefix(HTTP_500_PREFIX)
            .expect("the message carries the HTTP prefix")
            .to_string(),
        other => panic!("unexpected upstream error: {other:?}"),
    };
    assert!(text.len() <= DEFAULT_MAX_ERROR_BODY);
    assert!(original.starts_with(text.as_bytes()));
}

// ===========================================================================
// Native-only tests: E9 token parsing, password error message, and parking.
// ===========================================================================

#[cfg(not(target_family = "wasm"))]
mod native {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;
    use foundation_auth::{
        ConfidentialText, NativeOAuth, OAuthConfig, OAuthToken, PasswordAuthClient,
        PasswordAuthError,
    };
    use foundation_core::valtron::timer::sleep_for;

    /// WHY: the old client read the token body as an empty body, so this parse
    /// failed. The streamed read must deliver the body and parse it.
    /// WHAT: a 200 with a full token JSON parses into the expected fields.
    #[valtron_test]
    fn native_oauth_parses_token_response() {
        let json = r#"{"access_token":"a","token_type":"Bearer","expires_in":3600,"refresh_token":"r","scope":"s","id_token":"i"}"#;
        let server = TestHttpServer::with_response(move |_req| HttpResponse::ok(json))
            .close_after_response(true);

        let config = OAuthConfig::builder()
            .client_id("client-id")
            .authorization_url("https://idp.example/authorize")
            .token_url(server.url("/token"))
            .redirect_uri("https://app.example/callback")
            .build();
        let client = NativeOAuth::with_client(HttpClientBuilder::new().build(), config);

        let token: OAuthToken = drive(async move {
            client.exchange_code_async("code", Some("verifier")).await
        })
        .expect("token exchange succeeds");

        assert_eq!(token.access_token, "a");
        assert_eq!(token.token_type, "Bearer");
        assert_eq!(token.expires_in, Some(3600));
        assert_eq!(token.refresh_token.as_deref(), Some("r"));
        assert_eq!(token.scope.as_deref(), Some("s"));
        assert_eq!(token.id_token.as_deref(), Some("i"));
    }

    /// WHY: a non-2xx login response must report its body as the error message.
    /// WHAT: a 500 with a JSON body becomes `ServerError` with that body.
    #[valtron_test]
    fn password_auth_reports_error_message() {
        let server = TestHttpServer::with_response(move |_req| {
            error_response(
                500,
                "Internal Server Error",
                br#"{"error":"internal","message":"something broke"}"#,
            )
        })
        .close_after_response(true);

        let base_url = server.url("");
        let client = PasswordAuthClient::with_client(HttpClientBuilder::new().build(), base_url);
        let password = ConfidentialText::new("secret".to_string());

        let result = drive(async move { client.login_email("alice@example.com", &password).await });

        match result {
            Err(PasswordAuthError::ServerError { status, message }) => {
                assert_eq!(status, 500);
                assert!(!message.is_empty(), "the error message must carry the body");
            }
            other => panic!("expected a server error, got {other:?}"),
        }
    }

    /// WHY: a body read that spins would stop the pool from running other work.
    /// The streamed read parks while it waits, so a concurrent ticker advances.
    /// WHAT: a discovery fetch waits 300 ms for its body; a ticker counts
    /// every 10 ms on another pool task. The ticker must advance while the
    /// body read is pending.
    ///
    /// NOTE: the native valtron pool always runs at least two task workers, so
    /// this proves the pending read does not block the pool. It cannot, on its
    /// own, tell a spinning single task from a parked one.
    #[valtron_test]
    fn pending_body_read_does_not_block_other_tasks() {
        let json = DISCOVERY_JSON;
        let server = TestHttpServer::with_response(move |_req| {
            std::thread::sleep(std::time::Duration::from_millis(300));
            HttpResponse::ok(json)
        });

        let url = server.url("/.well-known/openid-configuration");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_ticker = Arc::clone(&counter);
        let counter_for_client = Arc::clone(&counter);
        let snapshot = Arc::new(AtomicUsize::new(0));
        let snapshot_for_client = Arc::clone(&snapshot);

        // Task one: the discovery fetch whose body read stays pending ~300 ms.
        let client_task = from_future(async move {
            let ok = DiscoveryClient::with_client(HttpClientBuilder::new().build())
                .fetch_from_url(&url)
                .await
                .is_ok();
            snapshot_for_client.store(counter_for_client.load(Ordering::SeqCst), Ordering::SeqCst);
            ok
        });
        let client_stream = execute(client_task, None).expect("execute client");

        // Task two: a ticker that counts every 10 ms on the same pool.
        let ticker_task = from_future(async move {
            for _ in 0..50 {
                counter_for_ticker.fetch_add(1, Ordering::SeqCst);
                sleep_for(std::time::Duration::from_millis(10)).await;
            }
        });
        let ticker_stream = execute(ticker_task, None).expect("execute ticker");

        assert_eq!(collect_one(client_stream), Some(true), "the fetch must succeed");
        let _ = collect_one(ticker_stream);

        let seen = snapshot.load(Ordering::SeqCst);
        assert!(
            seen >= 1,
            "the ticker advanced {seen} time(s) while the body read was pending"
        );
    }
}
