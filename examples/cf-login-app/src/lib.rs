#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! Cloudflare Workers login app demo.
//!
//! Uses foundation_auth types:
//! - `SessionManager<CredentialStorage>` — session management via `AsyncCredentialStore`
//! - `CredentialStorage` wraps `StorageProvider` backed by `D1WasmStorage`
//!
//! All async handlers call `SessionManager::*_async` methods which delegate
//! to `AsyncCredentialStore::get_async/set_async/list_keys_async` on
//! `CredentialStorage`, which in turn calls `StorageProvider::*_async`,
//! which delegates to `D1WasmStorage::*_async` — direct JS Promise resolution,
//! no valtron stream involved.
//!
//! Raw SQL queries (`query_async`/`execute_async`) also call `D1WasmStorage`
//! async methods directly.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use foundation_auth::{CredentialStorage, SessionConfig, SessionManager};
use foundation_db::{
    core::schema::{MIGRATIONS, MigrationRunner},
    core::storage_provider::DataValue,
    D1WasmStorage, StorageBackend, StorageProvider,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys;

// ===========================================================================
// Signing key — CHANGE THIS to a random 32-byte value in production.
// ===========================================================================

const SIGNING_KEY: &[u8; 32] = b"CHANGE-ME-TO-32-RANDOM-BYTES!!!!";

// ===========================================================================
// Lazy app initialization — runs once on first request (async).
// ===========================================================================

struct LazyApp {
    session_mgr: Arc<SessionManager<CredentialStorage>>,
    storage: Arc<D1WasmStorage>,
}

static LAZY_APP: std::sync::OnceLock<Arc<LazyApp>> = std::sync::OnceLock::new();

async fn get_or_init_app(db: foundation_db::D1Database) -> Result<Arc<LazyApp>, String> {
    if let Some(app) = LAZY_APP.get() {
        return Ok(app.clone());
    }

    let db = Arc::new(db);
    let storage = Arc::new(D1WasmStorage::new(Arc::clone(&db), "app"));

    // Run migrations to create user tables.
    let runner = MigrationRunner::new(MIGRATIONS);
    runner
        .run_async(&*storage)
        .await
        .map_err(|e| format!("migration failed: {e:?}"))?;

    // Initialize KV table for session/credential storage.
    storage
        .init_schema_async()
        .await
        .map_err(|e| format!("kv init failed: {e:?}"))?;

    // CredentialStorage → StorageProvider → D1WasmStorage
    // All SessionManager async calls go through StorageProvider → D1 JS API.
    let provider = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::clone(&db),
        table_prefix: "app".to_string(),
    })
    .map_err(|e| format!("storage provider init failed: {e:?}"))?;
    let cred_storage = CredentialStorage::new(provider);

    let session_mgr = SessionManager::new(cred_storage, Default::default(), SIGNING_KEY)
        .map_err(|e| format!("session manager init failed: {e:?}"))?;

    let app = Arc::new(LazyApp {
        session_mgr: Arc::new(session_mgr),
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

fn extract_session_token_from_cookie(cookie_values: &[String]) -> Option<String> {
    for cookie_str in cookie_values {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{}=", SessionConfig::default().token_cookie_name)) {
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

fn json_response(body: &serde_json::Value, status: u16) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "application/json").unwrap();
    init.set_headers(&headers);
    let text = serde_json::to_string(body).unwrap();
    web_sys::Response::new_with_opt_str_and_init(Some(&text), &init).unwrap()
}

// ===========================================================================
// User CRUD (async — calls D1 JS API directly)
// ===========================================================================

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
    console_log::init_with_level(log::Level::Debug).ok();

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
        ("POST", "/api/note") => handle_save_note(&app, &req).await,
        ("GET", "/api/note") => handle_get_note(&app, &req).await,
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
    log::info!("Login attempt: email={}, body_len={}", email, body.len());

    if email.is_empty() || password.is_empty() {
        return html_response(&render_login(Some("Email and password required")), 400);
    }

    let password_hash = match find_user_password_hash(&app.storage, &email).await {
        Ok(Some(hash)) => {
            log::info!("Found hash: {}", &hash[..30.min(hash.len())]);
            hash
        }
        Ok(None) => {
            log::info!("User not found");
            return html_response(&render_login(Some("Invalid credentials")), 401);
        }
        Err(e) => {
            log::error!("User lookup failed: {:?}", e);
            return html_response(&render_login(Some("Internal server error")), 500);
        }
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(e) => {
            log::info!("PasswordHash::new failed: {:?}", e);
            return html_response(&render_login(Some("Invalid credentials")), 401);
        }
    };

    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        log::info!("Argon2 verify failed for password={}", &password);
        return html_response(&render_login(Some("Invalid credentials")), 401);
    }
    log::info!("Argon2 verify OK");

    // SessionManager async — calls AsyncCredentialStore → StorageProvider → D1WasmStorage async.
    // All JS Promises, no valtron streams.
    let (session, cookies) = match app.session_mgr.create_session_async(
        &email,
        None,
        None,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("Session creation failed: {:?}", e);
            return html_response(&render_login(Some("Internal server error")), 500);
        }
    };

    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", "/dashboard").unwrap();
    for cookie in cookies {
        headers.append("Set-Cookie", &cookie.to_set_cookie_string()).unwrap();
    }
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(
        Some(&format!(r#"{{"status":"ok","session_id":"{}"}}"#, session.id)),
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

    // SessionManager async — calls AsyncCredentialStore methods directly.
    match app.session_mgr.get_session_async(&token).await {
        Ok(Some(session)) => html_response(&render_dashboard(&session.user_id), 200),
        Ok(None) => redirect_response("/login", "Redirecting to login..."),
        Err(e) => {
            log::error!("Session validation failed: {:?}", e);
            redirect_response("/login", "Redirecting to login...")
        }
    }
}

async fn handle_logout(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    if let Some(cookie_values) = cookie_values {
        let token = extract_session_token_from_cookie(&[cookie_values]);
        if let Some(token) = token {
            match app.session_mgr.get_session_async(&token).await {
                Ok(Some(session)) => {
                    let _ = app.session_mgr.revoke_session_async(&session.id).await;
                }
                _ => {}
            }
        }
    }

    let config = SessionConfig::default();
    let init = web_sys::ResponseInit::new();
    init.set_status(302);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Location", "/login").unwrap();
    headers.append("Set-Cookie", &format!("{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax", config.token_cookie_name)).unwrap();
    headers.append("Set-Cookie", &format!("{}=; Path=/; Max-Age=0", config.data_cookie_name)).unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some("Logged out. Redirecting..."), &init).unwrap()
}

// ===========================================================================
// Note handlers — simple KV operations via D1WasmStorage async methods
// ===========================================================================

async fn handle_save_note(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    let Some(cookie_values) = cookie_values else {
        return json_response(&serde_json::json!({"error": "not authenticated"}), 401);
    };

    let token = extract_session_token_from_cookie(&[cookie_values]);
    let Some(token) = token else {
        return json_response(&serde_json::json!({"error": "not authenticated"}), 401);
    };

    let session = match app.session_mgr.get_session_async(&token).await {
        Ok(Some(s)) => s,
        _ => return json_response(&serde_json::json!({"error": "not authenticated"}), 401),
    };

    let body = read_body_text(req).await;
    let note = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        json.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string()
    } else {
        String::new()
    };

    if note.is_empty() {
        return json_response(&serde_json::json!({"error": "note is required"}), 400);
    }

    // Direct D1WasmStorage async — calls D1 JS API via JsFuture.
    let key = format!("note:{}", session.user_id);
    match app.storage.set_async(&key, note.clone()).await {
        Ok(()) => {
            log::info!("Stored note for {} via D1 async", session.user_id);
            json_response(&serde_json::json!({"status": "ok", "note": note}), 200)
        }
        Err(e) => {
            log::error!("Note set failed: {:?}", e);
            json_response(&serde_json::json!({"error": format!("storage failed: {e}")}), 500)
        }
    }
}

async fn handle_get_note(app: &LazyApp, req: &web_sys::Request) -> web_sys::Response {
    let cookie_values = get_cookie_header(req);
    let Some(cookie_values) = cookie_values else {
        return json_response(&serde_json::json!({"error": "not authenticated"}), 401);
    };

    let token = extract_session_token_from_cookie(&[cookie_values]);
    let Some(token) = token else {
        return json_response(&serde_json::json!({"error": "not authenticated"}), 401);
    };

    let session = match app.session_mgr.get_session_async(&token).await {
        Ok(Some(s)) => s,
        _ => return json_response(&serde_json::json!({"error": "not authenticated"}), 401),
    };

    // Direct D1WasmStorage async — calls D1 JS API via JsFuture.
    let key = format!("note:{}", session.user_id);
    match app.storage.get_async::<String>(&key).await {
        Ok(Some(note)) => {
            log::info!("Retrieved note for {} via D1 async", session.user_id);
            json_response(&serde_json::json!({"note": note}), 200)
        }
        Ok(None) => json_response(&serde_json::json!({"note": ""}), 200),
        Err(e) => {
            log::error!("Note get failed: {:?}", e);
            json_response(&serde_json::json!({"error": format!("storage failed: {e}")}), 500)
        }
    }
}
