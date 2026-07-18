//! Native backend transport (spec-57, F008 Stage 2).
//!
//! WHY: wires the portable `core/api` handlers to a real HTTP server for the
//! native (Docker/VPS) deployment. `foundation_http`'s `Serve` trait is
//! synchronous and runs on valtron workers; the keychain handlers are `async`
//! but complete in a single poll over a local Turso database (no real I/O wait),
//! so — like `foundation_auth`'s IdP server — we drive them with a single
//! noop-waker poll rather than a nested executor.
//!
//! WHAT: [`KeychainServer`] builds an [`HttpApp`] routing the Bitwarden API paths
//! to [`ServeAdapter`], which authenticates the request, parses the body, calls
//! the matching handler, and renders a JSON response. Errors map to Bitwarden's
//! shape via [`AppError::to_json`] + [`AppError::status`].
//!
//! HOW: `KeychainServer::new(ctx)` → `http_app()` → `server(addr)` /
//! `serve_with_listener`. Notifications (foundation_netio WS) and cron
//! (foundation_cronjobs) are layered on separately (Stage 2 remainder).

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeError, ServeFactory};
use foundation_http::{RawStream, SimpleIncomingRequest, SimpleMethod};
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleOutgoingResponse, Status,
};
use foundation_errstacks::ErrorTrace;

use crate::core::api::{accounts, ciphers, events, folders, icons, identity, orgs, sends, two_factor};
use crate::core::auth::verify_access_token;
use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::cipher::{CipherCreateRequest, CipherUpdateRequest};
use crate::core::models::folder::{FolderCreateRequest, FolderUpdateRequest};
use crate::core::models::org::{
    CollectionCreateRequest, ConfirmMemberRequest, InviteMemberRequest, OrganizationCreateRequest,
};
use crate::core::models::send::{SendCreateRequest, SendUpdateRequest};
use crate::core::models::user::{RegisterRequest, TokenRequest};

/// A configured native keychain server.
pub struct KeychainServer {
    ctx: KeychainContext,
}

impl KeychainServer {
    /// Build a server over the given context (holds the DB + JWT keys).
    #[must_use]
    pub fn new(ctx: KeychainContext) -> Self {
        Self { ctx }
    }

    /// Build the routed [`HttpApp`].
    #[must_use]
    pub fn http_app(self) -> HttpApp<Arc<dyn Serve>> {
        let mut app = HttpApp::new_serve();
        app.ctx.store(self.ctx);
        register_routes(&mut app);
        app
    }

    /// Build a bound [`HttpServer`](foundation_http::native::server::HttpServer).
    #[must_use]
    pub fn server(self, addr: &str) -> foundation_http::native::server::HttpServer {
        self.http_app().server(addr)
    }
}

fn register_routes(app: &mut HttpApp<Arc<dyn Serve>>) {
    use SimpleMethod::{DELETE, GET, POST, PUT};
    // Identity / accounts (unauthenticated).
    app.route::<ServeAdapter>(POST, "/identity/accounts/prelogin");
    app.route::<ServeAdapter>(POST, "/identity/accounts/register");
    app.route::<ServeAdapter>(POST, "/identity/connect/token");
    // Accounts (authenticated).
    app.route::<ServeAdapter>(GET, "/api/accounts/profile");
    app.route::<ServeAdapter>(PUT, "/api/accounts/profile");
    app.route::<ServeAdapter>(POST, "/api/accounts/verify-password");
    // Sync.
    app.route::<ServeAdapter>(GET, "/api/sync");
    // Folders.
    app.route::<ServeAdapter>(GET, "/api/folders");
    app.route::<ServeAdapter>(POST, "/api/folders");
    app.route_any::<ServeAdapter>("/api/folders/{id}");
    // Ciphers.
    app.route::<ServeAdapter>(GET, "/api/ciphers");
    app.route::<ServeAdapter>(POST, "/api/ciphers");
    app.route::<ServeAdapter>(PUT, "/api/ciphers/{id}/delete");
    app.route::<ServeAdapter>(PUT, "/api/ciphers/{id}/restore");
    app.route_any::<ServeAdapter>("/api/ciphers/{id}");
    // Sends.
    app.route::<ServeAdapter>(GET, "/api/sends");
    app.route::<ServeAdapter>(POST, "/api/sends");
    app.route::<ServeAdapter>(POST, "/api/sends/access/{id}");
    app.route_any::<ServeAdapter>("/api/sends/{id}");
    // Two-factor.
    app.route::<ServeAdapter>(GET, "/api/two-factor/get-authenticator");
    app.route::<ServeAdapter>(POST, "/api/two-factor/authenticator");
    app.route::<ServeAdapter>(DELETE, "/api/two-factor/authenticator");
    // Events + icons.
    app.route::<ServeAdapter>(POST, "/api/events/collect");
    app.route::<ServeAdapter>(GET, "/api/events");
    app.route_any::<ServeAdapter>("/icons/{domain}/icon.png");
    // Organizations.
    app.route::<ServeAdapter>(POST, "/api/organizations");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/collections");
    app.route::<ServeAdapter>(GET, "/api/organizations/{id}/collections");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/users/invite");
    app.route::<ServeAdapter>(GET, "/api/organizations/{id}/users");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/users/{mid}/confirm");
    app.route::<ServeAdapter>(DELETE, "/api/organizations/{id}/collections/{cid}");
    app.route_any::<ServeAdapter>("/api/organizations/{id}");
}

/// The per-connection dispatcher.
pub struct ServeAdapter {
    ctx: KeychainContext,
}

impl ServeFactory for ServeAdapter {
    fn create(bag: &ContextBag) -> Self {
        let ctx = bag
            .get_cloned::<KeychainContext>()
            .expect("KeychainContext must be in ContextBag");
        Self { ctx }
    }
}

impl Serve for ServeAdapter {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let (status, body) = match dispatch(&self.ctx, req) {
            Ok((status, body)) => (status, body),
            Err(err) => (err.status(), err.to_json()),
        };
        match write_json(&mut conn, status, &body) {
            Ok(()) => ConnectionResult::Keep,
            Err(e) => ConnectionResult::Close(Some(e)),
        }
    }
}

// ── Dispatch ────────────────────────────────────────────────────────────────

fn dispatch(ctx: &KeychainContext, req: SimpleIncomingRequest) -> AppResult<(u16, serde_json::Value)> {
    let method = req.method.clone();
    let path = req.request_url.url.split('?').next().unwrap_or("").to_string();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let body = extract_body_text(&req.body);
    let headers = &req.headers;

    // A tiny helper: JSON-parse the request body into `T`.
    let json = |b: &str| -> AppResult<serde_json::Value> {
        if b.trim().is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::from_str(b).map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))
        }
    };
    let de = |v: serde_json::Value| -> AppResult<serde_json::Value> { Ok(v) };
    let _ = de;

    let m = &method;
    let s = segments.as_slice();

    // ── Unauthenticated ─────────────────────────────────────────────────────
    match (m, s) {
        (SimpleMethod::POST, ["identity", "accounts", "prelogin"]) => {
            let email = json(&body)?
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            return ok(block(accounts::prelogin(ctx, &email))?);
        }
        (SimpleMethod::POST, ["identity", "accounts", "register"]) => {
            let req: RegisterRequest = from_json(&body)?;
            return ok(block(accounts::register(ctx, req))?);
        }
        (SimpleMethod::POST, ["identity", "connect", "token"]) => {
            let req = parse_token_request(&body);
            return ok(block(identity::connect_token(ctx, req))?);
        }
        (SimpleMethod::POST, ["api", "sends", "access", id]) => {
            let pw = json(&body)?
                .get("password")
                .and_then(|v| v.as_str())
                .map(String::from);
            return ok(block(sends::access(ctx, id, pw.as_deref()))?);
        }
        (_, ["icons", domain, "icon.png"]) => {
            let icon = block(icons::fetch_icon(domain))?;
            return Ok((200, serde_json::json!({
                "contentType": icon.content_type,
                "length": icon.bytes.len(),
            })));
        }
        _ => {}
    }

    // ── Authenticated ───────────────────────────────────────────────────────
    let user = authenticate(ctx, headers)?;
    let uid = user.as_str();

    match (m, s) {
        (SimpleMethod::GET, ["api", "sync"]) => ok(block(sync_snapshot(ctx, uid))?),
        (SimpleMethod::GET, ["api", "accounts", "profile"]) => ok(block(accounts::get_profile(ctx, uid))?),
        (SimpleMethod::PUT, ["api", "accounts", "profile"]) => {
            ok(block(accounts::update_profile(ctx, uid, from_json(&body)?))?)
        }
        (SimpleMethod::POST, ["api", "accounts", "verify-password"]) => {
            let hash = json(&body)?
                .get("masterPasswordHash")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let valid = block(accounts::verify_master_password(ctx, uid, &hash))?;
            Ok((200, serde_json::json!({ "valid": valid })))
        }

        // Folders
        (SimpleMethod::GET, ["api", "folders"]) => ok(block(folders::list(ctx, uid))?),
        (SimpleMethod::POST, ["api", "folders"]) => {
            let req: FolderCreateRequest = from_json(&body)?;
            ok(block(folders::create(ctx, uid, req))?)
        }
        (SimpleMethod::GET, ["api", "folders", id]) => ok(block(folders::get(ctx, uid, id))?),
        (SimpleMethod::PUT, ["api", "folders", id]) => {
            let req: FolderUpdateRequest = from_json(&body)?;
            ok(block(folders::update(ctx, uid, id, req))?)
        }
        (SimpleMethod::DELETE, ["api", "folders", id]) => {
            block(folders::delete(ctx, uid, id))?;
            Ok((200, serde_json::Value::Null))
        }

        // Ciphers
        (SimpleMethod::GET, ["api", "ciphers"]) => ok(block(ciphers::list(ctx, uid))?),
        (SimpleMethod::POST, ["api", "ciphers"]) => {
            let req: CipherCreateRequest = from_json(&body)?;
            ok(block(ciphers::create(ctx, uid, req))?)
        }
        (SimpleMethod::GET, ["api", "ciphers", id]) => ok(block(ciphers::get(ctx, uid, id))?),
        (SimpleMethod::PUT, ["api", "ciphers", id]) => {
            let req: CipherUpdateRequest = from_json(&body)?;
            ok(block(ciphers::update(ctx, uid, id, req))?)
        }
        (SimpleMethod::DELETE, ["api", "ciphers", id]) => {
            block(ciphers::delete(ctx, uid, id))?;
            Ok((200, serde_json::Value::Null))
        }
        (SimpleMethod::PUT, ["api", "ciphers", id, "delete"]) => ok(block(ciphers::soft_delete(ctx, uid, id))?),
        (SimpleMethod::PUT, ["api", "ciphers", id, "restore"]) => ok(block(ciphers::restore(ctx, uid, id))?),

        // Sends
        (SimpleMethod::GET, ["api", "sends"]) => ok(block(sends::list(ctx, uid))?),
        (SimpleMethod::POST, ["api", "sends"]) => {
            let req: SendCreateRequest = from_json(&body)?;
            ok(block(sends::create(ctx, uid, req))?)
        }
        (SimpleMethod::GET, ["api", "sends", id]) => ok(block(sends::get(ctx, uid, id))?),
        (SimpleMethod::PUT, ["api", "sends", id]) => {
            let req: SendUpdateRequest = from_json(&body)?;
            ok(block(sends::update(ctx, uid, id, req))?)
        }
        (SimpleMethod::DELETE, ["api", "sends", id]) => {
            block(sends::delete(ctx, uid, id))?;
            Ok((200, serde_json::Value::Null))
        }

        // Two-factor
        (SimpleMethod::GET, ["api", "two-factor", "get-authenticator"]) => {
            ok(block(two_factor::get_authenticator(ctx, uid))?)
        }
        (SimpleMethod::POST, ["api", "two-factor", "authenticator"]) => {
            let token = json(&body)?.get("token").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            ok(block(two_factor::enable_authenticator(ctx, uid, &token))?)
        }
        (SimpleMethod::DELETE, ["api", "two-factor", "authenticator"]) => {
            let token = json(&body)?.get("token").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            block(two_factor::disable_authenticator(ctx, uid, &token))?;
            Ok((200, serde_json::Value::Null))
        }

        // Events
        (SimpleMethod::POST, ["api", "events", "collect"]) => {
            let events: Vec<events::EventRequest> = from_json(&body)?;
            block(events::collect(ctx, uid, events))?;
            Ok((200, serde_json::Value::Null))
        }
        (SimpleMethod::GET, ["api", "events"]) => ok(block(events::list(ctx, uid))?),

        // Organizations
        (SimpleMethod::POST, ["api", "organizations"]) => {
            let email = block(accounts::account(ctx, uid))?.email;
            let req: OrganizationCreateRequest = from_json(&body)?;
            ok(block(orgs::create(ctx, uid, &email, req))?)
        }
        (SimpleMethod::GET, ["api", "organizations", id]) => ok(block(orgs::get(ctx, uid, id))?),
        (SimpleMethod::DELETE, ["api", "organizations", id]) => {
            block(orgs::delete(ctx, uid, id))?;
            Ok((200, serde_json::Value::Null))
        }
        (SimpleMethod::GET, ["api", "organizations", id, "collections"]) => {
            ok(block(orgs::list_collections(ctx, uid, id))?)
        }
        (SimpleMethod::POST, ["api", "organizations", id, "collections"]) => {
            let req: CollectionCreateRequest = from_json(&body)?;
            ok(block(orgs::create_collection(ctx, uid, id, req))?)
        }
        (SimpleMethod::DELETE, ["api", "organizations", id, "collections", cid]) => {
            block(orgs::delete_collection(ctx, uid, id, cid))?;
            Ok((200, serde_json::Value::Null))
        }
        (SimpleMethod::GET, ["api", "organizations", id, "users"]) => {
            ok(block(orgs::list_members(ctx, uid, id))?)
        }
        (SimpleMethod::POST, ["api", "organizations", id, "users", "invite"]) => {
            let req: InviteMemberRequest = from_json(&body)?;
            ok(block(orgs::invite_member(ctx, uid, id, req))?)
        }
        (SimpleMethod::POST, ["api", "organizations", id, "users", mid, "confirm"]) => {
            let req: ConfirmMemberRequest = from_json(&body)?;
            block(orgs::confirm_member(ctx, uid, id, mid, req))?;
            Ok((200, serde_json::Value::Null))
        }

        _ => Err(AppError::NotFound(format!("no route for {method:?} {path}"))),
    }
}

/// Wrap a full sync so `dispatch` stays a single `block` call.
async fn sync_snapshot(ctx: &KeychainContext, uid: &str) -> AppResult<crate::core::models::sync::SyncData> {
    crate::core::api::sync::sync(ctx, uid).await
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Serialize `value` and wrap in a `(200, json)`.
fn ok<T: serde::Serialize>(value: T) -> AppResult<(u16, serde_json::Value)> {
    let json = serde_json::to_value(value)
        .map_err(|e| AppError::Internal(format!("serialize response: {e}")))?;
    Ok((200, json))
}

/// Deserialize the request body into `T`.
fn from_json<T: serde::de::DeserializeOwned>(body: &str) -> AppResult<T> {
    serde_json::from_str(body).map_err(|e| AppError::BadRequest(format!("invalid request body: {e}")))
}

/// Drive an `async` handler to completion via a single poll (safe here because
/// the handlers complete synchronously over a local Turso DB).
fn block<T>(fut: impl Future<Output = AppResult<T>>) -> AppResult<T> {
    let mut fut = core::pin::pin!(fut);
    let waker = noop_waker();
    let mut cx = core::task::Context::from_waker(&waker);
    match fut.as_mut().poll(&mut cx) {
        core::task::Poll::Ready(result) => result,
        core::task::Poll::Pending => {
            Err(AppError::Internal("handler did not complete synchronously".into()))
        }
    }
}

fn noop_waker() -> core::task::Waker {
    use core::task::{RawWaker, RawWakerVTable};
    fn no_op(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    unsafe { core::task::Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

/// Extract the `Bearer` token, verify it, and return the authenticated user uuid.
fn authenticate(
    ctx: &KeychainContext,
    headers: &foundation_netio::shared::http::SimpleHeaders,
) -> AppResult<String> {
    let token = headers
        .get(&SimpleHeader::AUTHORIZATION)
        .and_then(|v| v.first())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?
        .trim()
        .to_string();
    let authed = verify_access_token(ctx.verifier(), &token)?;
    Ok(authed.user_uuid)
}

fn extract_body_text(body: &Option<SendSafeBody>) -> String {
    match body {
        Some(SendSafeBody::Text(t)) => t.clone(),
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

/// Parse `connect/token`'s `application/x-www-form-urlencoded` body.
fn parse_token_request(body: &str) -> TokenRequest {
    let map: HashMap<String, String> = body
        .split('&')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let key = it.next()?;
            let val = it.next().unwrap_or("");
            Some((urldecode(key), urldecode(val)))
        })
        .collect();
    TokenRequest {
        grant_type: map.get("grant_type").cloned().unwrap_or_default(),
        username: map.get("username").cloned(),
        master_password_hash: map.get("password").cloned(),
        refresh_token: map.get("refresh_token").cloned(),
        device_identifier: map.get("deviceIdentifier").cloned(),
        device_name: map.get("deviceName").cloned(),
        device_type: map.get("deviceType").and_then(|v| v.parse().ok()),
    }
}

fn urldecode(s: &str) -> String {
    // Minimal application/x-www-form-urlencoded decoding (`+` → space, `%XX`).
    let bytes = s.replace('+', " ");
    let mut out = Vec::with_capacity(bytes.len());
    let raw = bytes.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(byte) = u8::from_str_radix(&bytes[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn status_code(code: u16) -> Status {
    match code {
        200 => Status::OK,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        409 => Status::Conflict,
        429 => Status::TooManyRequests,
        500 => Status::InternalServerError,
        n => Status::Numbered(n as usize, String::new()),
    }
}

fn write_json(
    conn: &mut impl std::io::Write,
    status: u16,
    body: &serde_json::Value,
) -> Result<(), ErrorTrace<ServeError>> {
    let bytes = serde_json::to_vec(body)
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))?;
    let response = SimpleOutgoingResponse::builder()
        .with_status(status_code(status))
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_body(SendSafeBody::Bytes(bytes))
        .build()
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))?;
    Http11::response(response)
        .http_render_to_writer(conn)
        .map(|_| ())
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))
}
