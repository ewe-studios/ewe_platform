//! Integration tests for `UpstreamOidcClient` token exchange + userinfo.
//!
//! WHY: F007 made the upstream broker client fully cross-platform by running
//! `exchange_code` and `fetch_userinfo` over the shared `default_http_client()`
//! (`Arc<dyn HttpClient>`). These tests drive both async methods end-to-end
//! against a real local HTTP server, over the same client wasm/Workers use,
//! proving the OAuth2 `authorization_code` grant and the userinfo Bearer fetch
//! actually speak HTTP correctly — not just that the parsing helpers work.
//!
//! WHAT: `exchange_code` posts the grant to a live token endpoint and parses the
//! `OAuthToken`; `fetch_userinfo` sends `Authorization: Bearer` to a live
//! userinfo endpoint and maps the claims through the provider's mapping.
//!
//! HOW: `foundation_testing::TestHttpServer::with_response` serves canned JSON;
//! it auto-answers the client's default `Expect: 100-continue` and reads the
//! full request body before invoking the handler, so the posted form is
//! observable. The async calls are driven on the valtron pool via `from_future`
//! + `execute` + `collect_one` inside `#[valtron_test]`.

use std::sync::{Arc, Mutex};

use foundation_auth::{
    DiscoveryClient, ProviderMapping, ProviderType, UpstreamOidcClient, UpstreamProvider,
};
use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};
use foundation_testing::http::{HttpResponse, TestHttpServer};

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

/// A minimal OAuth2 provider; callers override the endpoint URLs to point at the
/// test server.
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

/// Read a request body as a UTF-8 string regardless of framing.
fn body_string(body: &SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(t) => t.clone(),
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

fn header_value(headers: &SimpleHeaders, name: SimpleHeader) -> Option<String> {
    headers.get(&name).and_then(|v| v.first()).cloned()
}

#[valtron_test]
fn exchange_code_posts_grant_and_parses_token() {
    // Capture what the client actually sent to the token endpoint.
    let captured: Arc<Mutex<Option<(SimpleMethod, String)>>> = Arc::new(Mutex::new(None));
    let cap = Arc::clone(&captured);

    // `with_response` auto-answers the client's default `Expect: 100-continue`
    // with `100 Continue` and reads the full body before invoking the handler,
    // so `req.body` holds the posted form.
    let server = TestHttpServer::with_response(move |req| {
        *cap.lock().unwrap() = Some((req.method.clone(), body_string(&req.body)));
        let token = r#"{"access_token":"at-123","token_type":"Bearer","expires_in":3600,"refresh_token":"rt-456","scope":"openid email","id_token":"idt-789"}"#;
        HttpResponse::ok(token)
    })
    .close_after_response(true);

    let mut provider = test_provider();
    provider.token_url = Some(server.url("/token"));
    let mut client =
        UpstreamOidcClient::new(provider, "https://app.example/callback".into());
    client.configure().expect("configure");

    let token = drive(async move {
        client
            .exchange_code("auth-code-xyz", Some("verifier-abc"), Some("secret-shh"))
            .await
    })
    .expect("token exchange succeeds");

    assert_eq!(token.access_token, "at-123");
    assert_eq!(token.token_type, "Bearer");
    assert_eq!(token.expires_in, Some(3600));
    assert_eq!(token.refresh_token.as_deref(), Some("rt-456"));
    assert_eq!(token.id_token.as_deref(), Some("idt-789"));

    let (method, body) = captured.lock().unwrap().clone().expect("server saw the request");
    assert_eq!(method, SimpleMethod::POST);
    assert!(body.contains("grant_type=authorization_code"), "body: {body}");
    assert!(body.contains("code=auth-code-xyz"), "body: {body}");
    assert!(body.contains("redirect_uri="), "body: {body}");
    assert!(body.contains("client_id=client-abc"), "body: {body}");
    assert!(body.contains("client_secret=secret-shh"), "body: {body}");
    assert!(body.contains("code_verifier=verifier-abc"), "body: {body}");
}

#[valtron_test]
fn exchange_code_surfaces_endpoint_error() {
    let server = TestHttpServer::with_response(|_req| HttpResponse::status(400, "Bad Request"))
        .close_after_response(true);

    let mut provider = test_provider();
    provider.token_url = Some(server.url("/token"));
    let mut client =
        UpstreamOidcClient::new(provider, "https://app.example/callback".into());
    client.configure().expect("configure");

    let result = drive(async move {
        client.exchange_code("bad-code", None, None).await
    });
    assert!(result.is_err(), "a 400 from the token endpoint must be an error");
}

#[valtron_test]
fn fetch_userinfo_sends_bearer_and_maps_profile() {
    let captured: Arc<Mutex<Option<(SimpleMethod, Option<String>)>>> = Arc::new(Mutex::new(None));
    let cap = Arc::clone(&captured);

    let server = TestHttpServer::with_response(move |req| {
        let auth = header_value(&req.headers, SimpleHeader::AUTHORIZATION);
        *cap.lock().unwrap() = Some((req.method.clone(), auth));
        let profile = r#"{"sub":"user-1","email":"alice@example.com","email_verified":true,"name":"Alice","preferred_username":"alice"}"#;
        HttpResponse::ok(profile)
    })
    .close_after_response(true);

    let mut provider = test_provider();
    provider.userinfo_url = Some(server.url("/userinfo"));
    // fetch_userinfo reads provider.userinfo_url directly — no configure() needed.
    let client = UpstreamOidcClient::new(provider, "https://app.example/callback".into());

    let profile = drive(async move { client.fetch_userinfo("access-tok-xyz").await })
        .expect("userinfo fetch succeeds");

    assert_eq!(profile.sub, "user-1");
    assert_eq!(profile.email.as_deref(), Some("alice@example.com"));
    assert!(profile.email_verified);
    assert_eq!(profile.name.as_deref(), Some("Alice"));
    assert_eq!(profile.preferred_username.as_deref(), Some("alice"));

    let (method, auth) = captured.lock().unwrap().clone().expect("server saw the request");
    assert_eq!(method, SimpleMethod::GET);
    assert_eq!(auth.as_deref(), Some("Bearer access-tok-xyz"));
}

#[valtron_test]
fn discover_and_configure_reads_endpoints_from_discovery_doc() {
    // Proves the OIDC-discovery HTTP path (the direct dependency of an OIDC
    // provider login): the `.well-known` document is fetched over the shared
    // client and its endpoints drive the client's config + userinfo endpoint.
    let server = TestHttpServer::with_response(move |req| {
        // Serve a discovery doc whose endpoints point back at this server.
        // The handler can't know its own base URL, so use a relative-free doc
        // with absolute placeholders the test rewrites below is overkill —
        // instead echo a doc built from the Host header.
        let host = req
            .headers
            .get(&SimpleHeader::HOST)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();
        let base = format!("http://{host}");
        let doc = format!(
            r#"{{"issuer":"{base}","authorization_endpoint":"{base}/authorize","token_endpoint":"{base}/token","userinfo_endpoint":"{base}/userinfo","jwks_uri":"{base}/jwks","response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"]}}"#
        );
        HttpResponse::ok(doc)
    })
    .close_after_response(true);

    let mut provider = test_provider();
    provider.provider_type = ProviderType::Oidc;
    provider.authorization_url = None;
    provider.discovery_url = Some(server.url("/.well-known/openid-configuration"));

    let mut client =
        UpstreamOidcClient::new(provider, "https://app.example/callback".into());

    let configured = drive(async move {
        let mut discovery = DiscoveryClient::new();
        client.discover_and_configure(&mut discovery).await.map(|()| client)
    });
    let client = configured.expect("discovery + configure succeeds");

    // The userinfo endpoint must come from the discovered document.
    let userinfo = client.userinfo_endpoint().expect("discovered userinfo endpoint");
    assert!(userinfo.ends_with("/userinfo"), "userinfo: {userinfo}");

    // And the client can now build an authorize URL against the discovered endpoint.
    let (url, _pkce) = client.authorize_url("state-123").expect("authorize url");
    assert!(url.contains("/authorize?"), "url: {url}");
    assert!(url.contains("response_type=code"), "url: {url}");
}

#[valtron_test]
fn fetch_userinfo_without_endpoint_errors() {
    // Provider has no userinfo_url and no discovery — fetch must fail, not panic.
    let client = UpstreamOidcClient::new(
        test_provider(),
        "https://app.example/callback".into(),
    );
    let result = drive(async move { client.fetch_userinfo("tok").await });
    assert!(result.is_err(), "missing userinfo endpoint must error");
}
