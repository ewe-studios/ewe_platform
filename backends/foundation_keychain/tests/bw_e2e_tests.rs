//! Bitwarden API integration tests (spec-57).
//!
//! Exercises the full Bitwarden API lifecycle — server config, registration,
//! prelogin, login, and authenticated CRUD — against the keychain server with
//! a Docker-hosted `bw` CLI for connectivity validation.
//!
//! NOTE: Full `bw login` requires client-side encryption keys that self-hosted
//! servers don't generate. The API is tested via HTTP; `bw config server`
//! validates CLI connectivity. Full CLI CRUD is a follow-up.
//!
//! The `bw` image is built from `Dockerfile.bw_e2e` by the macro (cached by
//! tag), and the container shares the test process's network namespace, so
//! `bw` reaches the in-process server on `localhost` — on a developer machine
//! and inside a CI job container alike. Without a Docker daemon the test logs
//! a skip and returns.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
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

/// Percent-encode a value for an `application/x-www-form-urlencoded` body:
/// everything but the RFC 3986 unreserved characters is escaped.
fn form_encode(value: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            write!(out, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    out
}

#[test]
fn form_encode_escapes_base64_and_email_characters() {
    assert_eq!(form_encode("a+b/c=="), "a%2Bb%2Fc%3D%3D");
    assert_eq!(form_encode("test-1@e2e.local"), "test-1%40e2e.local");
}

fn bw_password_hash(email: &str, password: &str) -> String {
    use base64::Engine; use pbkdf2::pbkdf2_hmac; use sha2::Sha256;
    let e = email.trim().to_lowercase(); let mut mk = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), e.as_bytes(), 600_000, &mut mk);
    let mut h = vec![0u8; 32]; pbkdf2_hmac::<Sha256>(&mk, password.as_bytes(), 1, &mut h);
    base64::engine::general_purpose::STANDARD.encode(&h)
}

// ── Test ─────────────────────────────────────────────────────────────────

#[docker_container(
    image = "foundation-keychain-bw-e2e",
    dockerfile = "Dockerfile.bw_e2e",
    as = "bw",
    share_caller_network = true,
    stop_timeout = 5
)]
#[valtron_test]
fn bw_e2e_full_lifecycle(containers: ContainerGroup) {
    let bw = containers.container("bw").expect("bw container");

    // ── 1. Start server ──────────────────────────────────────────────────
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    // A throwaway file in a temp dir, as the other keychain tests use. Turso
    // reads its URL as a path, so `sqlite::memory:` wrote a file of that name
    // into the crate directory on every run.
    let db_dir = tempfile::tempdir().expect("tempdir");
    let db_path = db_dir.path().join("bw_e2e.db");
    let p = foundation_core::valtron::BlockingFutureExt::block({
        let url = db_path.to_str().expect("utf-8 temp path").to_string();
        async move { StorageProvider::new(StorageBackend::Turso { url }).await }
    })
    .expect("db");
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
    let cfg = bw.exec(&["bw", "config", "server", &url]).expect("bw config");
    assert!(cfg.success(), "bw config (exit {}): {}{}", cfg.exit_code, cfg.stdout, cfg.stderr);

    // ── 4. Register ──────────────────────────────────────────────────────
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    // Pick an account whose password hash contains a `+`, so every run covers
    // the form-encoding case that once failed only for some hashes (about
    // half of all hashes contain one; 64 tries make a miss vanishingly rare,
    // and a miss is reported rather than silently testing the easy case).
    let (email, hash) = (0..64)
        .map(|n| {
            let email = format!("test-{ts}-{n}@e2e.local");
            let hash = bw_password_hash(&email, "correct-horse-battery-staple");
            (email, hash)
        })
        .find(|(_, hash)| hash.contains('+'))
        .expect("one of 64 password hashes should contain a '+'");
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
    // Form values must be percent-encoded: the hash is base64, and an
    // unencoded `+` in it decodes to a space on the server (as the form
    // encoding says it must), failing the login for roughly every other hash.
    let (s, b) = http_req("POST", addr, "/identity/connect/token",
        &format!("grant_type=password&username={}&password={}", form_encode(&email), form_encode(&hash)),
        "application/x-www-form-urlencoded");
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