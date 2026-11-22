//! HTTP server for web test modes, built on `foundation_http`.
//!
//! WHY: Browser tests need an HTTP server to serve the integration directory,
//! and every suite needs a local endpoint it can call (spec 88 invariant I5).
//! WHAT: Wraps `foundation_http`'s `HttpServer` with `StaticFileHandler` on
//! `/*`, [`EchoHandler`] on `/echo` and [`RpcRoute`] on `/connect/*`.
//! HOW: Creates an `HttpApp` with the routes, starts the server on a thread,
//! and provides shutdown via `ShutdownSignal`.

use std::path::Path;
use std::sync::Arc;
use std::thread;

use base64::Engine as _;
use foundation_connectrpc::{
    Codec, CodecError, CodecFor, ConnectError, ConnectRpcHandler, HandlerOptions, Request,
    Response, Router,
};
use foundation_core::io::readers::DataBytesIterator;
use foundation_http::native::handlers::static_file::StaticFileHandler;
use foundation_http::native::serve::Serve;
use foundation_http::native::writer::{
    ResponseWriter, WriterConfig, render_message, send_json, send_text,
};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_http::{
    RawStream, SendSafeBody, SharedByteBufferStream, ShutdownSignal, SimpleIncomingRequest,
};
use serde_json::json;
use tracing::debug;

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Limits of the test server.
///
/// Every field has a builder method and a documented default (owner rule 8).
#[derive(Clone, Debug)]
pub struct ServerLimits {
    /// The largest body the `/echo` route accepts, in bytes. A larger body gets
    /// 413. Default 1 MiB: the order of a default nginx `client_max_body_size`,
    /// and enough for a suite that echoes a request body once.
    pub max_echo_body: usize,
    /// The largest unary request body a `/connect` procedure accepts, in bytes.
    /// Default 4 MiB: gRPC's default maximum receive message size, the same
    /// default the `ConnectRPC` router uses.
    pub max_connect_body: usize,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            max_echo_body: 1024 * 1024,
            max_connect_body: 4 * 1024 * 1024,
        }
    }
}

impl ServerLimits {
    /// The default limits. See the field docs for each default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::max_echo_body`].
    #[must_use]
    pub fn with_max_echo_body(mut self, bytes: usize) -> Self {
        self.max_echo_body = bytes;
        self
    }

    /// Set [`Self::max_connect_body`].
    #[must_use]
    pub fn with_max_connect_body(mut self, bytes: usize) -> Self {
        self.max_connect_body = bytes;
        self
    }
}

/// Answers `/echo` with the request it received (spec 88 invariant I5).
///
/// The body is a JSON object: `method`, `path`, `headers`, `body` as base64 and
/// `bytes` (the body length). A body over the limit gets 413.
pub struct EchoHandler {
    max_body: usize,
}

impl EchoHandler {
    /// An echo handler that accepts up to `max_body` bytes.
    #[must_use]
    pub fn new(max_body: usize) -> Self {
        Self { max_body }
    }
}

impl Serve for EchoHandler {
    fn serve(
        &self,
        ctx: Ctx,
        request: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> foundation_http::shared::serve::ConnectionResult {
        // The connection reader streams the body, so read it the way the
        // client's own body reader does: one chunk at a time, and stop at the
        // limit instead of buffering an unbounded body.
        let (body, too_large) = match request.body {
            None | Some(SendSafeBody::None | SendSafeBody::Stream(None)) => (Vec::new(), false),
            Some(SendSafeBody::Text(text)) => {
                let bytes = text.into_bytes();
                let too_large = bytes.len() > self.max_body;
                (bytes, too_large)
            }
            Some(SendSafeBody::Bytes(bytes)) => {
                let too_large = bytes.len() > self.max_body;
                (bytes, too_large)
            }
            Some(SendSafeBody::Stream(Some(iter))) => {
                let mut bytes = Vec::new();
                let mut too_large = false;
                for chunk in DataBytesIterator::new(iter) {
                    match chunk {
                        Ok(data) => {
                            bytes.extend_from_slice(&data);
                            if bytes.len() > self.max_body {
                                too_large = true;
                                break;
                            }
                        }
                        Err(e) => {
                            return send_text(
                                conn,
                                400,
                                &format!("echo could not read the body: {e}"),
                                &ctx,
                            );
                        }
                    }
                }
                (bytes, too_large)
            }
            Some(_) => {
                return send_text(
                    conn,
                    400,
                    "echo accepts a body with a length or a byte stream, not a chunked or event stream",
                    &ctx,
                );
            }
        };
        if too_large {
            // The body is still arriving: answer and close, so the unread
            // bytes never become the next request line.
            return match send_text(conn, 413, "echo body too large", &ctx) {
                foundation_http::shared::serve::ConnectionResult::Keep => {
                    foundation_http::shared::serve::ConnectionResult::Close(None)
                }
                other => other,
            };
        }
        let headers: serde_json::Map<String, serde_json::Value> = request
            .headers
            .iter()
            .map(|(name, values)| (name.to_string(), json!(values.join(", "))))
            .collect();
        let response = json!({
            "method": request.method.to_string(),
            "path": request.request_uri.path(),
            "headers": headers,
            "body": base64::engine::general_purpose::STANDARD.encode(&body),
            "bytes": body.len(),
        });
        send_json(conn, 200, &response, &ctx)
    }
}

/// A pass-through codec. Its wire name is `"proto"`, so a client reads the
/// response bytes without a schema (spec 88 feature 03, T4).
struct IdentityCodec;

impl Codec for IdentityCodec {
    fn name(&self) -> &'static str {
        "proto"
    }

    fn is_binary(&self) -> bool {
        true
    }
}

impl CodecFor<Vec<u8>> for IdentityCodec {
    fn marshal(&self, message: &Vec<u8>) -> std::result::Result<bytes::Bytes, CodecError> {
        Ok(bytes::Bytes::copy_from_slice(message))
    }

    fn unmarshal(&self, data: bytes::Bytes) -> std::result::Result<Vec<u8>, CodecError> {
        if data.is_empty() {
            return Err(CodecError::ZeroLength { codec: "identity" });
        }
        Ok(data.to_vec())
    }

    fn marshal_stable(&self, message: &Vec<u8>) -> std::result::Result<bytes::Bytes, CodecError> {
        Ok(bytes::Bytes::copy_from_slice(message))
    }

    fn marshal_append(
        &self,
        buf: &mut Vec<u8>,
        message: &Vec<u8>,
    ) -> std::result::Result<(), CodecError> {
        buf.extend_from_slice(message);
        Ok(())
    }
}

/// Serves a small `ConnectRPC` service on `/connect/*` (spec 88 feature 03).
///
/// The service answers three procedures:
///
/// - `case.CaseService/Unary` echoes the request with an `echo:` prefix;
/// - `case.CaseService/Stream` answers three frames;
/// - `case.CaseService/Fail` answers `invalid_argument`.
///
/// A suite reaches it on the same origin as its page, so the browser needs no
/// cross-origin grant.
struct RpcRoute {
    handler: Arc<ConnectRpcHandler>,
    /// The largest request body the route reads, in bytes.
    max_body: usize,
}

impl RpcRoute {
    /// Build the route's router with the given unary body limit.
    fn new(max_body: usize) -> Self {
        let codecs =
            || foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
        let options = || HandlerOptions::new().with_read_max_bytes(max_body);

        let mut router = Router::new();
        router.unary(
            "/connect/case.CaseService/Unary",
            codecs(),
            |_ctx, req: Request<Vec<u8>>| async move {
                let mut out = b"echo:".to_vec();
                out.extend_from_slice(&req.msg);
                Ok(Response::new(out))
            },
            options(),
        );
        router.server_stream(
            "/connect/case.CaseService/Stream",
            codecs(),
            |_ctx, _req: Request<Vec<u8>>| async move {
                let frames: Vec<foundation_connectrpc::ConnectResult<Vec<u8>>> = (0..3)
                    .map(|index| Ok(format!("frame-{index}").into_bytes()))
                    .collect();
                Ok(futures::stream::iter(frames))
            },
            options(),
        );
        router.unary(
            "/connect/case.CaseService/Fail",
            codecs(),
            |_ctx, _req: Request<Vec<u8>>| async move {
                Err(ConnectError::invalid_argument("the handler rejected the call").into())
            },
            options(),
        );

        Self {
            handler: Arc::new(router.into_handler()),
            max_body,
        }
    }
}

impl Serve for RpcRoute {
    fn serve(
        &self,
        ctx: Ctx,
        request: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let handler = Arc::clone(&self.handler);
        let max_body = self.max_body;
        ConnectionResult::Async(Box::pin(async move {
            // The native connection delivers a chunked body lazily, and the
            // router's in-memory dispatch cannot wait on the connection. Read
            // the body here, bounded by the configured limit, then hand it to
            // dispatch as bytes. A fetch with a streamed body sends it chunked.
            let mut request = request;
            match read_body(&mut request, max_body) {
                Ok(Ok(())) => {}
                Ok(Err(())) => {
                    let mut writer = ResponseWriter::new(conn, WriterConfig::default(), ctx);
                    let _ = writer.write_all(b"HTTP/1.1 413 Payload Too Large\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
                    let _ = writer.flush().await;
                    return ServeOutcome::Close(None);
                }
                Err(error) => {
                    tracing::debug!("connect route could not read the body: {error}");
                    return ServeOutcome::Close(None);
                }
            }
            let mut response = handler.dispatch(ctx.clone(), request).await;
            // The HTTP/1.1 renderer requires a header. A route miss from the
            // router carries none, so add the framing header the connection uses.
            if response.headers.is_empty() {
                response.headers.insert(
                    foundation_http::SimpleHeader::CONNECTION,
                    vec!["keep-alive".to_string()],
                );
            }
            let bytes = match render_message(response) {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::error!("connect route could not render the response: {error}");
                    return ServeOutcome::Close(None);
                }
            };
            let mut writer = ResponseWriter::new(conn, WriterConfig::default(), ctx);
            if let Err(error) = writer.write_all(&bytes).await {
                tracing::debug!("connect route write failed: {error}");
                return ServeOutcome::Close(None);
            }
            if let Err(error) = writer.flush().await {
                tracing::debug!("connect route flush failed: {error}");
                return ServeOutcome::Close(None);
            }
            ServeOutcome::Keep
        }))
    }
}

/// Read a request body into memory, bounded by `max`.
///
/// Returns `Ok(Err(()))` when the body passes the limit, and `Err` when the
/// body cannot be read. `/connect` is a test endpoint, so a bounded in-memory
/// body is acceptable here (root rule 4: a small body whose contents feed the
/// next step).
fn read_body(
    request: &mut SimpleIncomingRequest,
    max: usize,
) -> std::io::Result<std::result::Result<(), ()>> {
    let mut bytes = Vec::new();
    match request.body.take() {
        None | Some(SendSafeBody::None | SendSafeBody::Stream(None)) => {}
        Some(SendSafeBody::Text(text)) => bytes = text.into_bytes(),
        Some(SendSafeBody::Bytes(value)) => bytes = value,
        Some(SendSafeBody::Stream(Some(iter))) => {
            for chunk in DataBytesIterator::new(iter) {
                let chunk = chunk.map_err(|e| std::io::Error::other(e.to_string()))?;
                bytes.extend_from_slice(&chunk);
                if bytes.len() > max {
                    return Ok(Err(()));
                }
            }
        }
        Some(SendSafeBody::ChunkedStream(Some(iter))) => {
            for item in iter {
                let item = item.map_err(|e| std::io::Error::other(e.to_string()))?;
                if let foundation_netio::shared::http::ChunkedData::Data(chunk, _) = item {
                    bytes.extend_from_slice(&chunk);
                    if bytes.len() > max {
                        return Ok(Err(()));
                    }
                }
            }
        }
        Some(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "connect accepts a length body or a byte stream",
            ));
        }
    }
    if bytes.len() > max {
        return Ok(Err(()));
    }
    request.body = Some(SendSafeBody::Bytes(bytes));
    Ok(Ok(()))
}

/// Running test server.
pub struct TestServer {
    port: u16,
    shutdown_signal: Arc<ShutdownSignal>,
    server_thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    /// Returns the bound port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the URL for a given path on this server.
    ///
    /// The host is the same as [`Self::base_url`], so a page and the endpoint a
    /// case calls share one origin and the browser needs no cross-origin grant.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url(), path.trim_start_matches('/'))
    }

    /// The origin every suite calls: `http://127.0.0.1:<port>`.
    ///
    /// It uses the loopback address, not `localhost`, so a fetch never waits on
    /// a name lookup.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Signal the server to stop and wait for the thread to finish.
    pub fn shutdown(mut self) {
        self.shutdown_signal.turn_on();
        if let Some(handle) = self.server_thread.take() {
            let _ = handle.join();
        }
        debug!("Test server shut down");
    }
}

/// Start a static file server serving the integration directory.
///
/// The `/echo` route uses the default limits. Use
/// [`start_serving_with_echo`] to set them.
///
/// # Errors
///
/// Returns an error if no available port can be found.
pub fn start_serving(integration_dir: &Path) -> Result<TestServer> {
    start_serving_with_echo(integration_dir, &ServerLimits::default())
}

/// Start a static file server with the `/echo` route and the given limits.
///
/// The server binds to a random available port on 127.0.0.1. `/echo` answers
/// before `/*`, so a file named `echo` never shadows the route.
///
/// # Errors
///
/// Returns an error if no available port can be found.
pub fn start_serving_with_echo(
    integration_dir: &Path,
    limits: &ServerLimits,
) -> Result<TestServer> {
    let port =
        portpicker::pick_unused_port().ok_or_else(|| WasmTestbedError::NoPortAvailable.trace())?;

    let shutdown = Arc::new(ShutdownSignal::new());
    let shutdown_clone = shutdown.clone();
    let root = integration_dir.to_path_buf();
    let max_echo_body = limits.max_echo_body;
    let max_connect_body = limits.max_connect_body;
    let addr = format!("127.0.0.1:{port}");

    let handle = thread::spawn(move || {
        let mut app = HttpApp::new_serve();
        let echo: Arc<dyn Serve> = Arc::new(EchoHandler::new(max_echo_body));
        app.router.add_route_any("/echo", &echo);
        // The RPC route answers before `/*`, so a file named `connect` never
        // shadows it. A browser suite on the same origin reaches it directly.
        let rpc: Arc<dyn Serve> = Arc::new(RpcRoute::new(max_connect_body));
        app.router.add_route_any("/connect/*", &rpc);
        let handler: Arc<dyn Serve> = Arc::new(StaticFileHandler::new(&root));
        app.router.add_route_any("/*", &handler);

        let server = app.server(&addr);
        server.serve(&shutdown_clone);
    });

    tracing::info!("Test server listening on http://127.0.0.1:{port}");

    Ok(TestServer {
        port,
        shutdown_signal: shutdown,
        server_thread: Some(handle),
    })
}
