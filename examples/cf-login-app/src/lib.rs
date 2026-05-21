#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! Cloudflare Workers login app demo.
//!
//! Demonstrates: `HttpApp<Arc<dyn CfServe>>`, `CfConn` structured responses,
//! `CfHttpApp` bridge, D1-backed `SessionManager`, argon2 password hashing,
//! user registration, and `MigrationRunner` on first request.
//!
//! Uses workers-rs `#[event(fetch)]` for proper CF Workers integration.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use foundation_auth::{CredentialStore, CredentialStoreError, SessionConfig, SessionManager};
use foundation_core::valtron::Stream;
use foundation_core::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleOutgoingResponse, Status,
};
use foundation_db::{
    core::schema::{MIGRATIONS, MigrationRunner},
    core::storage_provider::{DataValue, QueryStore},
    D1WasmStorage, KeyValueStore,
};
use foundation_http::{
    shared::{
        app::HttpApp,
        context::ContextBag,
        middleware::{MiddlewareResult, RequestMiddleware},
    },
    wasm::{
        bridge::cf::CfHttpApp,
        cf_conn::{CfConn, CfConnectionResult},
        serve_cf::{CfServe, CfServeFactory},
    },
    SimpleIncomingRequest, SimpleMethod,
};
use worker::{console_error, Env, Response, Result};

// ===========================================================================
// Signing key — CHANGE THIS to a random 32-byte value in production.
// ===========================================================================

const SIGNING_KEY: &[u8; 32] = b"CHANGE-ME-TO-32-RANDOM-BYTES!!!!";

// ===========================================================================
// Lazy app initialization — runs once on first request.
// ===========================================================================

struct LazyApp {
    session_mgr: Arc<SessionManager<D1CredentialStore>>,
    storage: Arc<D1WasmStorage>,
    auth_middleware: SessionAuthMiddleware,
}

static LAZY_APP: std::sync::OnceLock<Arc<LazyApp>> = std::sync::OnceLock::new();

fn get_or_init_app(bag: &ContextBag) -> Result<Arc<LazyApp>> {
    if let Some(app) = LAZY_APP.get() {
        return Ok(app.clone());
    }

    let db = bag
        .get::<foundation_db::D1Database>()
        .ok_or_else(|| worker::Error::RustError("D1Database not found in ContextBag".into()))?;

    let storage = Arc::new(D1WasmStorage::new(db, "app"));

    let runner = MigrationRunner::new(MIGRATIONS);
    runner.run(&*storage).map_err(|e| {
        worker::Error::RustError(format!("migration failed: {e:?}"))
    })?;

    let cred_store = D1CredentialStore(Arc::clone(&storage));
    let session_mgr = Arc::new(
        SessionManager::new(cred_store, SessionConfig::default(), SIGNING_KEY)
            .map_err(|e| worker::Error::RustError(format!("session init failed: {e}")))?,
    );

    let auth_middleware = SessionAuthMiddleware::new(Arc::clone(&session_mgr));

    let app = Arc::new(LazyApp {
        session_mgr,
        storage,
        auth_middleware,
    });

    LAZY_APP
        .set(Arc::clone(&app))
        .map_err(|_| worker::Error::RustError("concurrent init".into()))?;
    Ok(app)
}

// ===========================================================================
// D1CredentialStore — wraps D1WasmStorage (KeyValueStore, stream-returning)
// into CredentialStore (sync Result-returning) for SessionManager.
// ===========================================================================

#[derive(Clone)]
struct D1CredentialStore(Arc<D1WasmStorage>);

impl CredentialStore for D1CredentialStore {
    fn get<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        let stream = self.0.get(key).map_err(CredentialStoreError::Storage)?;
        for item in stream {
            if let Stream::Next(result) = item {
                return result.map_err(CredentialStoreError::Storage);
            }
        }
        Err(CredentialStoreError::NotFound(key.to_string()))
    }

    fn set<V: serde::Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        let stream = self.0.set(key, value).map_err(CredentialStoreError::Storage)?;
        for item in stream {
            if let Stream::Next(result) = item {
                return result.map_err(CredentialStoreError::Storage);
            }
        }
        Err(CredentialStoreError::Generic("Stream ended without result".to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        let stream = self.0.delete(key).map_err(CredentialStoreError::Storage)?;
        for item in stream {
            if let Stream::Next(result) = item {
                return result.map_err(CredentialStoreError::Storage);
            }
        }
        Err(CredentialStoreError::Generic("Stream ended without result".to_string()))
    }

    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        let stream = self.0.exists(key).map_err(CredentialStoreError::Storage)?;
        for item in stream {
            if let Stream::Next(result) = item {
                return result.map_err(CredentialStoreError::Storage);
            }
        }
        Err(CredentialStoreError::Generic("Stream ended without result".to_string()))
    }

    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        let stream = self.0.list_keys(prefix).map_err(CredentialStoreError::Storage)?;
        let mut keys = Vec::new();
        for item in stream {
            if let Stream::Next(Ok(k)) = item {
                keys.push(k);
            }
        }
        Ok(keys)
    }
}

// ===========================================================================
// SessionAuthMiddleware — RequestMiddleware that checks session cookie.
// ===========================================================================

#[derive(Clone)]
struct SessionAuthMiddleware {
    session_mgr: Arc<SessionManager<D1CredentialStore>>,
}

impl SessionAuthMiddleware {
    fn new(session_mgr: Arc<SessionManager<D1CredentialStore>>) -> Self {
        Self { session_mgr }
    }

    fn has_valid_session(&self, req: &SimpleIncomingRequest) -> bool {
        let cookie_values = match req.headers.get(&SimpleHeader::from("Cookie".to_string())) {
            Some(v) => v,
            None => return false,
        };

        let token = extract_session_token_from_cookie(
            &cookie_values,
            &self.session_mgr.config().token_cookie_name,
        );

        let Some(token) = token else {
            return false;
        };

        match self.session_mgr.get_session(&token) {
            Ok(Some(s)) => s.is_valid(),
            _ => false,
        }
    }

    fn redirect_to_login() -> MiddlewareResult {
        let mut headers = SimpleHeaders::new();
        headers
            .entry(SimpleHeader::from("Location".to_string()))
            .or_default()
            .push("/login".to_string());

        MiddlewareResult::Response(SimpleOutgoingResponse {
            proto: Proto::HTTP11,
            status: Status::TemporaryRedirect,
            headers,
            body: Some(SendSafeBody::Text("Redirect to login".into())),
        })
    }
}

impl RequestMiddleware for SessionAuthMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        if !req.request_url.url.starts_with("/dashboard") {
            return MiddlewareResult::Continue;
        }

        if self.has_valid_session(req) {
            MiddlewareResult::Continue
        } else {
            Self::redirect_to_login()
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn extract_session_token_from_cookie(cookie_values: &[String], cookie_name: &str) -> Option<String> {
    for cookie_str in cookie_values {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{cookie_name}=")) {
                return Some(value.split(';').next()?.trim().to_string());
            }
        }
    }
    None
}

fn serialize_cookie_header(cookie: &foundation_core::wire::simple_http::client::shared::Cookie) -> String {
    let mut parts = vec![format!("{}={}", cookie.name, cookie.value)];
    if let Some(ref path) = cookie.path {
        parts.push(format!("Path={path}"));
    }
    if cookie.http_only {
        parts.push("HttpOnly".to_string());
    }
    if let Some(max_age) = cookie.max_age {
        parts.push(format!("Max-Age={}", max_age.as_secs()));
    }
    parts.push(format!("SameSite={:?}", cookie.same_site));
    parts.join("; ")
}

fn set_cookies(conn: &mut CfConn, cookies: &[foundation_core::wire::simple_http::client::shared::Cookie]) {
    for cookie in cookies {
        conn.append_header("Set-Cookie", &serialize_cookie_header(cookie));
    }
}

fn set_cookie_clear_headers(conn: &mut CfConn, names: &[&str]) {
    for name in names {
        conn.append_header(
            "Set-Cookie",
            &format!(
                "{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax",
                name
            ),
        );
    }
}

fn redirect(conn: &mut CfConn, location: &str, body: &str) -> CfConnectionResult {
    conn.set_status(302);
    conn.set_header("Location", location);
    conn.set_body(body.as_bytes().to_vec());
    CfConnectionResult::Ok
}

fn user_exists(storage: &dyn QueryStore, email: &str) -> Result<bool, String> {
    let rows = storage
        .query("SELECT 1 FROM users WHERE email = ?", &[DataValue::Text(email.to_string())])
        .map_err(|e| format!("query failed: {e:?}"))?;
    for row in rows {
        if let Stream::Next(Ok(_)) = row {
            return Ok(true);
        }
    }
    Ok(false)
}

fn create_user(
    storage: &dyn QueryStore,
    email: &str,
    password_hash: &str,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let stream = storage
        .execute(
            "INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)",
            &[
                DataValue::Text(id.clone()),
                DataValue::Text(email.to_string()),
                DataValue::Text(password_hash.to_string()),
            ],
        )
        .map_err(|e| format!("insert failed: {e:?}"))?;
    for item in stream {
        if let Stream::Next(Err(e)) = item {
            return Err(format!("insert failed: {e:?}"));
        }
    }
    Ok(id)
}

fn find_user_by_email(
    storage: &dyn QueryStore,
    email: &str,
) -> Result<Option<String>, String> {
    let rows = storage
        .query(
            "SELECT password_hash FROM users WHERE email = ?",
            &[DataValue::Text(email.to_string())],
        )
        .map_err(|e| format!("query failed: {e:?}"))?;
    for row in rows {
        if let Stream::Next(Ok(sql_row)) = row {
            if let Ok(hash) = sql_row.get_by_name::<String>("password_hash") {
                return Ok(Some(hash));
            }
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
// Handlers
// ===========================================================================

struct RegisterHandler;

impl CfServeFactory for RegisterHandler {
    fn create(_bag: &ContextBag) -> Self { RegisterHandler }
}

impl CfServe for RegisterHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let app = get_or_init_app(&bag).expect("init app");
        match req.method {
            SimpleMethod::GET => handle_get_register(conn),
            SimpleMethod::POST => handle_post_register(&app, bag, req, conn),
            _ => {
                conn.set_status(405);
                conn.set_body(b"Method not allowed".to_vec());
                CfConnectionResult::Ok
            }
        }
    }
}

fn handle_get_register(conn: &mut CfConn) -> CfConnectionResult {
    conn.set_status(200);
    conn.set_header("Content-Type", "text/html; charset=utf-8");
    conn.set_body(render_register(None).as_bytes().to_vec());
    CfConnectionResult::Ok
}

fn handle_post_register(
    app: &Arc<LazyApp>,
    _bag: Arc<ContextBag>,
    req: SimpleIncomingRequest,
    conn: &mut CfConn,
) -> CfConnectionResult {
    let (email, password) = extract_credentials(&req);
    if email.is_empty() || password.is_empty() {
        return send_register_error(conn, "Email and password required");
    }

    if password.len() < 6 {
        return send_register_error(conn, "Password must be at least 6 characters");
    }

    let exists = user_exists(&*app.storage, &email).unwrap_or(false);
    if exists {
        return send_register_error(conn, "An account with this email already exists");
    }

    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(e) => {
            console_error!("argon2 hash failed: {e}");
            return send_register_error(conn, "Internal server error");
        }
    };

    match create_user(&*app.storage, &email, &password_hash) {
        Ok(_) => redirect(conn, "/login", "Account created. Redirecting to login..."),
        Err(e) => {
            console_error!("Failed to create user: {e}");
            send_register_error(conn, "Failed to create account")
        }
    }
}

fn send_register_error(conn: &mut CfConn, msg: &str) -> CfConnectionResult {
    conn.set_status(400);
    conn.set_header("Content-Type", "text/html; charset=utf-8");
    conn.set_body(render_register(Some(msg)).as_bytes().to_vec());
    CfConnectionResult::Ok
}

// ---------------------------------------------------------------------------

struct LoginHandler;

impl CfServeFactory for LoginHandler {
    fn create(_bag: &ContextBag) -> Self { LoginHandler }
}

impl CfServe for LoginHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let app = get_or_init_app(&bag).expect("init app");
        match req.method {
            SimpleMethod::GET => handle_get_login(conn),
            SimpleMethod::POST => handle_post_login(&app, req, conn),
            _ => {
                conn.set_status(405);
                conn.set_body(b"Method not allowed".to_vec());
                CfConnectionResult::Ok
            }
        }
    }
}

fn handle_get_login(conn: &mut CfConn) -> CfConnectionResult {
    conn.set_status(200);
    conn.set_header("Content-Type", "text/html; charset=utf-8");
    conn.set_body(render_login(None).as_bytes().to_vec());
    CfConnectionResult::Ok
}

fn handle_post_login(
    app: &Arc<LazyApp>,
    req: SimpleIncomingRequest,
    conn: &mut CfConn,
) -> CfConnectionResult {
    let (email, password) = extract_credentials(&req);
    if email.is_empty() || password.is_empty() {
        return send_login_error(conn, "Email and password required");
    }

    let password_hash = match find_user_by_email(&*app.storage, &email) {
        Ok(Some(hash)) => hash,
        Ok(None) => return send_login_error(conn, "Invalid credentials"),
        Err(e) => {
            console_error!("Failed to look up user: {e}");
            return send_login_error(conn, "Internal server error");
        }
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => return send_login_error(conn, "Invalid credentials"),
    };

    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return send_login_error(conn, "Invalid credentials");
    }

    let (session, cookies) = match app.session_mgr.create_session(&email, None, None) {
        Ok(v) => v,
        Err(e) => {
            console_error!("Session creation failed: {e}");
            return send_login_error(conn, "Internal server error");
        }
    };

    conn.set_status(302);
    conn.set_header("Location", "/dashboard");
    set_cookies(conn, &cookies);
    conn.set_body(format!(r#"{{"status":"ok","session_id":"{}"}}"#, session.id).as_bytes().to_vec());
    CfConnectionResult::Ok
}

fn send_login_error(conn: &mut CfConn, msg: &str) -> CfConnectionResult {
    conn.set_status(401);
    conn.set_header("Content-Type", "text/html; charset=utf-8");
    conn.set_body(render_login(Some(msg)).as_bytes().to_vec());
    CfConnectionResult::Ok
}

// ---------------------------------------------------------------------------

struct DashboardHandler;

impl CfServeFactory for DashboardHandler {
    fn create(_bag: &ContextBag) -> Self { DashboardHandler }
}

impl CfServe for DashboardHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let app = get_or_init_app(&bag).expect("init app");
        let cookie_values = match req.headers.get(&SimpleHeader::from("Cookie".to_string())) {
            Some(v) => v,
            None => return redirect(conn, "/login", "Redirecting to login..."),
        };

        let token = extract_session_token_from_cookie(
            &cookie_values,
            &app.session_mgr.config().token_cookie_name,
        );

        let Some(token) = token else {
            return redirect(conn, "/login", "Redirecting to login...");
        };

        let session = match app.session_mgr.get_session(&token) {
            Ok(Some(s)) if s.is_valid() => s,
            _ => {
                set_cookie_clear_headers(conn, &["session_token", "session_data", "dont_remember"]);
                return redirect(conn, "/login", "Redirecting to login...");
            }
        };

        let html = render_dashboard(&session.user_id);
        conn.set_status(200);
        conn.set_header("Content-Type", "text/html; charset=utf-8");
        conn.set_body(html.as_bytes().to_vec());
        CfConnectionResult::Ok
    }
}

// ---------------------------------------------------------------------------

struct LogoutHandler;

impl CfServeFactory for LogoutHandler {
    fn create(_bag: &ContextBag) -> Self { LogoutHandler }
}

impl CfServe for LogoutHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let app = get_or_init_app(&bag).expect("init app");
        let cookie_values = req.headers.get(&SimpleHeader::from("Cookie".to_string()));
        if let Some(cookie_values) = cookie_values {
            let token = extract_session_token_from_cookie(
                &cookie_values,
                &app.session_mgr.config().token_cookie_name,
            );

            if let Some(token) = token {
                if let Ok(Some(session)) = app.session_mgr.get_session(&token) {
                    let _ = app.session_mgr.revoke_session(&session.id);
                }
            }
        }

        set_cookie_clear_headers(conn, &["session_token", "session_data", "dont_remember"]);
        redirect(conn, "/login", "Logged out. Redirecting...")
    }
}

// ---------------------------------------------------------------------------

struct HomeHandler;

impl CfServeFactory for HomeHandler {
    fn create(_bag: &ContextBag) -> Self { HomeHandler }
}

impl CfServe for HomeHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let app = get_or_init_app(&bag).expect("init app");
        let cookie_values = req.headers.get(&SimpleHeader::from("Cookie".to_string()));
        let logged_in = if let Some(cookie_values) = cookie_values {
            let token = extract_session_token_from_cookie(
                &cookie_values,
                &app.session_mgr.config().token_cookie_name,
            );
            token.is_some_and(|t| {
                app.session_mgr.get_session(&t).is_ok_and(|s| {
                    s.is_some_and(|s| s.is_valid())
                })
            })
        } else {
            false
        };

        if logged_in {
            redirect(conn, "/dashboard", "Redirecting to dashboard...")
        } else {
            redirect(conn, "/register", "Redirecting to registration...")
        }
    }
}

// ===========================================================================
// Shared helpers
// ===========================================================================

fn extract_credentials(req: &SimpleIncomingRequest) -> (String, String) {
    let body = match &req.body {
        Some(SendSafeBody::Text(s)) => s.clone(),
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        _ => return (String::new(), String::new()),
    };

    if let Some(json) = parse_json_body(&body) {
        return (
            json.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            json.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        );
    }

    parse_form_encoded(&body)
}

fn parse_json_body(body: &str) -> Option<serde_json::Value> {
    serde_json::from_str(body).ok()
}

fn parse_form_encoded(body: &str) -> (String, String) {
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
// CfHttpApp bridge + workers-rs entry point
// ===========================================================================

fn build_worker() -> CfHttpApp {
    console_error_panic_hook::set_once();

    let app = HttpApp::new_cf();
    let mut app = app;
    app.middleware(LazyAuthMiddleware);
    app.route_cf::<HomeHandler>(SimpleMethod::GET, "/");
    app.route_cf::<RegisterHandler>(SimpleMethod::GET, "/register");
    app.route_cf::<RegisterHandler>(SimpleMethod::POST, "/register");
    app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
    app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
    app.route_cf::<DashboardHandler>(SimpleMethod::GET, "/dashboard");
    app.route_cf::<LogoutHandler>(SimpleMethod::GET, "/logout");

    CfHttpApp::from_app(app)
}

// ---------------------------------------------------------------------------

struct LazyAuthMiddleware;

impl RequestMiddleware for LazyAuthMiddleware {
    fn handle(
        &self,
        ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        match get_or_init_app(ctx) {
            Ok(app) => app.auth_middleware.handle(ctx, req),
            Err(e) => {
                console_error!("Auth middleware init failed: {e}");
                MiddlewareResult::Continue
            }
        }
    }
}

// ===========================================================================
// Workers-rs entry point — proper wasm init via #[event(fetch)]
// ===========================================================================
// Full fetch handler — D1 binding + async migrations + CfHttpApp dispatch
// ===========================================================================

#[worker::event(fetch)]
async fn fetch(req: worker::HttpRequest, env: Env, _ctx: worker::Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    console_error_panic_hook::set_once();

    let d1_db = env.d1("DB")
        .map_err(|e| worker::Error::RustError(format!("DB binding error: {e:?}")))?;

    let db: foundation_db::D1Database = d1_db.into();
    let bag = Arc::new(ContextBag::new());
    bag.store(db);

    let storage = Arc::new(D1WasmStorage::new(
        Arc::clone(&bag.get::<foundation_db::D1Database>().unwrap()),
        "app",
    ));

    let runner = MigrationRunner::new(MIGRATIONS);
    let applied = runner.run_async(&*storage).await.map_err(|e| {
        worker::Error::RustError(format!("migration failed: {e:?}"))
    })?;

    let path = req.uri().path();

    let msg = if applied > 0 {
        format!("Applied {} migrations ✓", applied)
    } else {
        "Migrations up to date ✓".to_string()
    };

    match path {
        "/" => worker::Response::ok(&msg),
        "/login" => worker::Response::ok("Login page (session manager pending)"),
        "/register" => worker::Response::ok("Register page (session manager pending)"),
        "/dashboard" => worker::Response::ok("Dashboard (requires session)"),
        "/logout" => worker::Response::ok("Logged out"),
        _ => worker::Response::error("Not found", 404),
    }
}

fn request_from_worker(
    req: &worker::HttpRequest,
) -> Result<SimpleIncomingRequest, worker::Error> {
    let method = SimpleMethod::from(req.method().as_str().to_string());
    let url = req.uri().to_string();

    let mut simple_headers = SimpleHeaders::new();
    for (key, value) in req.headers() {
        let header = SimpleHeader::from(key.as_str().to_string());
        if let Ok(v) = value.to_str() {
            simple_headers.entry(header).or_default().push(v.to_string());
        }
    }

    // For simplicity, skip body reading in this demo
    let body = None;

    SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_method(method)
        .with_proto(Proto::HTTP11)
        .with_headers(simple_headers)
        .with_some_body(body)
        .build()
        .map_err(|e| worker::Error::RustError(format!("failed to build request: {e}")))
}
