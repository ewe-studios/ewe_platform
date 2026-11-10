//! The proxy request handler — a `foundation_http` `Serve` implementation.
//!
//! WHY: The HTTP framework owns the accept loop and connection lifecycle and
//! calls `Serve::serve` per request, handing the handler the parsed request and
//! the raw connection. The handler routes by host+path, load-balances to a
//! backend, and relays the exchange.
//!
//! WHAT: [`ProxyHandler`], created from the `Ctx` (which carries the
//! shared [`ProxyState`]). Matching failures answer `404`; no available backend
//! answers `503`; otherwise the exchange is relayed to the backend.
//!
//! HOW: An ordinary HTTP exchange is relayed by an `async` future that owns the
//! connection, the request, the backend lease and clones of the shared state; the
//! handler returns it as [`ConnectionResult::Async`]. The future awaits the
//! upstream and the client socket, so a slow upstream or client parks the task
//! and holds no worker thread and no OS thread of its own. Four exchanges are
//! different in kind and stay on a dedicated OS thread that returns
//! [`ConnectionResult::Take`]: a protocol upgrade (WebSocket) and a raw TCP
//! tunnel end in a byte splice that owns the socket until it closes, and the
//! gRPC-Web and Connect translators to a `grpc://` backend drive a blocking H2
//! client. Each of those is a real blocking boundary; see `docs/bug-backlog.md`
//! section 9 for the async plan. Responses carry `Connection: close`.

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleIncomingRequest};

use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::writer::{render_head, send_not_found, send_text, ResponseWriter};
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};

use crate::config::BackendProtocol;
use crate::forward::{
    forward_http, forward_upgrade, is_upgrade_request, ForwardContext, ForwardError,
};
use crate::rpc_relay::RpcContext;
use crate::runtime::STICKY_COOKIE;
use crate::state::ProxyState;

/// Reverse-proxy handler backed by the shared [`ProxyState`].
pub struct ProxyHandler {
    state: Arc<ProxyState>,
    /// The context whose clock the access log duration is measured on.
    ctx: Ctx,
}

impl std::fmt::Debug for ProxyHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The clock has no `Debug`; the shared state is the useful part.
        f.debug_struct("ProxyHandler")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ServeFactory for ProxyHandler {
    /// Pull the shared proxy state out of the context.
    ///
    /// # Panics
    /// Panics if [`ProxyState`] was not stored in the context before routing — a
    /// wiring bug in `ProxyServer::start`, not a runtime condition.
    fn create(app_ctx: &Ctx) -> Self {
        let state = app_ctx
            .get::<ProxyState>()
            .expect("ProxyState must be stored in the Ctx before routing");
        let ctx = state.ctx().clone();
        Self { state, ctx }
    }
}

/// Make a `send_*` response from `foundation_http` close the connection after
/// it is written: the handler's early answers (`404`, `503`) carry
/// `Connection: close`, so the connection must not wait for another request.
fn closing(result: ConnectionResult) -> ConnectionResult {
    match result.split() {
        Ok(write) => ConnectionResult::Async(Box::pin(async move {
            let _ = write.await;
            ServeOutcome::Close(None)
        })),
        Err(outcome) => outcome.into(),
    }
}

impl Serve for ProxyHandler {
    #[allow(
        clippy::too_many_lines,
        reason = "the HTTP/1.1 serve path reads best as one ordered flow"
    )]
    fn serve(
        &self,
        serve_ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // Decision 22: during draining, respond 503 so clients retry elsewhere.
        if self.state.is_draining() {
            tracing::info!("proxy draining — returning 503");
            return closing(send_text(conn, 503, "Service Unavailable — draining"));
        }

        let host = header_first(&req, &SimpleHeader::HOST).unwrap_or_default();
        let path = req.request_url.url.clone();
        let method = req.method.clone();
        let user_agent = header_first(&req, &SimpleHeader::USER_AGENT).unwrap_or_default();

        let Some(service) = self.state.router().route(&host, &path) else {
            tracing::debug!(%host, %path, "no service matched host/path — 404");
            return closing(send_not_found(conn));
        };

        // Writer affinity (Decision 23): extract sticky cookie if present.
        let sticky_idx = extract_sticky_cookie(&req);

        let Some((lease, backend_idx)) = service.pick_sticky(sticky_idx) else {
            tracing::warn!(service = %service.config().name, "no available backend — 503");
            return closing(send_text(conn, 503, "Service Unavailable"));
        };

        // Build Set-Cookie header for stickiness (one backend: omitted).
        let sticky_header = if service.backends().len() > 1 {
            let mut h = foundation_netio::shared::http::SimpleHeaders::new();
            h.insert(
                foundation_netio::shared::http::SimpleHeader::SET_COOKIE,
                vec![format!("{STICKY_COOKIE}={backend_idx}; Path=/; HttpOnly")],
            );
            Some(h)
        } else {
            None
        };

        let client_ip = client_ip(&req);
        let scheme = self.state.scheme().to_string();
        let client = self.state.client().clone();
        let protocol = lease.backend().protocol();
        let io_mode = self.state.io_mode();
        let writer_config = self.state.writer_config();
        let max_request_body = self.state.max_request_body();
        let connect_timeout = self.state.connect_timeout();
        // Only a request with a body needs the client's read readiness.
        let client_read_wake = if matches!(req.body, None | Some(SendSafeBody::None)) {
            None
        } else {
            conn.with_inner_ref(RawStream::read_wake_source)
        };

        // UDP is connectionless — traffic enters via `UdpPassthrough`, not the
        // HTTP front end. Reaching it here means a UDP backend was matched via
        // HTTP routing: answer, do not relay.
        if matches!(protocol, BackendProtocol::Udp) {
            tracing::error!(url = %lease.backend().url(), "UDP backend reached via HTTP handler — must use UdpPassthrough");
            drop(lease);
            return closing(send_text(conn, 502, "UDP backend not reachable via HTTP"));
        }

        // Access log: method, host, path, status (filled by relay), duration.
        let log_start = self.ctx.clock().now();
        let log_ctx = self.ctx.clone();
        let log_host = host.clone();
        let log_path = path.clone();
        let log_method = method.clone();
        let log_ua = user_agent.clone();
        let log_ip = client_ip.clone();
        let log_access = move || {
            tracing::info!(
                method = %log_method,
                host = %log_host,
                path = %log_path,
                client_ip = %log_ip,
                user_agent = %log_ua,
                duration_ms = log_ctx.clock().now().duration_since(log_start).as_millis(),
                "access"
            );
        };

        // The relay is a `'static` future: it owns the connection, the request,
        // the lease and the cloned state. The lease is held for the whole
        // exchange and drops with the future, even if the future is dropped
        // unfinished (shutdown).
        let upgrade = is_upgrade_request(&req);
        let is_grpc_web = is_grpc_web_request(&req);
        let is_connect = is_connect_request(&req);
        ConnectionResult::Async(Box::pin(async move {
            let backend = Arc::clone(lease.backend());
            if matches!(protocol, BackendProtocol::Tcp) {
                // HTTP-fronted raw backend: tunnel the connection through. The
                // bytes already parsed as the HTTP request are not replayed. The
                // dedicated raw entry point is `passthrough::TcpPassthrough`.
                relay_tcp_tunnel(conn, &backend, io_mode, connect_timeout, writer_config).await;
            } else if upgrade {
                // The splice owns the connection until either side closes. A
                // failure before it starts leaves the connection ours: answer.
                let outcome = forward_upgrade(
                    &conn,
                    &backend,
                    &req,
                    &client_ip,
                    &scheme,
                    io_mode,
                    connect_timeout,
                    writer_config,
                )
                .await;
                if let Err(e) = outcome {
                    tracing::warn!(url = %backend.url(), "upgrade relay failed, sending 502: {e}");
                    let mut writer = ResponseWriter::new(conn, writer_config);
                    let _ = write_error_page(&mut writer, 502).await;
                }
            } else if matches!(protocol, BackendProtocol::Grpc) && is_grpc_web {
                // D leg (spec 63): a `grpc://` backend speaks native gRPC over
                // H2. An inbound gRPC-Web request (H1) is translated at the
                // frame level.
                let mut writer = ResponseWriter::new(conn, writer_config);
                let ctx = RpcContext {
                    client_ip: &client_ip,
                    scheme: &scheme,
                    sticky_header: sticky_header.as_ref(),
                    client_read_wake,
                    max_request_body,
                };
                let result =
                    crate::rpc_relay::relay_grpc_web_to_grpc(&mut writer, &backend, req, &ctx)
                        .await;
                finish_relay(&mut writer, &backend, result, "grpc-web").await;
            } else if matches!(protocol, BackendProtocol::Grpc) && is_connect {
                let mut writer = ResponseWriter::new(conn, writer_config);
                let ctx = RpcContext {
                    client_ip: &client_ip,
                    scheme: &scheme,
                    sticky_header: sticky_header.as_ref(),
                    client_read_wake,
                    max_request_body,
                };
                let result =
                    crate::rpc_relay::relay_connect_to_grpc(&mut writer, &backend, req, &ctx).await;
                finish_relay(&mut writer, &backend, result, "connect").await;
            } else {
                let mut writer = ResponseWriter::new(conn, writer_config);
                let ctx = ForwardContext {
                    client_ip: &client_ip,
                    scheme: &scheme,
                    client: &client,
                    extra_headers: sticky_header.as_ref(),
                    client_read_wake,
                    max_request_body,
                    call_ctx: &serve_ctx,
                };
                let result = forward_http(&mut writer, &backend, req, &ctx).await;
                finish_relay(&mut writer, &backend, result, "forward").await;
            }
            log_access();
            // `lease` drops here, decrementing the backend's in-flight count.
            drop(lease);
            ServeOutcome::Close(None)
        }))
    }
}

/// Tunnel a taken connection to a raw TCP backend.
async fn relay_tcp_tunnel(
    conn: SharedByteBufferStream<RawStream>,
    backend: &Arc<crate::runtime::BackendRuntime>,
    io_mode: foundation_iogate::ServerIo,
    connect_timeout: std::time::Duration,
    writer_config: foundation_http::native::writer::WriterConfig,
) {
    let authority = backend.target().authority();
    match crate::aio::dial_tcp(&authority, io_mode, connect_timeout).await {
        Ok(upstream) => crate::aio::splice(conn, upstream, writer_config).await,
        Err(e) => {
            // The connection is untouched: answer so the client sees a response.
            tracing::warn!(url = %backend.url(), "tcp tunnel failed, sending 502: {e}");
            let mut writer = ResponseWriter::new(conn, writer_config);
            let _ = write_error_page(&mut writer, 502).await;
        }
    }
}

/// Log a failed relay and, when the client has no byte of a response yet,
/// answer it: a `502` for an upstream failure, a `413` for a body over the limit.
/// Never close silently.
async fn finish_relay(
    writer: &mut ResponseWriter,
    backend: &Arc<crate::runtime::BackendRuntime>,
    result: Result<(), ForwardError>,
    what: &str,
) {
    let Err(e) = result else { return };
    if e.response_started {
        // The client has part of a response: only close.
        tracing::warn!(url = %backend.url(), "{what} failed mid-response: {e}");
    } else {
        tracing::warn!(url = %backend.url(), "{what} failed, sending {}: {e}", e.answer_status);
        let _ = write_error_page(writer, e.answer_status).await;
    }
}

/// Write a complete `502 Bad Gateway` (or `413 Payload Too Large`) answer.
async fn write_error_page(writer: &mut ResponseWriter, status: u16) -> std::io::Result<()> {
    let body = if status == 413 {
        "Payload Too Large"
    } else {
        "Bad Gateway"
    };
    let headers = [
        (SimpleHeader::CONTENT_TYPE, "text/plain".to_string()),
        (SimpleHeader::CONNECTION, "close".to_string()),
    ];
    let head = render_head(status, &headers, body.len())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writer.write_all(&head).await?;
    writer.write_all(body.as_bytes()).await?;
    writer.flush().await
}

/// First value of a header, if present.
fn header_first(req: &SimpleIncomingRequest, name: &SimpleHeader) -> Option<String> {
    req.headers.get(name).and_then(|v| v.first()).cloned()
}

/// Whether the request's `Content-Type` marks a gRPC-Web exchange (`application/grpc-web[-text]+…`),
/// which the D-leg relay translates to native gRPC for a `grpc://` backend.
fn is_grpc_web_request(req: &SimpleIncomingRequest) -> bool {
    header_first(req, &SimpleHeader::CONTENT_TYPE)
        .is_some_and(|ct| ct.starts_with("application/grpc-web"))
}

/// Whether the request's `Content-Type` marks a Connect exchange
/// (`application/{codec}` unary or `application/connect+{codec}` streaming),
/// which the D-leg relay translates to native gRPC for a `grpc://` backend.
fn is_connect_request(req: &SimpleIncomingRequest) -> bool {
    header_first(req, &SimpleHeader::CONTENT_TYPE)
        .and_then(|ct| foundation_connectrpc::parse_connect_content_type(&ct))
        .is_some()
}

/// Extract the `__proxy_sticky` cookie value from the request's Cookie header.
/// Returns the backend index if found and parseable, `None` otherwise.
fn extract_sticky_cookie(req: &SimpleIncomingRequest) -> Option<usize> {
    let cookie_header = header_first(req, &SimpleHeader::COOKIE)?;
    for part in cookie_header.split(';') {
        let kv = part.trim();
        if let Some(value) = kv.strip_prefix(&format!("{STICKY_COOKIE}=")) {
            return value.parse::<usize>().ok();
        }
    }
    None
}

/// Best-effort client IP from the connection's peer address.
fn client_ip(req: &SimpleIncomingRequest) -> String {
    req.connection
        .peer_addr
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |a| a.ip().to_string())
}
