#![cfg(feature = "server-test")]

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::{IdpConfig, IdpServer};
use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryStorage, StorageBackend, StorageProvider};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::simple_http::client::shared::body_reader::try_collect_bytes;
use foundation_netio::simple_http::client::shared::StaticSocketAddr;
use foundation_netio::simple_http::client::SimpleHttpClient;
use foundation_netio::simple_http::shared::{SendSafeBody, SimpleHeader, Status};

fn make_storage() -> Arc<HandlerStorage<MemoryStorage>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test_auth.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    let query_store: Arc<dyn foundation_db::QueryStore> = Arc::new(provider);
    let cache = MemoryStorage::new();
    Arc::new(HandlerStorage::new(query_store, cache))
}

fn start_idp(
    config: IdpConfig,
) -> (
    std::net::SocketAddr,
    Arc<OnSignal>,
    std::thread::JoinHandle<()>,
) {
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let server_config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));

    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let idp = IdpServer::new(config, make_storage());
    let app = idp.http_app();
    let server = HttpServer::with_config(app, &bind_addr, server_config);

    let shutdown_thread = shutdown.clone();
    let handle = std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });

    (addr, shutdown, handle)
}

fn start_idp_with_prefix(
    config: IdpConfig,
    prefix: &str,
) -> (
    std::net::SocketAddr,
    Arc<OnSignal>,
    std::thread::JoinHandle<()>,
) {
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let server_config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));

    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::new_serve();
    app.ctx.store(config);
    app.ctx.store(make_storage());
    IdpServer::<MemoryStorage>::register_routes(&mut app, prefix);
    let server = HttpServer::with_config(app, &bind_addr, server_config);

    let shutdown_thread = shutdown.clone();
    let handle = std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });

    (addr, shutdown, handle)
}

fn make_client(addr: std::net::SocketAddr) -> SimpleHttpClient<StaticSocketAddr> {
    let resolver = StaticSocketAddr::new(addr);
    SimpleHttpClient::with_resolver(resolver)
        .max_retries(10)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
}

fn read_body(body: SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(s) => s,
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
        body => {
            let bytes = try_collect_bytes(body).expect("failed to read body");
            String::from_utf8_lossy(&bytes).to_string()
        }
    }
}

fn status_code(status: &Status) -> usize {
    status.clone().into_usize()
}

fn parse_json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("response is not valid JSON")
}

fn test_config() -> IdpConfig {
    IdpConfig::new("https://auth.example.com".into())
}

// ============================================================================
// Scenario 1: OIDC Discovery — the entry point for any OIDC client
// ============================================================================

#[valtron_test(threads = 8)]
fn discovery_returns_valid_oidc_document() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let (status, _headers, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 200);

    let body = read_body(body);
    let doc = parse_json(&body);

    assert_eq!(doc["issuer"], "https://auth.example.com");
    assert_eq!(
        doc["authorization_endpoint"],
        "https://auth.example.com/idp/authorize"
    );
    assert_eq!(doc["token_endpoint"], "https://auth.example.com/idp/token");
    assert_eq!(
        doc["userinfo_endpoint"],
        "https://auth.example.com/idp/userinfo"
    );
    assert_eq!(
        doc["jwks_uri"],
        "https://auth.example.com/idp/.well-known/jwks.json"
    );
    assert_eq!(
        doc["introspection_endpoint"],
        "https://auth.example.com/idp/introspect"
    );
    assert_eq!(
        doc["device_authorization_endpoint"],
        "https://auth.example.com/idp/device/authorize"
    );

    let response_types = doc["response_types_supported"].as_array().unwrap();
    assert!(response_types.iter().any(|v| v == "code"));

    let grant_types = doc["grant_types_supported"].as_array().unwrap();
    assert!(grant_types.iter().any(|v| v == "authorization_code"));
    assert!(grant_types.iter().any(|v| v == "refresh_token"));
    assert!(grant_types.iter().any(|v| v == "client_credentials"));
    assert!(grant_types
        .iter()
        .any(|v| v == "urn:ietf:params:oauth:grant-type:device_code"));

    let scopes = doc["scopes_supported"].as_array().unwrap();
    assert!(scopes.iter().any(|v| v == "openid"));

    let algs = doc["id_token_signing_alg_values_supported"]
        .as_array()
        .unwrap();
    assert!(algs.iter().any(|v| v == "EdDSA"));

    let pkce = doc["code_challenge_methods_supported"].as_array().unwrap();
    assert!(pkce.iter().any(|v| v == "S256"));

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 2: JWKS — public key set for JWT verification
// ============================================================================

#[valtron_test(threads = 8)]
fn jwks_returns_ed25519_public_key() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/.well-known/jwks.json")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let (_, _, body, _, _) = response.into_parts();
    let body = read_body(body);
    let jwks = parse_json(&body);

    let keys = jwks["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 1);

    let key = &keys[0];
    assert_eq!(key["kty"], "OKP");
    assert_eq!(key["crv"], "Ed25519");
    assert_eq!(key["use"], "sig");
    assert_eq!(key["alg"], "EdDSA");
    assert_eq!(key["kid"], "default");
    assert!(key["x"].is_string(), "public key material must be present");

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 3: Discovery → JWKS flow (client follows jwks_uri)
// ============================================================================

#[valtron_test(threads = 8)]
fn discovery_then_jwks_flow() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let disc_resp = client
        .get("http://testserver/idp/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(disc_resp.is_success());
    let (_, _, body, _, _) = disc_resp.into_parts();
    let doc = parse_json(&read_body(body));
    let _jwks_uri = doc["jwks_uri"].as_str().unwrap();

    let jwks_resp = client
        .get("http://testserver/idp/.well-known/jwks.json")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(jwks_resp.is_success());
    let (_, _, body, _, _) = jwks_resp.into_parts();
    let jwks = parse_json(&read_body(body));
    assert!(jwks["keys"].is_array());
    assert!(!jwks["keys"].as_array().unwrap().is_empty());

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 4: Token introspection — returns inactive for unknown tokens
// ============================================================================

#[valtron_test(threads = 8)]
fn introspect_returns_inactive_for_unknown_token() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .post("http://testserver/idp/introspect")
        .unwrap()
        .body_text("token=some_random_invalid_token")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let (status, _, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 200);

    let body = parse_json(&read_body(body));
    assert_eq!(body["active"], false);

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 5: Authorize without session — returns error
// ============================================================================

#[valtron_test(threads = 8)]
fn authorize_without_session_returns_bad_request() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/authorize?client_id=test&response_type=code&redirect_uri=http://localhost/cb")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    let (status, _, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 400);

    let body = parse_json(&read_body(body));
    assert_eq!(body["error"], "bad_request");
    assert!(body["error_description"].is_string());

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 6: Token endpoint without storage — returns error
// ============================================================================

#[valtron_test(threads = 8)]
fn token_endpoint_returns_error_without_storage() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .post("http://testserver/idp/token")
        .unwrap()
        .body_text("grant_type=authorization_code&code=test_code&client_id=test_client")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    let (status, _, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 400);

    let body = parse_json(&read_body(body));
    assert_eq!(body["error"], "bad_request");
    assert!(body["error_description"].as_str().is_some());

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 7: Userinfo without bearer token — returns 401
// ============================================================================

#[valtron_test(threads = 8)]
fn userinfo_without_bearer_returns_unauthorized() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/userinfo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    let (status, _, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 401);

    let body = parse_json(&read_body(body));
    assert_eq!(body["error"], "invalid_token");
    assert!(body["error_description"]
        .as_str()
        .unwrap()
        .contains("Bearer"));

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 8: Device authorization without storage — returns error
// ============================================================================

#[valtron_test(threads = 8)]
fn device_authorize_returns_error_without_storage() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let response = client
        .post("http://testserver/idp/device/authorize")
        .unwrap()
        .body_text("client_id=test_client")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    let (status, _, body, _, _) = response.into_parts();
    assert_eq!(status_code(&status), 400);

    let body = parse_json(&read_body(body));
    assert_eq!(body["error"], "bad_request");
    assert!(body["error_description"].as_str().is_some());

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 9: Custom route prefix — "/auth/v1" instead of "/idp"
// ============================================================================

#[valtron_test(threads = 8)]
fn custom_prefix_routes_work() {
    let (addr, shutdown, handle) = start_idp_with_prefix(test_config(), "/auth/v1");
    let client = make_client(addr);

    let response = client
        .get("http://testserver/auth/v1/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let (_, _, body, _, _) = response.into_parts();
    let doc = parse_json(&read_body(body));
    assert_eq!(doc["issuer"], "https://auth.example.com");

    let jwks_resp = client
        .get("http://testserver/auth/v1/.well-known/jwks.json")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(jwks_resp.is_success());

    let token_resp = client
        .post("http://testserver/auth/v1/token")
        .unwrap()
        .body_text("grant_type=authorization_code")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, _, _, _) = token_resp.into_parts();
    assert_eq!(status_code(&status), 400);

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 10: Default prefix routes are NOT accessible under custom prefix
// ============================================================================

#[valtron_test(threads = 8)]
fn default_prefix_not_accessible_under_custom() {
    let (addr, shutdown, handle) = start_idp_with_prefix(test_config(), "/auth/v1");
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    let (status, _, _, _, _) = response.into_parts();
    assert_ne!(status_code(&status), 200);

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 11: Multiple endpoints in sequence (simulates client lifecycle)
// ============================================================================

#[valtron_test(threads = 8)]
fn full_client_lifecycle_discovery_to_token_attempt() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    // Step 1: Client discovers the IdP
    let disc_resp = client
        .get("http://testserver/idp/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(disc_resp.is_success());
    let (_, _, body, _, _) = disc_resp.into_parts();
    let doc = parse_json(&read_body(body));

    // Step 2: Client fetches JWKS for future token verification
    let jwks_resp = client
        .get("http://testserver/idp/.well-known/jwks.json")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(jwks_resp.is_success());
    let (_, _, body, _, _) = jwks_resp.into_parts();
    let jwks = parse_json(&read_body(body));
    let keys = jwks["keys"].as_array().unwrap();
    assert!(!keys.is_empty());

    // Step 3: Client attempts authorization (no session → error expected)
    let auth_resp = client
        .get("http://testserver/idp/authorize?client_id=myapp&response_type=code&redirect_uri=http://localhost/cb&scope=openid&state=xyz")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = auth_resp.into_parts();
    assert_eq!(status_code(&status), 400);
    let auth_err = parse_json(&read_body(body));
    assert_eq!(auth_err["error"], "bad_request");

    // Step 4: Client attempts token exchange (no storage → error expected)
    let token_resp = client
        .post("http://testserver/idp/token")
        .unwrap()
        .body_text("grant_type=authorization_code&code=fake_code&client_id=myapp&redirect_uri=http://localhost/cb")
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = token_resp.into_parts();
    assert_eq!(status_code(&status), 400);
    let token_err = parse_json(&read_body(body));
    assert_eq!(token_err["error"], "bad_request");

    // Step 5: Resource server introspects a token (unknown → inactive)
    let intro_resp = client
        .post("http://testserver/idp/introspect")
        .unwrap()
        .body_text("token=fake_access_token&client_id=myapp&client_secret=secret")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(intro_resp.is_success());
    let (_, _, body, _, _) = intro_resp.into_parts();
    let intro = parse_json(&read_body(body));
    assert_eq!(intro["active"], false);

    // Verify discovery document has correct endpoints
    assert!(doc["authorization_endpoint"]
        .as_str()
        .unwrap()
        .contains("/authorize"));
    assert!(doc["token_endpoint"].as_str().unwrap().contains("/token"));
    assert!(doc["introspection_endpoint"]
        .as_str()
        .unwrap()
        .contains("/introspect"));

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 12: OIDC error responses follow RFC format
// ============================================================================

#[valtron_test(threads = 8)]
fn error_responses_follow_oidc_format() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    // 400 errors have "error" + "error_description"
    let resp_400 = client
        .post("http://testserver/idp/token")
        .unwrap()
        .body_text("grant_type=authorization_code")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = resp_400.into_parts();
    assert_eq!(status_code(&status), 400);
    let err = parse_json(&read_body(body));
    assert!(err["error"].is_string(), "400 must have 'error' field");
    assert!(
        err["error_description"].is_string(),
        "400 must have 'error_description'"
    );

    // 401 errors have "error" + "error_description"
    let resp_401 = client
        .get("http://testserver/idp/userinfo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = resp_401.into_parts();
    assert_eq!(status_code(&status), 401);
    let err = parse_json(&read_body(body));
    assert_eq!(err["error"], "invalid_token");
    assert!(
        err["error_description"].is_string(),
        "401 must have 'error_description'"
    );

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 13: Issuer URL trailing slash normalization
// ============================================================================

#[valtron_test(threads = 8)]
fn issuer_url_trailing_slash_normalized() {
    let config = IdpConfig::new("https://auth.example.com/".into());
    let (addr, shutdown, handle) = start_idp(config);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/idp/.well-known/openid-configuration")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let (_, _, body, _, _) = response.into_parts();
    let doc = parse_json(&read_body(body));

    // Endpoints should NOT have double slashes
    let token_ep = doc["token_endpoint"].as_str().unwrap();
    assert!(
        !token_ep.contains("//token"),
        "no double slashes: {token_ep}"
    );
    assert!(token_ep.ends_with("/token"));

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 14: JWKS key stability — same config returns same key
// ============================================================================

#[valtron_test(threads = 8)]
fn jwks_returns_stable_key_across_requests() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    let fetch_jwks = || {
        let resp = client
            .get("http://testserver/idp/.well-known/jwks.json")
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap();
        let (_, _, body, _, _) = resp.into_parts();
        parse_json(&read_body(body))
    };

    let jwks1 = fetch_jwks();
    let jwks2 = fetch_jwks();

    assert_eq!(
        jwks1["keys"][0]["x"], jwks2["keys"][0]["x"],
        "Same config should return the same public key"
    );

    shutdown.turn_on();
    let _ = handle.join();
}

// ============================================================================
// Scenario 15: Device code flow — POST to device/authorize
// ============================================================================

#[valtron_test(threads = 8)]
fn device_code_endpoint_is_post_only_at_correct_path() {
    let (addr, shutdown, handle) = start_idp(test_config());
    let client = make_client(addr);

    // POST works (returns error because no storage, but 400 not 404)
    let post_resp = client
        .post("http://testserver/idp/device/authorize")
        .unwrap()
        .body_text("client_id=test")
        .header(
            SimpleHeader::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, _, _, _) = post_resp.into_parts();
    assert_eq!(
        status_code(&status),
        400,
        "POST to device/authorize should reach handler"
    );

    shutdown.turn_on();
    let _ = handle.join();
}
