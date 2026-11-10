//! Native backend transport (spec-57, F008 Stage 2).
//!
//! WHY: wires the portable [`core::router::route`](crate::core::router::route) to
//! a real HTTP server for the native (Docker/VPS) deployment. The keychain
//! handlers are `async` and may wait on the store, so they must run on the
//! executor with a real waker, never be polled once by hand.
//!
//! WHAT: [`KeychainServer`] builds an [`HttpApp`] routing the Bitwarden API paths
//! to [`ServeAdapter`], which extracts (method, path, body, bearer) and returns
//! [`ConnectionResult::Async`]. The future owns that data and a clone of the
//! context, runs the router, and writes the JSON response with the shared
//! non-blocking response writer.

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::writer::{render_head, write_response, ResponseWriter, WriterConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_http::{RawStream, SimpleIncomingRequest, SimpleMethod};
use foundation_netio::shared::client::body_reader::{collect_bytes_bounded, AsyncSendSafeBody};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::router::route;

/// Default for [`KeychainServerConfig::max_request_body`]: 1 MiB, the default of
/// nginx's `client_max_body_size`.
pub const DEFAULT_MAX_REQUEST_BODY: usize = 1024 * 1024;

/// Limits for the native keychain server.
#[derive(Debug, Clone, Copy)]
pub struct KeychainServerConfig {
    /// Largest request body, in bytes, the server reads. The handlers parse
    /// whole JSON documents, so the body is collected up to this size. A larger
    /// body is answered with `413 Payload Too Large` and the connection is
    /// closed without the rest of the body being read. Default 1 MiB, nginx's
    /// `client_max_body_size` default.
    pub max_request_body: usize,
}

impl Default for KeychainServerConfig {
    fn default() -> Self {
        Self {
            max_request_body: DEFAULT_MAX_REQUEST_BODY,
        }
    }
}

impl KeychainServerConfig {
    /// A configuration with the documented defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the largest request body the server reads (see
    /// [`max_request_body`](Self::max_request_body)).
    #[must_use]
    pub fn with_max_request_body(mut self, bytes: usize) -> Self {
        self.max_request_body = bytes;
        self
    }
}

/// A configured native keychain server.
pub struct KeychainServer {
    ctx: KeychainContext,
    config: KeychainServerConfig,
}

impl KeychainServer {
    /// Build a server over the given context (holds the DB + JWT keys).
    #[must_use]
    pub fn new(ctx: KeychainContext) -> Self {
        Self {
            ctx,
            config: KeychainServerConfig::default(),
        }
    }

    /// Replace the server limits.
    #[must_use]
    pub fn with_config(mut self, config: KeychainServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the largest request body the server reads (default 1 MiB).
    #[must_use]
    pub fn with_max_request_body(mut self, bytes: usize) -> Self {
        self.config.max_request_body = bytes;
        self
    }

    /// Build the routed [`HttpApp`].
    #[must_use]
    pub fn http_app(self) -> HttpApp<Arc<dyn Serve>> {
        let mut app = HttpApp::new_serve();
        app.ctx.store(self.ctx);
        app.ctx.store(self.config);
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
fn ser_ok<T: serde::Serialize>(r: AppResult<T>) -> AppResult<(u16, serde_json::Value)> {
    let v = r?;
    Ok((
        200,
        serde_json::to_value(v).map_err(|e| AppError::Internal(e.to_string()))?,
    ))
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
) -> Option<AppResult<(u16, serde_json::Value)>> {
    use crate::core::provisioning::{apps, ssh_keys};

    // App registration is unauthenticated.
    if method == "POST" && segments == ["api", "apps", "register"] {
        let req = match serde_json::from_str(body) {
            Ok(r) => r,
            Err(e) => {
                return Some(Err(
                    AppError::BadRequest(format!("invalid body: {e}")).into()
                ))
            }
        };
        return Some(ser_ok(apps::register(ctx, req).await));
    }

    let is_provisioning = matches!(
        segments,
        ["api", "apps", ..] | ["api", "credentials", "ssh-keys", ..]
    );
    if !is_provisioning {
        return None;
    }

    // Everything else needs a valid app secret.
    let Some(secret) = app_secret else {
        return Some(Err(AppError::Unauthorized.into()));
    };
    let app_id = match apps::authenticate(ctx, secret).await {
        Ok(id) => id,
        Err(e) => return Some(Err(e)),
    };
    let app_id = app_id.as_str();

    let result = match (method, segments) {
        ("GET", ["api", "apps", id]) => ser_ok(apps::get(ctx, app_id, id).await),
        ("DELETE", ["api", "apps", id]) => match apps::delete(ctx, app_id, id).await {
            Ok(()) => Ok((200, serde_json::Value::Null)),
            Err(e) => Err(e),
        },
        ("GET", ["api", "credentials", "ssh-keys"]) => ser_ok(ssh_keys::list(ctx, app_id).await),
        ("POST", ["api", "credentials", "ssh-keys"]) => match serde_json::from_str(body) {
            Ok(req) => ser_ok(ssh_keys::create(ctx, master_key, app_id, req).await),
            Err(e) => Err(AppError::BadRequest(format!("invalid body: {e}")).into()),
        },
        ("GET", ["api", "credentials", "ssh-keys", id]) => {
            ser_ok(ssh_keys::get(ctx, master_key, app_id, id).await)
        }
        ("DELETE", ["api", "credentials", "ssh-keys", id]) => {
            match ssh_keys::delete(ctx, app_id, id).await {
                Ok(()) => Ok((200, serde_json::Value::Null)),
                Err(e) => Err(e),
            }
        }
        _ => Err(AppError::NotFound(format!("no provisioning route for {method}")).into()),
    };
    Some(result)
}

/// The per-connection dispatcher.
pub struct ServeAdapter {
    ctx: KeychainContext,
    config: KeychainServerConfig,
}

impl ServeFactory for ServeAdapter {
    fn create(app_ctx: &Ctx) -> Self {
        let ctx = app_ctx
            .get_cloned::<KeychainContext>()
            .expect("KeychainContext must be in the Ctx");
        let config = app_ctx
            .get_cloned::<KeychainServerConfig>()
            .unwrap_or_default();
        Self { ctx, config }
    }
}

impl Serve for ServeAdapter {
    fn serve(
        &self,
        _ctx: Ctx,
        mut req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // Take everything the handlers need out of the request now: the future
        // returned below is `'static` and cannot borrow `self` or `req`. The
        // body is not read here: it is read inside the future.
        let method = format!("{:?}", req.method);
        let path = req
            .request_url
            .url
            .split('?')
            .next()
            .unwrap_or("")
            .to_string();
        let body = req.body.take();
        let bearer = req
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|v| v.first())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|t| t.trim().to_string());
        let master_key = std::env::var("KEYCHAIN_MASTER_KEY")
            .unwrap_or_else(|_| "insecure-dev-master-key".to_string());
        // `KeychainContext` holds only `Arc`s, so this clone is cheap.
        let ctx = self.ctx.clone();
        let limit = self.config.max_request_body;

        ConnectionResult::Async(Box::pin(async move {
            let read_wake = conn.with_inner_ref(RawStream::read_wake_source);
            let body = match read_body(body, read_wake, limit).await {
                Ok(body) => body,
                Err(BodyError::TooLarge) => {
                    // The rest of the body is unread, so the connection cannot
                    // carry another request: answer, then close.
                    let json = serde_json::json!({
                        "error": "payload_too_large",
                        "error_description": "The request body is too large",
                        "ErrorModel": {
                            "Message": "The request body is too large",
                            "Object": "error"
                        }
                    });
                    let _ = write_json(conn, 413, &json, true).await;
                    return ServeOutcome::Close(None);
                }
                Err(BodyError::NotUtf8) => {
                    let err = AppError::BadRequest("request body is not valid UTF-8".into());
                    return write_json(conn, err.status(), &err.to_json(), false).await;
                }
            };
            let (status, json) =
                handle(&ctx, &master_key, &method, &path, &body, bearer.as_deref()).await;
            write_json(conn, status, &json, false).await
        }))
    }
}

/// Run the handlers for one request and return the status and JSON body.
async fn handle(
    ctx: &KeychainContext,
    master_key: &str,
    method: &str,
    path: &str,
    body: &str,
    bearer: Option<&str>,
) -> (u16, serde_json::Value) {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    // Feature-009 provisioning routes take precedence; else the vault router.
    let outcome = match provisioning_route(ctx, master_key, method, &segments, body, bearer).await {
        Some(result) => result,
        None => route(ctx, method, path, body, bearer).await,
    };
    match outcome {
        Ok((status, json)) => (status, json),
        Err(err) => (
            err.current_context().status(),
            err.current_context().to_json(),
        ),
    }
}

/// Why a request body could not be read.
enum BodyError {
    /// The body is larger than the configured limit.
    TooLarge,
    /// The body is not valid UTF-8.
    NotUtf8,
}

/// Read a request body as an async stream, at most `limit` bytes.
///
/// One byte more than `limit` is read to tell "exactly `limit`" from "too
/// large"; the rest of the body is never read. `read_wake` lets the stream park
/// until the socket is readable instead of spinning.
async fn read_body(
    body: Option<SendSafeBody>,
    read_wake: Option<foundation_netio::shared::client::body_reader::ReadWakeSource>,
    limit: usize,
) -> Result<String, BodyError> {
    let body = match body {
        None | Some(SendSafeBody::None) => return Ok(String::new()),
        Some(body) => body,
    };
    let mut stream = AsyncSendSafeBody::from(body);
    if let Some(source) = read_wake {
        stream = stream.with_read_wake(source);
    }
    let bytes = collect_bytes_bounded(stream, limit.saturating_add(1)).await;
    if bytes.len() > limit {
        return Err(BodyError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| BodyError::NotUtf8)
}

/// Write `body` as a complete `application/json` response. The write is
/// non-blocking: the connection task parks until the socket can take more.
async fn write_json(
    conn: SharedByteBufferStream<RawStream>,
    status: u16,
    body: &serde_json::Value,
    close: bool,
) -> ServeOutcome {
    let bytes = match serde_json::to_vec(body) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("keychain response JSON encode failed: {e}");
            return ServeOutcome::Close(None);
        }
    };
    let mut headers = vec![(SimpleHeader::CONTENT_TYPE, "application/json".to_string())];
    if close {
        headers.push((SimpleHeader::CONNECTION, "close".to_string()));
    }
    match render_head(status, &headers, bytes.len()) {
        Ok(head) => {
            let writer = ResponseWriter::new(conn, WriterConfig::default());
            write_response(writer, head, bytes).await
        }
        Err(e) => {
            tracing::error!("keychain response build failed: {e}");
            ServeOutcome::Close(None)
        }
    }
}
