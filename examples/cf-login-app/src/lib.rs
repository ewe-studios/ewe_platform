#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! Cloudflare Workers login app demo.
//!
//! Demonstrates: D1-backed session management, argon2 password hashing,
//! user registration, and async migrations via `MigrationRunner::run_async`.
//!
//! NOTE: We use a custom `WasmSessionManager` with async D1 operations
//! instead of the native `SessionManager`, because the native one uses
//! sync `CredentialStore` trait methods which deadlock on wasm32 when
//! calling D1 (the JS event loop cannot run while a sync function executes).
//!
//! On native targets, the same handlers work via `HttpApp<Arc<dyn CfServe>>`
//! with the synchronous `QueryStore` trait.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use foundation_db::{
    core::schema::{MIGRATIONS, MigrationRunner},
    core::storage_provider::DataValue,
    D1WasmStorage,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use sha2::Digest;
use web_sys;

// ===========================================================================
// Signing key — CHANGE THIS to a random 32-byte value in production.
// ===========================================================================

const SIGNING_KEY: &[u8; 32] = b"CHANGE-ME-TO-32-RANDOM-BYTES!!!!";

// ===========================================================================
// HMAC-SHA256 (pure Rust, no external crate needed).
// Uses the SHA-256 implementation from the argon2 dependency's crate tree.
// ===========================================================================

/// Compute SHA-256 hash (delegates to sha2 crate via our own minimal impl).
fn sha256(data: &[u8]) -> [u8; 32] {
    // We use the `sha2` crate which is a transitive dep of argon2.
    // If not available, fall back to a simple construction.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// HMAC-SHA256 implementation (RFC 2104).
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hashed = sha256(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    let inner = {
        let mut h = sha2::Sha256::new();
        sha2::Digest::update(&mut h, &ipad);
        sha2::Digest::update(&mut h, message);
        h.finalize()
    };
    let mut h = sha2::Sha256::new();
    sha2::Digest::update(&mut h, &opad);
    sha2::Digest::update(&mut h, &inner);
    h.finalize().into()
}

// ===========================================================================
// Base64 helpers (uses the base64 crate).
// ===========================================================================

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.decode(s).map_err(|e| format!("base64 decode: {e}"))
}

// ===========================================================================
// WasmSessionManager — async session management with signed cookies + D1.
//
// Cookie format: base64(payload).base64(hmac_sha256(payload))
// where payload = JSON { "id": "...", "uid": "...", "exp": unix_ts }
// ===========================================================================

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionPayload {
    id: String,
    uid: String,
    exp: i64,
}

struct WasmSessionManager {
    storage: Arc<D1WasmStorage>,
}

impl WasmSessionManager {
    fn cookie_name() -> &'static str {
        "session"
    }

    fn session_duration_secs() -> i64 {
        86400 // 24 hours
    }

    fn sign_payload(payload_json: &[u8]) -> String {
        let sig = hmac_sha256(SIGNING_KEY, payload_json);
        base64_encode(&sig)
    }

    fn create_token(payload_json: &[u8]) -> String {
        let encoded = base64_encode(payload_json);
        let sig = Self::sign_payload(payload_json);
        format!("{encoded}.{sig}")
    }

    fn verify_token(token: &str) -> Result<SessionPayload, String> {
        let dot = token
            .rfind('.')
            .ok_or_else(|| "invalid token format".to_string())?;
        let encoded = &token[..dot];
        let sig = &token[dot + 1..];

        let payload_bytes = base64_decode(encoded)?;
        let expected_sig = Self::sign_payload(&payload_bytes);

        if sig != expected_sig {
            return Err("invalid signature".to_string());
        }

        let payload: SessionPayload =
            serde_json::from_slice(&payload_bytes).map_err(|e| format!("deserialize: {e}"))?;

        let now = chrono::Utc::now().timestamp();
        if payload.exp < now {
            return Err("session expired".to_string());
        }

        Ok(payload)
    }

    /// Create a new session: store in D1 and return cookie string.
    async fn create_session(&self, user_id: &str) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let exp = chrono::Utc::now().timestamp() + Self::session_duration_secs();
        let now_ms = chrono::Utc::now().timestamp_millis();

        let payload = SessionPayload {
            id: id.clone(),
            uid: user_id.to_string(),
            exp,
        };

        let json = serde_json::to_string(&payload).map_err(|e| format!("serialize: {e}"))?;
        let token = Self::create_token(json.as_bytes());

        // Store session in D1 kv_store table
        let session_json = serde_json::to_string(&payload).map_err(|e| format!("serialize: {e}"))?;
        self.storage
            .execute_async(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, ?)",
                &[
                    DataValue::Text(format!("session:{}", id)),
                    DataValue::Text(session_json),
                    DataValue::Integer(now_ms),
                ],
            )
            .await
            .map_err(|e| format!("D1 insert failed: {e:?}"))?;

        Ok(token)
    }

    /// Validate a session token and return the user_id.
    async fn validate_session(&self, token: &str) -> Result<String, String> {
        let payload = Self::verify_token(token)?;

        // Check server-side session exists
        let rows = self
            .storage
            .query_async(
                "SELECT value FROM kv_store WHERE key = ?",
                &[DataValue::Text(format!("session:{}", payload.id))],
            )
            .await
            .map_err(|e| format!("D1 query failed: {e:?}"))?;

        if rows.is_empty() {
            return Err("session not found".to_string());
        }

        Ok(payload.uid)
    }

    /// Revoke a session.
    async fn revoke_session(&self, token: &str) -> Result<(), String> {
        let payload = match Self::verify_token(token) {
            Ok(p) => p,
            Err(_) => return Ok(()), // Already invalid, no-op
        };

        self.storage
            .execute_async(
                "DELETE FROM kv_store WHERE key = ?",
                &[DataValue::Text(format!("session:{}", payload.id))],
            )
            .await
            .map_err(|e| format!("D1 delete failed: {e:?}"))?;

        Ok(())
    }
}

// ===========================================================================
// Lazy app initialization — runs once on first request (async).
// ===========================================================================

struct LazyApp {
    session_mgr: WasmSessionManager,
    storage: Arc<D1WasmStorage>,
}

static LAZY_APP: std::sync::OnceLock<Arc<LazyApp>> = std::sync::OnceLock::new();

async fn get_or_init_app(db: foundation_db::D1Database) -> Result<Arc<LazyApp>, String> {
    if let Some(app) = LAZY_APP.get() {
        return Ok(app.clone());
    }

    let db = Arc::new(db);
    let storage = Arc::new(D1WasmStorage::new(Arc::clone(&db), "app"));

    // Run migrations to create tables.
    let runner = MigrationRunner::new(MIGRATIONS);
    runner
        .run_async(&*storage)
        .await
        .map_err(|e| format!("migration failed: {e:?}"))?;

    let session_mgr = WasmSessionManager {
        storage: Arc::clone(&storage),
    };

    let app = Arc::new(LazyApp {
        session_mgr,
        storage,
    });

    LAZY_APP
        .set(Arc::clone(&app))
        .map_err(|_| "concurrent init".to_string())?;
    Ok(app)
}

// ===========================================================================
// Helpers
// ===========================================================================

fn redirect_response(location: &str, body: &str) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", location).unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

fn html_response(body: &str, status: u16) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/html; charset=utf-8").unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

fn error_response(msg: &str) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(500);
    web_sys::Response::new_with_opt_str_and_init(Some(msg), &init).unwrap()
}

fn session_cookie(token: &str) -> String {
    let max_age = WasmSessionManager::session_duration_secs();
    format!(
        "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
        WasmSessionManager::cookie_name(),
        token,
        max_age
    )
}

fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax")
}

fn get_session_token_from_cookie(req: &web_sys::Request) -> Option<String> {
    let cookie_header = req.headers().get("Cookie").ok().flatten()?;
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix(&format!("{}=", WasmSessionManager::cookie_name()))
        {
            return Some(value.split(';').next()?.trim().to_string());
        }
    }
    None
}

async fn user_exists_async(storage: &D1WasmStorage, email: &str) -> Result<bool, String> {
    let rows = storage
        .query_async("SELECT 1 FROM users WHERE email = ?", &[DataValue::Text(email.to_string())])
        .await
        .map_err(|e| format!("query failed: {e:?}"))?;
    Ok(!rows.is_empty())
}

async fn create_user_async(
    storage: &D1WasmStorage,
    email: &str,
    password_hash: &str,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    storage
        .execute_async(
            "INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)",
            &[
                DataValue::Text(id.clone()),
                DataValue::Text(email.to_string()),
                DataValue::Text(password_hash.to_string()),
            ],
        )
        .await
        .map_err(|e| format!("insert failed: {e:?}"))?;
    Ok(id)
}

async fn find_user_password_hash(
    storage: &D1WasmStorage,
    email: &str,
) -> Result<Option<String>, String> {
    let rows = storage
        .query_async(
            "SELECT password_hash FROM users WHERE email = ?",
            &[DataValue::Text(email.to_string())],
        )
        .await
        .map_err(|e| format!("query failed: {e:?}"))?;
    for row in &rows {
        if let Ok(hash) = row.get_by_name::<String>("password_hash") {
            return Ok(Some(hash));
        }
    }
    Ok(None)
}

// ===========================================================================
// HTML Templates — embedded via include_str! and rendered with minijinja.
// ===========================================================================

const REGISTER_HTML: &str = include_str!("../templates/register.html");
const LOGIN_HTML: &str = include_str!("../templates/login.html");
const DASHBOARD_HTML: &str = include_str!("../templates/dashboard.html");

fn render_register(error: Option<&str>) -> String {
    let env = minijinja::Environment::new();
    let tmpl = env.template_from_str(REGISTER_HTML).unwrap();
    let ctx = minijinja::context! { error => error.unwrap_or("") };
    tmpl.render(ctx).unwrap()
}

fn render_login(error: Option<&str>) -> String {
    let env = minijinja::Environment::new();
    let tmpl = env.template_from_str(LOGIN_HTML).unwrap();
    let ctx = minijinja::context! { error => error.unwrap_or("") };
    tmpl.render(ctx).unwrap()
}

fn render_dashboard(user: &str) -> String {
    let env = minijinja::Environment::new();
    let tmpl = env.template_from_str(DASHBOARD_HTML).unwrap();
    let ctx = minijinja::context! { user };
    tmpl.render(ctx).unwrap()
}

// ===========================================================================
// Request parsing helpers
// ===========================================================================

fn parse_credentials_from_body(body: &str) -> (String, String) {
    // Try JSON first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        return (
            json.get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            json.get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
    }

    // Fall back to form-encoded
    let mut email = String::new();
    let mut password = String::new();
    for pair in body.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            let decoded = url_decode(v);
            match k {
                "email" => email = decoded,
                "password" => password = decoded,
                _ => {}
            }
        }
    }
    (email, password)
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h = bytes.next().and_then(|c| (c as char).to_digit(16));
            let l = bytes.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h * 16 + l) as u8 as char);
                continue;
            }
            out.push('%');
        } else if b == b'+' {
            out.push(' ');
        } else {
            out.push(b as char);
        }
    }
    out
}

// ===========================================================================
// wasm_bindgen entry point — async fetch handler for CF Workers
// ===========================================================================

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();

    // Extract D1 binding via workers-rs (robust with miniflare).
    let d1_db = match env.d1("DB") {
        Ok(db) => db,
        Err(e) => {
            log::error!("D1 binding error: {:?}", e);
            return error_response("D1 binding error");
        }
    };
    let db: foundation_db::D1Database = d1_db.into();

    // Initialize app (migrations + session manager) on first request.
    let app = match get_or_init_app(db).await {
        Ok(a) => a,
        Err(e) => {
            log::error!("App init failed: {:?}", e);
            return error_response(&format!("App init failed: {e}"));
        }
    };

    // Route based on path and method.
    let path = {
        let u = req.url();
        // web_sys::Request.url() returns full URL, extract path portion
        if let Some(pos) = u.find("//") {
            let rest = &u[pos + 2..];
            if let Some(pos2) = rest.find('/') {
                rest[pos2..].to_string()
            } else {
                "/".to_string()
            }
        } else {
            u
        }
    };

    let method = req.method();

    match (method.as_str(), path.as_str()) {
        ("GET", "/") => handle_home(&app, &req).await,
        ("GET", "/register") => handle_get_register(),
        ("POST", "/register") => handle_post_register(&app, &req).await,
        ("GET", "/login") => handle_get_login(),
        ("POST", "/login") => handle_post_login(&app, &req).await,
        ("GET", "/dashboard") => handle_dashboard(&app, &req).await,
        ("GET", "/logout") => handle_logout(&app, &req).await,
        _ => html_response("Not Found", 404),
    }
}

// ===========================================================================
// Route handlers (async)
// ===========================================================================

async fn handle_home(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let logged_in = if let Some(token) = get_session_token_from_cookie(req) {
        app.session_mgr.validate_session(&token).await.is_ok()
    } else {
        false
    };

    if logged_in {
        redirect_response("/dashboard", "Redirecting to dashboard...")
    } else {
        redirect_response("/register", "Redirecting to registration...")
    }
}

fn handle_get_register() -> web_sys::Response {
    html_response(&render_register(None), 200)
}

async fn handle_post_register(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let body = read_body_text(req).await;
    let (email, password) = parse_credentials_from_body(&body);

    if email.is_empty() || password.is_empty() {
        return html_response(&render_register(Some("Email and password required")), 400);
    }
    if password.len() < 6 {
        return html_response(
            &render_register(Some("Password must be at least 6 characters")),
            400,
        );
    }

    match user_exists_async(&app.storage, &email).await {
        Ok(true) => {
            return html_response(
                &render_register(Some("An account with this email already exists")),
                400,
            );
        }
        Err(e) => {
            log::error!("User check failed: {:?}", e);
            return html_response(&render_register(Some("Internal server error")), 500);
        }
        Ok(false) => {}
    }

    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(e) => {
            log::error!("argon2 hash failed: {:?}", e);
            return html_response(&render_register(Some("Internal server error")), 500);
        }
    };

    match create_user_async(&app.storage, &email, &password_hash).await {
        Ok(_) => redirect_response("/login", "Account created. Redirecting to login..."),
        Err(e) => {
            log::error!("Create user failed: {:?}", e);
            html_response(&render_register(Some("Failed to create account")), 500)
        }
    }
}

fn handle_get_login() -> web_sys::Response {
    html_response(&render_login(None), 200)
}

async fn handle_post_login(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let body = read_body_text(req).await;
    let (email, password) = parse_credentials_from_body(&body);

    if email.is_empty() || password.is_empty() {
        return html_response(&render_login(Some("Email and password required")), 400);
    }

    let password_hash = match find_user_password_hash(&app.storage, &email).await {
        Ok(Some(hash)) => hash,
        Ok(None) => return html_response(&render_login(Some("Invalid credentials")), 401),
        Err(e) => {
            log::error!("User lookup failed: {:?}", e);
            return html_response(&render_login(Some("Internal server error")), 500);
        }
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => return html_response(&render_login(Some("Invalid credentials")), 401),
    };

    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return html_response(&render_login(Some("Invalid credentials")), 401);
    }

    let token = match app.session_mgr.create_session(&email).await {
        Ok(t) => t,
        Err(e) => {
            log::error!("Session creation failed: {:?}", e);
            return html_response(&render_login(Some("Internal server error")), 500);
        }
    };

    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", "/dashboard").unwrap();
    headers.append("Set-Cookie", &session_cookie(&token)).unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(
        Some(r#"{"status":"ok"}"#),
        &init,
    )
    .unwrap()
}

async fn handle_dashboard(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let Some(token) = get_session_token_from_cookie(req) else {
        return redirect_response("/login", "Redirecting to login...");
    };

    let user_id = match app.session_mgr.validate_session(&token).await {
        Ok(uid) => uid,
        Err(e) => {
            log::debug!("Dashboard auth failed: {:?}", e);
            return redirect_response("/login", "Redirecting to login...");
        }
    };

    html_response(&render_dashboard(&user_id), 200)
}

async fn handle_logout(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    if let Some(token) = get_session_token_from_cookie(req) {
        let _ = app.session_mgr.revoke_session(&token).await;
    }

    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", "/login").unwrap();
    headers
        .append("Set-Cookie", &clear_cookie(WasmSessionManager::cookie_name()))
        .unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some("Logged out. Redirecting..."), &init).unwrap()
}

// ===========================================================================
// Body reading helper
// ===========================================================================

async fn read_body_text(req: &web_sys::Request) -> String {
    let text_promise = match req.text() {
        Ok(p) => p,
        Err(_) => return String::new(),
    };
    match JsFuture::from(text_promise).await {
        Ok(v) => v.as_string().unwrap_or_default(),
        Err(_) => String::new(),
    }
}
