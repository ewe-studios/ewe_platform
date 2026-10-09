//! Bitwarden API integration tests (spec-57).
//!
//! Exercises the full Bitwarden API lifecycle — server config, registration,
//! prelogin, login, and authenticated CRUD — against the keychain server with
//! a Docker-hosted `bw` CLI for connectivity validation.
//!
//! NOTE: Full `bw login` requires client-side encryption keys that self-hosted
//! servers don't generate. The API is tested via HTTP; `bw config server`
//! validates CLI connectivity. Full CLI CRUD is a follow-up.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{block_on_future, valtron_test};
use foundation_deployment_platform::docker::ContainerGroup;
use foundation_deployment_platform::docker_container;
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::server::native::KeychainServer;
use foundation_keychain::KeychainContext;
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};

// ── Helpers ──────────────────────────────────────────────────────────────

fn bw_image_available() -> bool {
    Command::new("docker").args(["images", "-q", "foundation-keychain-bw-e2e"])
        .output().map(|o| !o.stdout.is_empty()).unwrap_or(false)
}

fn read_response(s: &mut TcpStream) -> (u16, String) {
    let mut buf = Vec::new(); let mut c = [0u8; 4096];
    loop { match s.read(&mut c) { Ok(0) => { break; } Ok(n) => { buf.extend_from_slice(&c[..n]); } Err(_) => { break; } } }
    let r = String::from_utf8_lossy(&buf).to_string();
    let st = r.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0);
    (st, r.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

fn http_req(method: &str, addr: SocketAddr, path: &str, body: &str, ct: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).expect("write");
    s.flush().expect("flush");
    read_response(&mut s)
}

fn auth_req(method: &str, addr: SocketAddr, path: &str, body: &str, ct: &str, bearer: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {bearer}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).expect("write");
    s.flush().expect("flush");
    read_response(&mut s)
}

fn bw_password_hash(email: &str, password: &str) -> String {
    use base64::Engine; use pbkdf2::pbkdf2_hmac; use sha2::Sha256;
    let e = email.trim().to_lowercase(); let mut mk = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), e.as_bytes(), 600_000, &mut mk);
    let mut h = vec![0u8; 32]; pbkdf2_hmac::<Sha256>(&mk, password.as_bytes(), 1, &mut h);
    base64::engine::general_purpose::STANDARD.encode(&h)
}

// ── Test ─────────────────────────────────────────────────────────────────

#[docker_container(image = "foundation-keychain-bw-e2e", as = "bw", network = "host")]
#[valtron_test]
fn bw_e2e_full_lifecycle(containers: ContainerGroup) {
    if !bw_image_available() { println!("SKIP — build image first"); return; }
    let bw = containers.container("bw").expect("bw container");
    let cid = bw.id().to_string();

    // ── 1. Start server ──────────────────────────────────────────────────
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let p = StorageProvider::new(StorageBackend::Turso { url: "sqlite::memory:".into() }).expect("db");
    let store: Arc<dyn AsyncQueryStore> = Arc::new(p);
    let sdb = Arc::clone(&store);
    block_on_future(async move { apply_schema(sdb.as_ref()).await }).expect("schema");
    let srv = KeychainServer::new(KeychainContext::new(store)).http_app()
        .server_with_config(&addr.to_string(), foundation_http::native::server::ServerConfig::defaults());
    let sh = Arc::new(foundation_core::synca::OnSignal::new());
    std::thread::spawn(move || { srv.serve_with_listener(&listener, &sh); });
    std::thread::sleep(Duration::from_millis(300));

    // ── 2. Server config endpoints ───────────────────────────────────────
    for p in &["/api", "/api/config"] {
        let (s, b) = http_req("GET", addr, p, "", "application/json");
        assert_eq!(s, 200, "{p}: {b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert!(v.get("version").is_some(), "version missing in {p}");
    }

    // ── 3. `bw config server` validates CLI connectivity ─────────────────
    let url = format!("http://localhost:{}", addr.port());
    let cfg = Command::new("docker")
        .args(["exec", &cid, "bw", "config", "server", &url])
        .output().expect("bw config");
    assert!(cfg.status.success(), "bw config: {}", String::from_utf8_lossy(&cfg.stderr));

    // ── 4. Register ──────────────────────────────────────────────────────
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let email = format!("test-{ts}@e2e.local");
    let hash = bw_password_hash(&email, "correct-horse-battery-staple");
    use base64::Engine;
    let akey = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);

    let (s, b) = http_req("POST", addr, "/identity/accounts/register",
        &serde_json::to_string(&serde_json::json!({
            "name":"E2E","email":&email,"masterPasswordHash":&hash,
            "kdf":0,"kdfIterations":600_000,"key":&akey,
        })).unwrap(), "application/json");
    assert_eq!(s, 200, "register: {b}");

    // ── 5. Prelogin ──────────────────────────────────────────────────────
    let (s, b) = http_req("POST", addr, "/identity/accounts/prelogin",
        &serde_json::to_string(&serde_json::json!({"email":&email})).unwrap(), "application/json");
    assert_eq!(s, 200, "prelogin: {b}");
    let pre: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(pre["kdf"], 0, "prelogin kdf");

    // ── 6. Login (connect/token) ─────────────────────────────────────────
    let (s, b) = http_req("POST", addr, "/identity/connect/token",
        &format!("grant_type=password&username={email}&password={hash}"), "application/x-www-form-urlencoded");
    assert_eq!(s, 200, "login: {b}");
    let tok: serde_json::Value = serde_json::from_str(&b).unwrap();
    let at = tok["access_token"].as_str().expect("access_token").to_string();
    assert!(!at.is_empty());
    assert!(tok.get("AccountKeys").is_some(), "AccountKeys required by bw v2026");
    assert!(tok.get("Key").is_some(), "Key required by bw");

    // ── 7. Authenticated endpoints ───────────────────────────────────────
    // Sync
    let (s, b) = auth_req("GET", addr, "/api/sync", "", "application/json", &at);
    assert_eq!(s, 200, "sync: {b}");
    let sync: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert!(sync.get("profile").is_some(), "sync profile");

    // Folders CRUD
    let (s, b) = auth_req("POST", addr, "/api/folders",
        &serde_json::to_string(&serde_json::json!({"name":"e2e-folder"})).unwrap(), "application/json", &at);
    assert_eq!(s, 200, "create folder: {b}");

    let (s, b) = auth_req("GET", addr, "/api/folders", "", "application/json", &at);
    assert_eq!(s, 200, "list folders: {b}");
    let folders: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(folders.as_array().map(|a| a.len()), Some(1), "1 folder");

    // Ciphers CRUD
    let (s, b) = auth_req("POST", addr, "/api/ciphers",
        &serde_json::to_string(&serde_json::json!({
            "type":1,"name":"example.com",
            "login":{"username":"u","password":"p","uris":[{"uri":"https://example.com","match":null}]},
        })).unwrap(), "application/json", &at);
    assert_eq!(s, 200, "create cipher: {b}");

    let (s, b) = auth_req("GET", addr, "/api/ciphers", "", "application/json", &at);
    assert_eq!(s, 200, "list ciphers: {b}");
    let ciphers: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(ciphers.as_array().map(|a| a.len()), Some(1), "1 cipher");

    // Auth edge cases
    let (s, _) = http_req("GET", addr, "/api/sync", "", "application/json");
    assert_eq!(s, 401, "no auth = 401");
    let (s, _) = auth_req("GET", addr, "/api/sync", "", "application/json", "not.a.real.token");
    assert_eq!(s, 401, "bad auth = 401");

    println!("✓ bw e2e: config → register → prelogin → login → sync → folders → ciphers — all passed");
}