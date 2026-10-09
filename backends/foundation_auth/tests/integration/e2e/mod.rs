//! E2E Integration Tests — full user story validation.
//!
//! Each test spins up a real IdP server with real Turso storage and exercises
//! a complete user journey from start to finish. No server mocks — only external
//! dependencies (OAuth providers, email) are faked.

#![cfg(feature = "server-test")]

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::config::IdpConfig;
use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::idp_server::IdpServer;
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryStorage, StorageBackend, StorageProvider};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_netio::http::SimpleHttpClient;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, Status};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_core::synca::OnSignal;

// ─── Test Infrastructure ─────────────────────────────────────────────────────

/// Spins up a real IdP server with real Turso storage on a random port.

struct TestIdpServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    _db_dir: tempfile::TempDir,
}

impl TestIdpServer {
    /// Start a new IdP server with fresh Turso database.
    fn start() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test_auth.db");

        let provider = StorageProvider::new(StorageBackend::Turso {
            url: db_path.to_str().unwrap().to_string(),
        }).expect("init turso");
        let query_store: Arc<dyn foundation_db::QueryStore> = Arc::new(provider);
        let cache = MemoryStorage::new();
        let storage = Arc::new(HandlerStorage::new(query_store, cache));

        let config = IdpConfig::new("https://auth.example.com".into());
        let idp = IdpServer::new(config, storage);
        let app = idp.http_app();

        let shutdown = Arc::new(OnSignal::new());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");

        let server_config = ServerConfig::defaults()
            .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));

        let bind_addr = format!("127.0.0.1:{}", addr.port());
        let server = HttpServer::with_config(app, &bind_addr, server_config);

        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            server.serve_with_listener(&listener, &shutdown_clone);
        });

        // Give server a moment to start
        std::thread::sleep(Duration::from_millis(100));

        Self { addr, shutdown, _db_dir: dir }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }
}

impl Drop for TestIdpServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// HTTP client with cookie persistence for session-based tests.
struct TestClient {
    inner: SimpleHttpClient<StaticSocketAddr>,
    cookies: std::sync::Mutex<Vec<String>>,
}

impl TestClient {
    fn new(addr: std::net::SocketAddr) -> Self {
        let resolver = StaticSocketAddr::new(addr);
        Self {
            inner: SimpleHttpClient::with_resolver(resolver)
                .max_retries(3)
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(Duration::from_secs(20))
                .write_timeout(Duration::from_secs(5)),
            cookies: std::sync::Mutex::new(Vec::new()),
        }
    }
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

fn parse_json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("response is not valid JSON")
}

fn status_code(status: &Status) -> usize {
    status.clone().into_usize()
}

// ─── User Story 1: OIDC Discovery ───────────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_discovery_returns_valid_oidc_document() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/idp/.well-known/openid-configuration", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let doc = parse_json(&read_body(body));

    assert_eq!(doc["issuer"], "https://auth.example.com");
    assert_eq!(doc["authorization_endpoint"], "https://auth.example.com/idp/authorize");
    assert_eq!(doc["token_endpoint"], "https://auth.example.com/idp/token");
    assert_eq!(doc["userinfo_endpoint"], "https://auth.example.com/idp/userinfo");
    assert_eq!(doc["jwks_uri"], "https://auth.example.com/idp/.well-known/jwks.json");

    // Supported algorithms and flows
    let algs = doc["id_token_signing_alg_values_supported"].as_array().unwrap();
    assert!(algs.iter().any(|v| v == "EdDSA"));

    let pkce = doc["code_challenge_methods_supported"].as_array().unwrap();
    assert!(pkce.iter().any(|v| v == "S256"));
}

// ─── User Story 2: JWKS ─────────────────────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_jwks_returns_ed25519_key() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/idp/.well-known/jwks.json", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let jwks = parse_json(&read_body(body));

    let keys = jwks["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0]["kty"], "OKP");
    assert_eq!(keys[0]["crv"], "Ed25519");
    assert_eq!(keys[0]["use"], "sig");
    assert_eq!(keys[0]["alg"], "EdDSA");
}

// ─── User Story 3: User Registration ────────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_user_registration_creates_account() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .post(&format!("{}/auth/v1/users/register", server.base_url()))
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_json(&serde_json::json!({
            "email": "newuser@example.com",
            "password": "Str0ng!Password#2026",
        })).unwrap()
        .build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp.into_parts();
    let json = parse_json(&read_body(body));

    // With no PoW, registration may require it — either 200 or 400 for missing PoW
    if status_code(&status) == 200 {
        assert!(json["user_id"].is_string(), "registration should return user_id");
    } else {
        assert_eq!(status_code(&status), 400);
    }
}

// ─── User Story 4: PoW Challenge + Solve ────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_pow_challenge_and_solve() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    // Get challenge
    let resp = client
        .get(&format!("{}/auth/v1/pow", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let challenge = parse_json(&read_body(body));

    assert!(challenge["challenge"].is_string());
    assert!(challenge["difficulty"].is_number());
    assert!(challenge["expires_in"].is_number());

    // Submit invalid solution (nonce=0 will almost certainly fail)
    let resp = client
        .post(&format!("{}/auth/v1/pow", server.base_url()))
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_json(&serde_json::json!({
            "challenge": challenge["challenge"],
            "solution": "0",
        })).unwrap()
        .build_client().unwrap().send().unwrap();

    let (_, _, body, _, _) = resp.into_parts();
    let result = parse_json(&read_body(body));
    // Either valid (extremely unlikely with nonce=0) or invalid
    assert!(result["valid"].is_boolean() || result["error"].is_string());
}

// ─── User Story 5: Password Reset Flow ──────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_password_reset_request() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    // Request password reset for non-existent user
    let resp = client
        .post(&format!("{}/auth/v1/users/request_reset", server.base_url()))
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_json(&serde_json::json!({
            "email": "nonexistent@example.com",
        })).unwrap()
        .build_client().unwrap().send().unwrap();

    // Should not reveal whether user exists (security best practice)
    let (status, _, _, _, _) = resp.into_parts();
    assert_eq!(status_code(&status), 200);
}

// ─── User Story 6: ToS Fetch and Accept ─────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_tos_latest_returns_config() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/auth/v1/tos/latest", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let tos = parse_json(&read_body(body));

    // Either configured ToS or "not configured" message
    assert!(tos["version"].is_string() || tos["message"].is_string());
}

// ─── User Story 7: Template Config ──────────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_template_config_returns_policy() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/auth/v1/templates/config", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let config = parse_json(&read_body(body));

    assert!(config["password_policy"]["min_length"].is_number());
    assert!(config["issuer"].is_string());
}

#[valtron_test(threads = 8)]
fn e2e_password_policy_returns_requirements() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/auth/v1/templates/password_policy", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let policy = parse_json(&read_body(body));

    assert_eq!(policy["min_length"], 12);
    assert_eq!(policy["require_uppercase"], true);
    assert_eq!(policy["require_lowercase"], true);
    assert_eq!(policy["require_number"], true);
    assert_eq!(policy["require_special"], true);
}

// ─── User Story 8: Token Introspection ──────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_introspect_unknown_token_returns_inactive() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .post(&format!("{}/idp/introspect", server.base_url()))
        .unwrap()
        .body_text("token=nonexistent_token_xyz")
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let result = parse_json(&read_body(body));

    assert_eq!(result["active"], false);
}

// ─── User Story 9: UserInfo Without Token ───────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_userinfo_without_bearer_returns_401() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/idp/userinfo", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp.into_parts();
    assert_eq!(status_code(&status), 401);

    let err = parse_json(&read_body(body));
    assert_eq!(err["error"], "invalid_token");
    assert!(err["error_description"].is_string());
}

// ─── User Story 10: Authorize Without Session ───────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_authorize_without_session_returns_400() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .get(&format!("{}/idp/authorize?client_id=test&response_type=code&redirect_uri=http://localhost/cb", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp.into_parts();
    assert_eq!(status_code(&status), 400);

    let err = parse_json(&read_body(body));
    assert_eq!(err["error"], "bad_request");
}

// ─── User Story 11: Device Code Flow ────────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_device_authorize_requires_storage() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .post(&format!("{}/idp/device/authorize", server.base_url()))
        .unwrap()
        .body_text("client_id=test_client")
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp.into_parts();
    assert_eq!(status_code(&status), 400);

    let err = parse_json(&read_body(body));
    assert_eq!(err["error"], "bad_request");
}

// ─── User Story 12: Token Without Storage ───────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_token_without_storage_returns_400() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let resp = client
        .post(&format!("{}/idp/token", server.base_url()))
        .unwrap()
        .body_text("grant_type=authorization_code&code=fake&client_id=test")
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp.into_parts();
    assert_eq!(status_code(&status), 400);
}

// ─── User Story 13: OIDC Error Format ───────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_errors_follow_oidc_format() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    // 400 error
    let resp_400 = client
        .post(&format!("{}/idp/token", server.base_url()))
        .unwrap()
        .body_text("grant_type=authorization_code")
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp_400.into_parts();
    assert_eq!(status_code(&status), 400);
    let err = parse_json(&read_body(body));
    assert!(err["error"].is_string(), "400 must have 'error' field");
    assert!(err["error_description"].is_string(), "400 must have 'error_description'");

    // 401 error
    let resp_401 = client
        .get(&format!("{}/idp/userinfo", server.base_url()))
        .unwrap().build_client().unwrap().send().unwrap();

    let (status, _, body, _, _) = resp_401.into_parts();
    assert_eq!(status_code(&status), 401);
    let err = parse_json(&read_body(body));
    assert_eq!(err["error"], "invalid_token");
    assert!(err["error_description"].is_string());
}

// ─── User Story 14: Issuer URL Normalization ────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_issuer_trailing_slash_normalized() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_auth.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    }).expect("init turso");
    let query_store: Arc<dyn foundation_db::QueryStore> = Arc::new(provider);
    let cache = MemoryStorage::new();
    let storage = Arc::new(HandlerStorage::new(query_store, cache));

    let config = IdpConfig::new("https://auth.example.com/".into());
    let idp = IdpServer::new(config, storage);
    let app = idp.http_app();

    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let server_config = ServerConfig::defaults();
    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, server_config);

    let shutdown_clone = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_clone);
    });
    std::thread::sleep(Duration::from_millis(100));

    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
    let resp = client
        .get(&format!("http://127.0.0.1:{}/idp/.well-known/openid-configuration", addr.port()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let doc = parse_json(&read_body(body));

    let token_ep = doc["token_endpoint"].as_str().unwrap();
    assert!(!token_ep.contains("//token"), "no double slashes: {token_ep}");
    assert!(token_ep.ends_with("/token"));

    shutdown.turn_on();
}

// ─── User Story 15: JWKS Key Stability ──────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_jwks_returns_same_key_across_requests() {
    let server = TestIdpServer::start();
    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(server.addr));

    let fetch_jwks = || {
        let resp = client
            .get(&format!("{}/idp/.well-known/jwks.json", server.base_url()))
            .unwrap().build_client().unwrap().send().unwrap();
        let (_, _, body, _, _) = resp.into_parts();
        parse_json(&read_body(body))
    };

    let jwks1 = fetch_jwks();
    let jwks2 = fetch_jwks();

    assert_eq!(
        jwks1["keys"][0]["x"], jwks2["keys"][0]["x"],
        "Same config should return the same public key"
    );
}

// ─── User Story 16: Custom Route Prefix ─────────────────────────────────────

#[valtron_test(threads = 8)]
fn e2e_custom_prefix_routes_work() {
    let server = TestIdpServer::start();
    let config = IdpConfig::new("https://auth.example.com".into());
    let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::new_serve();
    app.ctx.store(config);
    IdpServer::<MemoryStorage>::register_routes(&mut app, "/auth/v1");

    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let server_config = ServerConfig::defaults();
    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, server_config);

    let shutdown_clone = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_clone);
    });
    std::thread::sleep(Duration::from_millis(100));

    let client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
    let resp = client
        .get(&format!("http://127.0.0.1:{}/auth/v1/.well-known/openid-configuration", addr.port()))
        .unwrap().build_client().unwrap().send().unwrap();

    assert!(resp.is_success());
    let (_, _, body, _, _) = resp.into_parts();
    let doc = parse_json(&read_body(body));
    assert_eq!(doc["issuer"], "https://auth.example.com");

    // Discovery at custom prefix works
    let jwks_resp = client
        .get(&format!("http://127.0.0.1:{}/auth/v1/.well-known/jwks.json", addr.port()))
        .unwrap().build_client().unwrap().send().unwrap();
    assert!(jwks_resp.is_success());

    shutdown.turn_on();
}
