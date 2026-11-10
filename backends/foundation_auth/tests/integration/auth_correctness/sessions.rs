//! Sessions live in the store the `IdP` is configured with, shared by all routes.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::{IdpConfig, IdpServer, TotpKeyRing};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryStorage, QueryStore};
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

/// A route that needs a session: 200 with one, 401 without.
const NEEDS_SESSION: &str = "/auth/v1/mfa/totp/enroll/start";

/// A server over an app and a client for it. Dropping it stops the server.
struct Running {
    client: DynNetClient,
    shutdown: Arc<ShutdownSignal>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve(app: HttpApp<Arc<dyn Serve>>) -> Running {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server_config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), server_config);
    let signal = Arc::clone(&shutdown);
    let handle = std::thread::spawn(move || {
        server.serve_with_listener(&listener, &signal);
    });
    let client = HttpClientBuilder::new()
        .resolve_to(addr)
        .max_retries(10)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
        .build();
    Running {
        client,
        shutdown,
        handle: Some(handle),
    }
}

impl Running {
    /// POST JSON; returns the status and the `session_token` cookie pair, if set.
    fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
        cookie: Option<&str>,
    ) -> (usize, Option<String>) {
        let mut builder = PreparedRequestBuilder::post(&format!("http://testserver{path}"))
            .unwrap()
            .body_text(body.to_string())
            .header(SimpleHeader::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            builder = builder.header(SimpleHeader::COOKIE, cookie);
        }
        let (status, headers, body) = self.client.send(builder.build()).unwrap().into_parts();
        match body {
            SendSafeBody::Text(_) | SendSafeBody::Bytes(_) | SendSafeBody::None => {}
            other => {
                let _ = try_collect_bytes(other);
            }
        }
        let token = headers
            .get(&SimpleHeader::SET_COOKIE)
            .and_then(|v| v.iter().find(|c| c.starts_with("session_token=")))
            .and_then(|c| c.split(';').next().map(str::to_string));
        (status.into_usize(), token)
    }

    fn register_and_login(&self, email: &str) -> String {
        let (status, _) = self.post(
            "/auth/v1/dev/register",
            &serde_json::json!({
                "email": email,
                "password": "ValidPass123!",
                "preferred_username": email,
            }),
            None,
        );
        assert_eq!(status, 200, "register");
        let (status, token) = self.post(
            "/auth/v1/oidc/authorize",
            &serde_json::json!({ "email": email, "password": "ValidPass123!" }),
            None,
        );
        assert_eq!(status, 202, "login");
        token.expect("session cookie")
    }
}

fn config() -> IdpConfig {
    IdpConfig::new("https://auth.example.com".into())
        .with_dev_mode(true)
        .with_totp_keys(TotpKeyRing::new("k1", [7u8; 32]))
}

fn test_clock() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

fn memory_storage(ctx: Ctx) -> Arc<HandlerStorage<MemoryStorage>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider =
        crate::integration::turso_provider(dir.path().join("s.db").to_str().unwrap().into());
    std::mem::forget(dir);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    Arc::new(HandlerStorage::new(query_store, MemoryStorage::new(), ctx))
}

#[valtron_test(threads = 8, timeout = 60000)]
fn register_routes_share_one_session_store() {
    // An app built with `register_routes` only: every route has its own
    // handler instance, and no shared session service was put in the app.
    let config = config();
    let storage = memory_storage(test_clock());
    let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::new_serve();
    app.ctx.store(config);
    app.ctx.store(storage);
    IdpServer::<MemoryStorage>::register_routes(&mut app, "/idp");
    IdpServer::<MemoryStorage>::register_dev_routes(&mut app);
    let server = serve(app);

    // The session made by the login route works on another route.
    let cookie = server.register_and_login("shared@example.com");
    let (with_session, _) = server.post(NEEDS_SESSION, &serde_json::json!({}), Some(&cookie));
    let (without, _) = server.post(NEEDS_SESSION, &serde_json::json!({}), None);

    assert_eq!(with_session, 200, "the session is valid on every route");
    assert_eq!(without, 401);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn session_survives_restart_with_a_persistent_store() {
    use foundation_core::valtron::BlockingFutureExt;
    use foundation_db::TursoStorage;

    // One database file holds both the SQL data and the cache (a persistent KV).
    let dir = tempfile::tempdir().expect("tempdir");
    let sql_url = dir.path().join("sql.db").to_str().unwrap().to_string();
    let kv_url = dir.path().join("kv.db").to_str().unwrap().to_string();
    let provider = crate::integration::turso_provider(sql_url);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    let cache = async move {
        let kv = TursoStorage::new(&kv_url).await.unwrap();
        kv.init_schema().await.unwrap();
        kv
    }
    .block();
    let ctx = test_clock();
    let storage = Arc::new(HandlerStorage::new(query_store, cache, ctx.clone()));
    // A stable key, as a deployment reads it from its secret store.
    let make_config = || config().with_session_key(vec![0x5A; 32]);

    // First run: log in, then stop the server.
    let cookie = {
        let app = IdpServer::new(make_config(), Arc::clone(&storage), ctx.clone()).http_app();
        let server = serve(app);
        server.register_and_login("restart@example.com")
    };

    // Second run: a new server over the same store and key accepts the cookie.
    let app = IdpServer::new(make_config(), Arc::clone(&storage), ctx.clone()).http_app();
    let server = serve(app);
    let (status, _) = server.post(NEEDS_SESSION, &serde_json::json!({}), Some(&cookie));
    assert_eq!(status, 200, "the session survived the restart");

    // With another key the cookie is not valid.
    let other_key = IdpConfig::new("https://auth.example.com".into())
        .with_dev_mode(true)
        .with_totp_keys(TotpKeyRing::new("k1", [7u8; 32]))
        .with_session_key(vec![0xA5; 32]);
    let app = IdpServer::new(other_key, Arc::clone(&storage), ctx).http_app();
    let server = serve(app);
    let (status, _) = server.post(NEEDS_SESSION, &serde_json::json!({}), Some(&cookie));
    assert_eq!(
        status, 401,
        "a session signed with another key is not valid"
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn a_short_session_key_is_refused() {
    let ctx = test_clock();
    let storage = memory_storage(ctx.clone());
    let refused = IdpServer::try_new(config().with_session_key(vec![1u8; 8]), storage, ctx);
    assert!(refused.is_err());
}
