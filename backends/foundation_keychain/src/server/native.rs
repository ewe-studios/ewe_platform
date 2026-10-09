//! Native backend transport (spec-57, F008 Stage 2).
//!
//! WHY: wires the portable [`core::router::route`](crate::core::router::route) to
//! a real HTTP server for the native (Docker/VPS) deployment. `foundation_http`'s
//! `Serve` trait is synchronous and runs on valtron workers; the keychain handlers
//! are `async` but complete in a single poll over a local Turso database (no real
//! I/O wait), so — like `foundation_auth`'s IdP server — we drive them with a
//! single noop-waker poll rather than a nested executor.
//!
//! WHAT: [`KeychainServer`] builds an [`HttpApp`] routing the Bitwarden API paths
//! to [`ServeAdapter`], which extracts (method, path, body, bearer), single-polls
//! the shared router, and renders a JSON response.

use std::future::Future;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_errstacks::ErrorTrace;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeError, ServeFactory};
use foundation_http::{RawStream, SimpleIncomingRequest, SimpleMethod};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleOutgoingResponse, Status,
};

use crate::core::context::KeychainContext;
use crate::core::error::AppError;
use crate::core::router::route;

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
    app.route_any::<ServeAdapter>("/api");
    app.route_any::<ServeAdapter>("/api/config");
    app.route::<ServeAdapter>(POST, "/identity/accounts/prelogin");
    app.route::<ServeAdapter>(POST, "/identity/accounts/register");
    app.route::<ServeAdapter>(POST, "/identity/connect/token");
    // bw v2026 uses /prelogin/password for master password prelogin (BWSDK path).
    app.route::<ServeAdapter>(POST, "/identity/accounts/prelogin/password");
    app.route::<ServeAdapter>(GET, "/api/accounts/profile");
    app.route::<ServeAdapter>(PUT, "/api/accounts/profile");
    app.route::<ServeAdapter>(POST, "/api/accounts/verify-password");
    app.route::<ServeAdapter>(GET, "/api/sync");
    app.route::<ServeAdapter>(GET, "/api/folders");
    app.route::<ServeAdapter>(POST, "/api/folders");
    app.route_any::<ServeAdapter>("/api/folders/{id}");
    app.route::<ServeAdapter>(GET, "/api/ciphers");
    app.route::<ServeAdapter>(POST, "/api/ciphers");
    app.route::<ServeAdapter>(PUT, "/api/ciphers/{id}/delete");
    app.route::<ServeAdapter>(PUT, "/api/ciphers/{id}/restore");
    app.route_any::<ServeAdapter>("/api/ciphers/{id}");
    app.route::<ServeAdapter>(GET, "/api/sends");
    app.route::<ServeAdapter>(POST, "/api/sends");
    app.route::<ServeAdapter>(POST, "/api/sends/access/{id}");
    app.route_any::<ServeAdapter>("/api/sends/{id}");
    app.route::<ServeAdapter>(GET, "/api/two-factor/get-authenticator");
    app.route::<ServeAdapter>(POST, "/api/two-factor/authenticator");
    app.route::<ServeAdapter>(DELETE, "/api/two-factor/authenticator");
    app.route::<ServeAdapter>(POST, "/api/events/collect");
    app.route::<ServeAdapter>(GET, "/api/events");
    app.route_any::<ServeAdapter>("/icons/{domain}/icon.png");
    app.route::<ServeAdapter>(POST, "/api/organizations");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/collections");
    app.route::<ServeAdapter>(GET, "/api/organizations/{id}/collections");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/users/invite");
    app.route::<ServeAdapter>(GET, "/api/organizations/{id}/users");
    app.route::<ServeAdapter>(POST, "/api/organizations/{id}/users/{mid}/confirm");
    app.route::<ServeAdapter>(DELETE, "/api/organizations/{id}/collections/{cid}");
    app.route_any::<ServeAdapter>("/api/organizations/{id}");
    // Feature 009 — app registry + SSH key provisioning.
    app.route::<ServeAdapter>(POST, "/api/apps/register");
    app.route_any::<ServeAdapter>("/api/apps/{id}");
    app.route::<ServeAdapter>(GET, "/api/credentials/ssh-keys");
    app.route::<ServeAdapter>(POST, "/api/credentials/ssh-keys");
    app.route_any::<ServeAdapter>("/api/credentials/ssh-keys/{id}");
}

/// Serialize a handler result into `(200, json)`.
fn ser_ok<T: serde::Serialize>(r: Result<T, AppError>) -> Result<(u16, serde_json::Value), AppError> {
    let v = r?;
    Ok((200, serde_json::to_value(v).map_err(|e| AppError::Internal(e.to_string()))?))
}

/// Handle the feature-009 provisioning routes (app registry + SSH keys). Returns
/// `None` when the path is not a provisioning route (fall through to the vault
/// router). App-scoped routes authenticate the `X-App-Secret` / `Bearer` secret.
async fn provisioning_route(
    ctx: &KeychainContext,
    master_key: &str,
    method: &str,
    segments: &[&str],
    body: &str,
    app_secret: Option<&str>,
) -> Option<Result<(u16, serde_json::Value), AppError>> {
    use crate::core::provisioning::{apps, ssh_keys};

    // App registration is unauthenticated.
    if method == "POST" && segments == ["api", "apps", "register"] {
        let req = match serde_json::from_str(body) {
            Ok(r) => r,
            Err(e) => return Some(Err(AppError::BadRequest(format!("invalid body: {e}")))),
        };
        return Some(ser_ok(apps::register(ctx, req).await));
    }

    let is_provisioning = matches!(segments, ["api", "apps", ..] | ["api", "credentials", "ssh-keys", ..]);
    if !is_provisioning {
        return None;
    }

    // Everything else needs a valid app secret.
    let secret = match app_secret {
        Some(s) => s,
        None => return Some(Err(AppError::Unauthorized)),
    };
    let app_id = match apps::authenticate(ctx, secret).await {
        Ok(id) => id,
        Err(e) => return Some(Err(e)),
    };
    let app_id = app_id.as_str();

    let result = match (method, segments) {
        ("GET", ["api", "apps", id]) => ser_ok(apps::get(ctx, app_id, id).await),
        ("DELETE", ["api", "apps", id]) => {
            match apps::delete(ctx, app_id, id).await {
                Ok(()) => Ok((200, serde_json::Value::Null)),
                Err(e) => Err(e),
            }
        }
        ("GET", ["api", "credentials", "ssh-keys"]) => ser_ok(ssh_keys::list(ctx, app_id).await),
        ("POST", ["api", "credentials", "ssh-keys"]) => {
            match serde_json::from_str(body) {
                Ok(req) => ser_ok(ssh_keys::create(ctx, master_key, app_id, req).await),
                Err(e) => Err(AppError::BadRequest(format!("invalid body: {e}"))),
            }
        }
        ("GET", ["api", "credentials", "ssh-keys", id]) => {
            ser_ok(ssh_keys::get(ctx, master_key, app_id, id).await)
        }
        ("DELETE", ["api", "credentials", "ssh-keys", id]) => {
            match ssh_keys::delete(ctx, app_id, id).await {
                Ok(()) => Ok((200, serde_json::Value::Null)),
                Err(e) => Err(e),
            }
        }
        _ => Err(AppError::NotFound(format!("no provisioning route for {method}"))),
    };
    Some(result)
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
        mut req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let method = format!("{:?}", req.method);
        let path = req.request_url.url.split('?').next().unwrap_or("").to_string();
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        let body = read_body(req.body.take());
        let bearer = req
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|v| v.first())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|t| t.trim().to_string());
        let master_key = std::env::var("KEYCHAIN_MASTER_KEY")
            .unwrap_or_else(|_| "insecure-dev-master-key".to_string());

        // Feature-009 provisioning routes take precedence; else the vault router.
        let outcome = match poll_ready(provisioning_route(
            &self.ctx, &master_key, &method, &segments, &body, bearer.as_deref(),
        )) {
            Some(result) => result,
            None => poll_ready(route(&self.ctx, &method, &path, &body, bearer.as_deref())),
        };
        let (status, json) = match outcome {
            Ok((status, json)) => (status, json),
            Err(err) => (err.status(), err.to_json()),
        };

        match write_json(&mut conn, status, &json) {
            Ok(()) => ConnectionResult::Keep,
            Err(e) => ConnectionResult::Close(Some(e)),
        }
    }
}

/// Drain a request body to a string, collecting the lazy stream variant.
fn read_body(body: Option<SendSafeBody>) -> String {
    match body {
        None | Some(SendSafeBody::None) => String::new(),
        Some(SendSafeBody::Text(t)) => t,
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(&b).to_string(),
        Some(other) => {
            let bytes = try_collect_bytes(other).unwrap_or_default();
            String::from_utf8_lossy(&bytes).to_string()
        }
    }
}

/// Drive an `async` handler via a single poll and return its output — safe
/// because the handlers finish synchronously over a local Turso DB (verified).
fn poll_ready<O>(fut: impl Future<Output = O>) -> O {
    let mut fut = core::pin::pin!(fut);
    let waker = noop_waker();
    let mut cx = core::task::Context::from_waker(&waker);
    match fut.as_mut().poll(&mut cx) {
        core::task::Poll::Ready(output) => output,
        core::task::Poll::Pending => {
            panic!("keychain handler did not complete in a single poll (unexpected async I/O)")
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
