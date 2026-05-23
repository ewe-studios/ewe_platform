#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! Cloudflare Workers login app demo.
//!
//! Uses async session management via `WasmSessionManager` to avoid the
//! valtron executor deadlock on miniflare. The sync `SessionManager`
//! routes D1 through `schedule_future` → `drive_non_send_iterator` which
//! blocks the JS event loop, preventing D1 Promises from resolving.
//! `WasmSessionManager` calls `D1WasmStorage`'s `*_async` methods directly
//! which use `JsFuture::from(promise).await` and yield properly.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use foundation_db::{
    core::schema::{MIGRATIONS, MigrationRunner},
    core::storage_provider::DataValue,
    D1WasmStorage, WasmCredentialStore,
};
use foundation_core::valtron::{initialize_pool, PoolGuard};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys;

// ===========================================================================
// Signing key — CHANGE THIS to a random 32-byte value in production.
// ===========================================================================

const SIGNING_KEY: &[u8; 32] = b"CHANGE-ME-TO-32-RANDOM-BYTES!!!!";

// ===========================================================================
// WasmSessionManager — async session management with signed cookies + D1.
//
// Copied from foundation_db/wasm/session.rs (not yet exported for review).
// Uses HMAC-SHA256 signed cookies and stores sessions in kv_store table.
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
    pub const COOKIE_NAME: &'static str = "session";
    pub const DEFAULT_SESSION_DURATION_SECS: i64 = 86_400;

    fn new(storage: Arc<D1WasmStorage>) -> Self {
        Self { storage }
    }

    async fn create_session(&self, user_id: &str) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let exp = chrono::Utc::now().timestamp() + Self::DEFAULT_SESSION_DURATION_SECS;
        let now_ms = chrono::Utc::now().timestamp_millis();

        let payload = SessionPayload { id: id.clone(), uid: user_id.to_string(), exp };
        let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        let token = Self::create_token(json.as_bytes());

        let session_json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        self.storage
            .execute_async(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, ?)",
                &[
                    DataValue::Text(format!("session:{id}")),
                    DataValue::Text(session_json),
                    DataValue::Integer(now_ms),
                ],
            )
            .await
            .map_err(|e| format!("D1 insert failed: {e:?}"))?;

        Ok(token)
    }

    async fn validate_session(&self, token: &str) -> Result<String, String> {
        let payload = Self::verify_token(token)?;
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

    async fn revoke_session(&self, token: &str) -> Result<(), String> {
        let payload = match Self::verify_token(token) {
            Ok(p) => p,
            Err(_) => return Ok(()),
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

    fn session_cookie(token: &str) -> String {
        format!(
            "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
            Self::COOKIE_NAME,
            token,
            Self::DEFAULT_SESSION_DURATION_SECS
        )
    }

    fn clear_cookie() -> String {
        format!("{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax", Self::COOKIE_NAME)
    }

    fn create_token(payload_json: &[u8]) -> String {
        let encoded = base64_encode(payload_json);
        let sig = Self::sign_payload(payload_json);
        format!("{encoded}.{sig}")
    }

    fn sign_payload(payload_json: &[u8]) -> String {
        let sig = hmac_sha256(SIGNING_KEY, payload_json);
        base64_encode(&sig)
    }

    fn verify_token(token: &str) -> Result<SessionPayload, String> {
        let dot = token.rfind('.').ok_or_else(|| "invalid token format".to_string())?;
        let encoded = &token[..dot];
        let sig = &token[dot + 1..];
        let payload_bytes = base64_decode(encoded)?;
        let expected_sig = Self::sign_payload(&payload_bytes);
        if sig != expected_sig {
            return Err("invalid signature".to_string());
        }
        let payload: SessionPayload =
            serde_json::from_slice(&payload_bytes).map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();
        if payload.exp < now {
            return Err("session expired".to_string());
        }
        Ok(payload)
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
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
        let mut h = Sha256::new();
        h.update(&ipad);
        h.update(message);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(&opad);
    h.update(&inner);
    h.finalize().into()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.decode(s).map_err(|e| e.to_string())
}

// ===========================================================================
// Lazy app initialization — runs once on first request (async).
// ===========================================================================

struct LazyApp {
    session_mgr: Arc<WasmSessionManager>,
    storage: Arc<D1WasmStorage>,
    #[allow(dead_code)]
    cred_store: Arc<WasmCredentialStore>,
    _pool_guard: PoolGuard,
}

static LAZY_APP: std::sync::OnceLock<Arc<LazyApp>> = std::sync::OnceLock::new();

async fn get_or_init_app(db: foundation_db::D1Database) -> Result<Arc<LazyApp>, String> {
    if let Some(app) = LAZY_APP.get() {
        return Ok(app.clone());
    }

    // Initialize valtron single executor for wasm32 non-Send task driving.
    let pool_guard = initialize_pool(42, None);

    let db = Arc::new(db);
    let storage = Arc::new(D1WasmStorage::new(Arc::clone(&db), "app"));
    let cred_store = Arc::new(WasmCredentialStore::new(Arc::clone(&storage)));

    // Run migrations to create tables.
    let runner = MigrationRunner::new(MIGRATIONS);
    runner
        .run_async(&*storage)
        .await
        .map_err(|e| format!("migration failed: {e:?}"))?;

    let session_mgr = Arc::new(WasmSessionManager::new(Arc::clone(&storage)));

    let app = Arc::new(LazyApp {
        session_mgr,
        storage,
        cred_store,
        _pool_guard: pool_guard,
    });

    LAZY_APP
        .set(Arc::clone(&app))
        .map_err(|_| "concurrent init".to_string())?;
    Ok(app)
}

// ===========================================================================
// Helpers
// ===========================================================================

fn extract_session_token_from_cookie(cookie_values: &[String]) -> Option<String> {
    for cookie_str in cookie_values {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{}=", WasmSessionManager::COOKIE_NAME)) {
                return Some(value.split(';').next()?.trim().to_string());
            }
        }
    }
    None
}

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
// HTML Templates
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
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        return (
            json.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            json.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        );
    }
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

fn get_cookie_header(req: &web_sys::Request) -> Option<String> {
    req.headers().get("Cookie").ok().flatten()
}

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

// ===========================================================================
// wasm_bindgen entry point
// ===========================================================================

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();

    let d1_db = match env.d1("DB") {
        Ok(db) => db,
        Err(e) => {
            log::error!("D1 binding error: {:?}", e);
            return error_response("D1 binding error");
        }
    };
    let db: foundation_db::D1Database = d1_db.into();

    let app = match get_or_init_app(db).await {
        Ok(a) => a,
        Err(e) => {
            log::error!("App init failed: {:?}", e);
            return error_response(&format!("App init failed: {e}"));
        }
    };

    let path = {
        let u = req.url();
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
        ("GET", "/") => handle_home(&app, &req),
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

fn handle_home(_app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    let logged_in = if let Some(ref cookies) = cookie_values {
        let token = extract_session_token_from_cookie(&[cookies.clone()]);
        token.is_some()
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
    headers
        .append("Set-Cookie", &WasmSessionManager::session_cookie(&token))
        .unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(
        Some(&format!(r#"{{"status":"ok","session_id":"{}"}}"#, token)),
        &init,
    )
    .unwrap()
}

async fn handle_dashboard(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    let Some(cookie_values) = cookie_values else {
        return redirect_response("/login", "Redirecting to login...");
    };

    let token = extract_session_token_from_cookie(&[cookie_values]);
    let Some(token) = token else {
        return redirect_response("/login", "Redirecting to login...");
    };

    match app.session_mgr.validate_session(&token).await {
        Ok(user_id) => html_response(&render_dashboard(&user_id), 200),
        Err(_) => redirect_response("/login", "Redirecting to login..."),
    }
}

async fn handle_logout(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    if let Some(cookie_values) = cookie_values {
        let token = extract_session_token_from_cookie(&[cookie_values]);
        if let Some(token) = token {
            let _ = app.session_mgr.revoke_session(&token).await;
        }
    }

    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", "/login").unwrap();
    headers
        .append("Set-Cookie", &WasmSessionManager::clear_cookie())
        .unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some("Logged out. Redirecting..."), &init).unwrap()
}
