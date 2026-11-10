//! The `ToS` gate at login, and `POST /auth/v1/tos/accept`.
//!
//! Every test starts a real `IdP` on a loopback port and talks to it over HTTP.
//! Every test has a hard time limit (`timeout`, in milliseconds).

#![cfg(feature = "server-test")]

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::models::TosVersion;
use foundation_auth::server::services::{MfaService, TosService};
use foundation_auth::server::storage::{self, HandlerStorage};
use foundation_auth::server::{IdpConfig, IdpServer, TotpKeyRing};
use foundation_auth::shared::auth_session::memory::MemoryAuthSessionStore;
use foundation_auth::TOTPSecret;
use foundation_core::valtron::{valtron_test, BlockingFutureExt, Ctx, MonotonicClock};
use foundation_db::{MemoryStorage, QueryStore};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::ShutdownSignal;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

const PASSWORD: &str = "ValidPass123!";

/// A running `IdP` plus a client and the storage the server uses.
pub(crate) struct Env {
    pub(crate) addr: std::net::SocketAddr,
    pub(crate) client: DynNetClient,
    pub(crate) storage: Arc<HandlerStorage<MemoryStorage>>,
    pub(crate) shutdown: Arc<ShutdownSignal>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Env {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// What the server answered.
pub(crate) struct Reply {
    pub(crate) status: usize,
    pub(crate) headers: SimpleHeaders,
    pub(crate) body: serde_json::Value,
}

impl Reply {
    pub(crate) fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get(&SimpleHeader::SET_COOKIE)
            .cloned()
            .unwrap_or_default()
    }

    /// The `name=value` pair of the session token cookie, ready to send back.
    pub(crate) fn session_cookie(&self) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .find(|c| c.starts_with("session_token="))
            .and_then(|c| c.split(';').next().map(str::to_string))
    }

    pub(crate) fn location(&self) -> Option<String> {
        self.headers
            .get(&SimpleHeader::LOCATION)
            .and_then(|v| v.first().cloned())
    }
}

fn make_storage(ctx: Ctx) -> Arc<HandlerStorage<MemoryStorage>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("tos_gate.db");
    let provider = super::turso_provider(db_path.to_str().unwrap().to_string());
    // The tempdir must outlive the database for the whole test process.
    std::mem::forget(dir);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    Arc::new(HandlerStorage::new(query_store, MemoryStorage::new(), ctx))
}

/// A server with the test TOTP keys.
pub(crate) fn start(config: IdpConfig) -> Env {
    start_raw(config.with_totp_keys(test_keys()))
}

/// A server with exactly this config (dev mode on).
pub(crate) fn start_raw(config: IdpConfig) -> Env {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    let storage = make_storage(ctx.clone());
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");
    let server_config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let idp = IdpServer::new(config.with_dev_mode(true), Arc::clone(&storage), ctx)
        .with_social_login(Arc::new(MemoryAuthSessionStore::new()));
    idp.start_sweeper(Arc::clone(&shutdown))
        .expect("start the cache sweeper");
    let app = idp.http_app();
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), server_config);
    let shutdown_thread = Arc::clone(&shutdown);
    let handle = std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });
    let client = HttpClientBuilder::new()
        .resolve_to(addr)
        .max_retries(10)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
        .build();
    Env {
        addr,
        client,
        storage,
        shutdown,
        handle: Some(handle),
    }
}

/// The TOTP key ring every test server uses.
pub(crate) fn test_keys() -> TotpKeyRing {
    TotpKeyRing::new("k1", [7u8; 32])
}

pub(crate) fn config() -> IdpConfig {
    IdpConfig::new("https://auth.example.com".into())
}

fn read_body(body: SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(s) => s,
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
        body => String::from_utf8_lossy(&try_collect_bytes(body).expect("read body")).to_string(),
    }
}

impl Env {
    pub(crate) fn post(&self, path: &str, body: &serde_json::Value, cookie: Option<&str>) -> Reply {
        let mut builder = PreparedRequestBuilder::post(&format!("http://testserver{path}"))
            .unwrap()
            .body_text(body.to_string())
            .header(SimpleHeader::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            builder = builder.header(SimpleHeader::COOKIE, cookie);
        }
        let (status, headers, body) = self.client.send(builder.build()).unwrap().into_parts();
        let text = read_body(body);
        Reply {
            status: status.into_usize(),
            headers,
            body: serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
        }
    }

    pub(crate) fn get(&self, path_and_query: &str, cookie: Option<&str>) -> Reply {
        let mut builder =
            PreparedRequestBuilder::get(&format!("http://testserver{path_and_query}")).unwrap();
        if let Some(cookie) = cookie {
            builder = builder.header(SimpleHeader::COOKIE, cookie);
        }
        let (status, headers, body) = self.client.send(builder.build()).unwrap().into_parts();
        let text = read_body(body);
        Reply {
            status: status.into_usize(),
            headers,
            body: serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
        }
    }

    /// A raw `GET` over TCP: the status code and the headers, with no
    /// redirect followed (the HTTP client follows them).
    pub(crate) fn raw_get(&self, path_and_query: &str) -> (u16, Vec<(String, String)>) {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(self.addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("timeout");
        write!(
            stream,
            "GET {path_and_query} HTTP/1.1\r\nHost: testserver\r\nConnection: close\r\n\r\n"
        )
        .expect("write");
        let mut text = String::new();
        stream.read_to_string(&mut text).ok();
        let head = text.split("\r\n\r\n").next().unwrap_or("");
        let mut lines = head.lines();
        let status = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|c| c.parse().ok())
            .expect("status line");
        let headers = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        (status, headers)
    }

    pub(crate) fn register(&self, email: &str) -> String {
        let reply = self.post(
            "/auth/v1/dev/register",
            &serde_json::json!({
                "email": email,
                "password": PASSWORD,
                // Each user needs its own username (the column is UNIQUE).
                "preferred_username": email,
            }),
            None,
        );
        assert_eq!(reply.status, 200, "register: {}", reply.body);
        reply.body["id"].as_str().expect("user id").to_string()
    }

    pub(crate) fn login(&self, email: &str) -> Reply {
        self.post(
            "/auth/v1/oidc/authorize",
            &serde_json::json!({ "email": email, "password": PASSWORD }),
            None,
        )
    }

    pub(crate) fn accept_code(
        &self,
        code: &str,
        version: &str,
        extra_user_id: Option<&str>,
    ) -> Reply {
        let mut body = serde_json::json!({ "tos_await_code": code, "tos_version": version });
        if let Some(user_id) = extra_user_id {
            body["user_id"] = user_id.into();
        }
        self.post("/auth/v1/tos/accept", &body, None)
    }

    pub(crate) fn seed_tos(&self, version: &str, effective_from: i64) {
        let service = TosService::new(Arc::clone(&self.storage), self.storage.ctx.clone());
        let version = TosVersion {
            version: version.to_string(),
            content: format!("terms {version}"),
            created_at: effective_from,
            effective_from,
        };
        async move { service.add_version(version).await }
            .block()
            .expect("add ToS version");
    }

    /// Store a TOTP secret for the user; returns a second handle on the secret.
    pub(crate) fn enroll(&self, user_id: &str) -> TOTPSecret {
        let secret = TOTPSecret::generate();
        let copy = TOTPSecret::from_base32(&secret.to_base32()).expect("base32 round trip");
        let service = MfaService::new(
            Arc::clone(&self.storage),
            test_keys(),
            self.storage.ctx.clone(),
        );
        let user_id = user_id.to_string();
        async move { service.enroll_totp(&user_id, &copy).await }
            .block()
            .expect("enroll");
        secret
    }

    pub(crate) fn has_accepted(&self, user_id: &str, version: &str) -> bool {
        let query_store = Arc::clone(&self.storage.query_store);
        let (user_id, version) = (user_id.to_string(), version.to_string());
        async move { storage::find_tos_acceptance(query_store.as_ref(), &user_id, &version).await }
            .block()
            .expect("find acceptance")
            .is_some()
    }
}

fn assert_no_session(reply: &Reply) {
    assert!(reply.set_cookies().is_empty(), "a cookie was set");
    assert!(reply.session_cookie().is_none(), "a session was created");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn login_with_unaccepted_tos_returns_206_and_no_session() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    env.register("gate@example.com");

    let reply = env.login("gate@example.com");

    assert_eq!(reply.status, 206, "body: {}", reply.body);
    assert_eq!(reply.body["status"], "tos_required");
    assert_eq!(reply.body["tos_version"], "v1");
    let code = reply.body["tos_await_code"].as_str().expect("code");
    assert_eq!(code.len(), 43, "256 bits in base64url without padding");
    assert_no_session(&reply);
    assert!(reply.location().is_none());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn tos_accept_ignores_user_id_in_body() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let victim = env.register("victim@example.com");

    let reply = env.post(
        "/auth/v1/tos/accept",
        &serde_json::json!({ "user_id": victim, "tos_version": "v1" }),
        None,
    );

    assert_eq!(reply.status, 401, "body: {}", reply.body);
    assert!(
        !env.has_accepted(&victim, "v1"),
        "victim acceptance recorded"
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn tos_code_completes_the_login() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user = env.register("complete@example.com");

    let gated = env.login("complete@example.com");
    assert_eq!(gated.status, 206, "body: {}", gated.body);
    let code = gated.body["tos_await_code"].as_str().unwrap().to_string();

    let done = env.accept_code(&code, "v1", None);

    assert_eq!(done.status, 202, "body: {}", done.body);
    assert_eq!(done.body["status"], "authenticated");
    assert_eq!(done.body["user_id"], user.as_str());
    assert!(done.session_cookie().is_some(), "no session cookie");
    assert_eq!(done.location().as_deref(), Some("/"));
    assert!(env.has_accepted(&user, "v1"));

    // The next login has no gate.
    let again = env.login("complete@example.com");
    assert_eq!(again.status, 202, "body: {}", again.body);
    assert!(again.session_cookie().is_some());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn tos_code_is_single_use() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    env.register("single@example.com");
    let code = env.login("single@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();

    let first = env.accept_code(&code, "v1", None);
    let second = env.accept_code(&code, "v1", None);

    assert_eq!(first.status, 202, "body: {}", first.body);
    assert_eq!(second.status, 401, "body: {}", second.body);
    assert_eq!(second.body["error"], "invalid_tos_code");
    assert_no_session(&second);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn tos_code_expires() {
    let env = start(config().with_tos_code_ttl(Duration::from_millis(1)));
    env.seed_tos("v1", 1000);
    let user = env.register("expire@example.com");
    let code = env.login("expire@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();
    std::thread::sleep(Duration::from_millis(100));

    let reply = env.accept_code(&code, "v1", None);

    assert_eq!(reply.status, 401, "body: {}", reply.body);
    assert_eq!(reply.body["error"], "invalid_tos_code");
    assert_no_session(&reply);
    assert!(!env.has_accepted(&user, "v1"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn unknown_tos_code_gives_the_same_error_as_used_and_expired() {
    let env = start(config());
    env.seed_tos("v1", 1000);

    let reply = env.accept_code("not-a-real-code", "v1", None);

    assert_eq!(reply.status, 401, "body: {}", reply.body);
    assert_eq!(reply.body["error"], "invalid_tos_code");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn tos_code_is_bound_to_its_user() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user_a = env.register("a@example.com");
    let user_b = env.register("b@example.com");
    let code_a = env.login("a@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();

    // The body names B. The code was issued for A.
    let reply = env.accept_code(&code_a, "v1", Some(&user_b));

    assert_eq!(reply.status, 202, "body: {}", reply.body);
    assert_eq!(reply.body["user_id"], user_a.as_str());
    assert!(env.has_accepted(&user_a, "v1"));
    assert!(!env.has_accepted(&user_b, "v1"), "B was recorded");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn stale_tos_version_is_409() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user = env.register("stale@example.com");
    let code = env.login("stale@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();

    let reply = env.accept_code(&code, "v0", None);

    assert_eq!(reply.status, 409, "body: {}", reply.body);
    assert_eq!(reply.body["error"], "tos_version_mismatch");
    assert_eq!(reply.body["active_version"], "v1");
    assert_no_session(&reply);
    assert!(!env.has_accepted(&user, "v1"));
    assert!(!env.has_accepted(&user, "v0"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn code_for_a_replaced_version_is_409() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user = env.register("replaced@example.com");
    let code = env.login("replaced@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();
    env.seed_tos("v2", 2000);

    let reply = env.accept_code(&code, "v1", None);

    assert_eq!(reply.status, 409, "body: {}", reply.body);
    assert_eq!(reply.body["active_version"], "v2");
    assert!(!env.has_accepted(&user, "v1"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn authenticated_user_can_accept_new_tos_version() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user = env.register("session@example.com");
    let other = env.register("other@example.com");
    let code = env.login("session@example.com").body["tos_await_code"]
        .as_str()
        .unwrap()
        .to_string();
    let done = env.accept_code(&code, "v1", None);
    let cookie = done.session_cookie().expect("session cookie");

    // A new version appears: the next login is gated again.
    env.seed_tos("v2", 2000);
    let gated = env.login("session@example.com");
    assert_eq!(gated.status, 206, "body: {}", gated.body);
    assert_eq!(gated.body["tos_version"], "v2");

    // The signed-in user accepts v2 with the session. The body names another user.
    let reply = env.post(
        "/auth/v1/tos/accept",
        &serde_json::json!({ "tos_version": "v2", "user_id": other }),
        Some(&cookie),
    );

    assert_eq!(reply.status, 200, "body: {}", reply.body);
    assert_eq!(reply.body["status"], "accepted");
    assert!(env.has_accepted(&user, "v2"));
    assert!(!env.has_accepted(&other, "v2"), "the body user_id was used");
    let after = env.login("session@example.com");
    assert_eq!(after.status, 202, "body: {}", after.body);

    // A stale version with a session is 409.
    let stale = env.post(
        "/auth/v1/tos/accept",
        &serde_json::json!({ "tos_version": "v1" }),
        Some(&cookie),
    );
    assert_eq!(stale.status, 409, "body: {}", stale.body);
    assert_eq!(stale.body["active_version"], "v2");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn forged_session_cookie_cannot_accept() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let victim = env.register("forge@example.com");

    let reply = env.post(
        "/auth/v1/tos/accept",
        &serde_json::json!({ "tos_version": "v1", "user_id": victim }),
        Some("session_token=forged-token-value"),
    );

    assert_eq!(reply.status, 401, "body: {}", reply.body);
    assert!(!env.has_accepted(&victim, "v1"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn no_active_tos_means_no_gate() {
    let env = start(config());
    env.register("nogate@example.com");

    let reply = env.login("nogate@example.com");

    assert_eq!(reply.status, 202, "body: {}", reply.body);
    assert_eq!(reply.body["status"], "authenticated");
    assert!(reply.session_cookie().is_some());
    assert_eq!(reply.location().as_deref(), Some("/"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn login_rejects_redirect_uri_without_client() {
    let env = start(config());
    env.register("redir@example.com");

    let reply = env.post(
        "/auth/v1/oidc/authorize",
        &serde_json::json!({
            "email": "redir@example.com",
            "password": PASSWORD,
            "redirect_uri": "https://evil.example/cb",
        }),
        None,
    );

    assert_eq!(reply.status, 400, "body: {}", reply.body);
    assert_no_session(&reply);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn mfa_then_unaccepted_tos_returns_206() {
    let env = start(config());
    env.seed_tos("v1", 1000);
    let user = env.register("mfa@example.com");
    let secret = env.enroll(&user);

    let challenge = env.login("mfa@example.com");
    assert_eq!(challenge.status, 200, "body: {}", challenge.body);
    assert_eq!(challenge.body["status"], "mfa_required");
    assert_no_session(&challenge);
    let challenge_id = challenge.body["challenge_id"].as_str().unwrap().to_string();

    let gated = env.post(
        "/auth/v1/mfa",
        &serde_json::json!({ "challenge_id": challenge_id, "code": secret.now(&env.storage.ctx) }),
        None,
    );

    assert_eq!(gated.status, 206, "body: {}", gated.body);
    assert_eq!(gated.body["status"], "tos_required");
    assert_no_session(&gated);

    let code = gated.body["tos_await_code"].as_str().unwrap().to_string();
    let done = env.accept_code(&code, "v1", None);
    assert_eq!(done.status, 202, "body: {}", done.body);
    assert!(done.session_cookie().is_some());
    assert!(env.has_accepted(&user, "v1"));
}

#[valtron_test(threads = 8, timeout = 60000)]
fn mfa_wrong_code_gives_no_session_and_challenge_is_single_use() {
    let env = start(config());
    let user = env.register("mfabad@example.com");
    let secret = env.enroll(&user);
    let challenge_id = env.login("mfabad@example.com").body["challenge_id"]
        .as_str()
        .unwrap()
        .to_string();

    let wrong = env.post(
        "/auth/v1/mfa",
        &serde_json::json!({ "challenge_id": challenge_id, "code": "000000x" }),
        None,
    );
    assert_eq!(wrong.status, 401, "body: {}", wrong.body);
    assert_no_session(&wrong);

    // The challenge is spent: the right code no longer works with it.
    let retry = env.post(
        "/auth/v1/mfa",
        &serde_json::json!({ "challenge_id": challenge_id, "code": secret.now(&env.storage.ctx) }),
        None,
    );
    assert_eq!(retry.status, 401, "body: {}", retry.body);
    assert_no_session(&retry);

    // An unknown challenge never authenticates (the old handler accepted any).
    let unknown = env.post(
        "/auth/v1/mfa",
        &serde_json::json!({ "challenge_id": "nope", "code": secret.now(&env.storage.ctx) }),
        None,
    );
    assert_eq!(unknown.status, 401, "body: {}", unknown.body);
    assert_no_session(&unknown);
}
