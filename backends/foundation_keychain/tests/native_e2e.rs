//! Native server end-to-end test (spec-57, F008 Stage 2).
//!
//! Boots the real `foundation_http` server over the keychain transport and drives
//! register → login (`connect/token`) → authenticated `sync` → folder create →
//! cipher create over actual TCP with `SimpleHttpClient`, proving the whole native
//! stack (routing, auth middleware, JSON, single-poll async handler drive).

use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_netio::http::SimpleHttpClient;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::server::native::KeychainServer;
use foundation_keychain::KeychainContext;
use tempfile::TempDir;

struct TestServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    _dir: TempDir,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    collect_one(execute(from_future(future), None).expect("execute")).expect("result")
}

fn start_server() -> TestServer {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("kc.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    let db: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    let d = Arc::clone(&db);
    drive(async move { apply_schema(d.as_ref()).await }).expect("apply schema");

    let app = KeychainServer::new(KeychainContext::new(db)).http_app();

    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let cfg = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), cfg);
    let sc = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &sc));
    std::thread::sleep(Duration::from_millis(150));

    TestServer { addr, shutdown, _dir: dir }
}

fn client(addr: std::net::SocketAddr) -> SimpleHttpClient<StaticSocketAddr> {
    SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr))
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
        // Send the body with the headers rather than waiting for 100 Continue —
        // exercises the common client path (and the foundation_http server's
        // Expect-continue body read is a separate known gap).
        .expect_continue(false)
}

fn read_body(body: SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(s) => s,
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
        other => String::from_utf8_lossy(&try_collect_bytes(other).unwrap_or_default()).to_string(),
    }
}

#[valtron_test(threads = 8)]
fn register_login_sync_and_create_over_http() {
    let server = start_server();
    let c = client(server.addr);
    let base = format!("http://127.0.0.1:{}", server.addr.port());

    // 1) Register.
    let reg = c
        .post(&format!("{base}/identity/accounts/register"))
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_text(
            r#"{"email":"e2e@example.com","name":"E2E","masterPasswordHash":"mph","key":"2.k|e","kdf":0,"kdfIterations":600000}"#
                .to_string(),
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = reg.into_parts();
    assert_eq!(status.into_usize(), 200, "register: {}", read_body(body));

    // 2) Login via connect/token (form-urlencoded).
    let login = c
        .post(&format!("{base}/identity/connect/token"))
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body_text(
            "grant_type=password&username=e2e@example.com&password=mph&deviceIdentifier=dev-1&deviceName=CLI&deviceType=8"
                .to_string(),
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = login.into_parts();
    let login_body = read_body(body);
    assert_eq!(status.into_usize(), 200, "login: {login_body}");
    let login_json: serde_json::Value = serde_json::from_str(&login_body).expect("login json");
    let access_token = login_json["access_token"].as_str().expect("access_token").to_string();
    assert!(!access_token.is_empty());

    let bearer = format!("Bearer {access_token}");

    // 3) Authenticated sync — profile present, no folders yet.
    let sync1 = c
        .get(&format!("{base}/api/sync"))
        .unwrap()
        .header(SimpleHeader::AUTHORIZATION, bearer.clone())
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = sync1.into_parts();
    let sync_body = read_body(body);
    assert_eq!(status.into_usize(), 200, "sync1: {sync_body}");
    let sync_json: serde_json::Value = serde_json::from_str(&sync_body).unwrap();
    assert_eq!(sync_json["profile"]["email"], "e2e@example.com");
    assert_eq!(sync_json["folders"].as_array().unwrap().len(), 0);

    // 4) Unauthenticated sync is rejected.
    let noauth = c
        .get(&format!("{base}/api/sync"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert_eq!(noauth.into_parts().0.into_usize(), 401);

    // 5) Create a folder.
    let mkfolder = c
        .post(&format!("{base}/api/folders"))
        .unwrap()
        .header(SimpleHeader::AUTHORIZATION, bearer.clone())
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_text(r#"{"name":"2.Personal|enc"}"#.to_string())
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (status, _, body, _, _) = mkfolder.into_parts();
    assert_eq!(status.into_usize(), 200, "folder: {}", read_body(body));

    // 6) Create a cipher.
    let mkcipher = c
        .post(&format!("{base}/api/ciphers"))
        .unwrap()
        .header(SimpleHeader::AUTHORIZATION, bearer.clone())
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_text(
            r#"{"type":1,"name":"2.Login|enc","login":{"username":"2.u|e","password":"2.p|e"}}"#.to_string(),
        )
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert_eq!(mkcipher.into_parts().0.into_usize(), 200);

    // 7) Sync again — folder + cipher now present.
    let sync2 = c
        .get(&format!("{base}/api/sync"))
        .unwrap()
        .header(SimpleHeader::AUTHORIZATION, bearer)
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let (_, _, body, _, _) = sync2.into_parts();
    let sync_json: serde_json::Value = serde_json::from_str(&read_body(body)).unwrap();
    assert_eq!(sync_json["folders"].as_array().unwrap().len(), 1);
    assert_eq!(sync_json["ciphers"].as_array().unwrap().len(), 1);
}
