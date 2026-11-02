//! The proxy request handler — a `foundation_http` `Serve` implementation.
//!
//! WHY: The HTTP framework owns the accept loop and connection lifecycle and
//! calls `Serve::serve` per request, handing the handler the parsed request and
//! the raw connection. The handler routes by host+path, load-balances to a
//! backend, and relays the exchange.
//!
//! WHAT: [`ProxyHandler`], created from the `ContextBag` (which carries the
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

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleIncomingRequest};

use foundation_http::native::writer::{render_head, send_not_found, send_text, ResponseWriter};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};

use crate::config::BackendProtocol;
use crate::forward::{forward_http, forward_upgrade, is_upgrade_request, ForwardContext};
use crate::passthrough::splice_bidirectional;
use crate::runtime::BackendLease;
use crate::runtime::STICKY_COOKIE;
use crate::state::ProxyState;

/// Reverse-proxy handler backed by the shared [`ProxyState`].
#[derive(Debug)]
pub struct ProxyHandler {
    state: Arc<ProxyState>,
}

impl ServeFactory for ProxyHandler {
    /// Pull the shared proxy state out of the context bag.
    ///
    /// # Panics
    /// Panics if [`ProxyState`] was not stored in the bag before routing — a
    /// wiring bug in `ProxyServer::start`, not a runtime condition.
    fn create(bag: &ContextBag) -> Self {
        let state = bag
            .get::<ProxyState>()
            .expect("ProxyState must be stored in the ContextBag before routing");
        Self { state }
    }
}

/// Make a `send_*` response from `foundation_http` close the connection after
/// it is written: the handler's early answers (`404`, `503`) carry
/// `Connection: close`, so the connection must not wait for another request.
fn closing(result: ConnectionResult) -> ConnectionResult {
    match result {
        ConnectionResult::Async(write) => ConnectionResult::Async(Box::pin(async move {
            let _ = write.await;
            ConnectionResult::Close(None)
        })),
        other => other,
    }
}

impl Serve for ProxyHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
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
        let log_start = Instant::now();
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
                duration_ms = log_start.elapsed().as_millis(),
                "access"
            );
        };

        if needs_blocking_relay(protocol, &req) {
            // A blocking boundary (see the module docs): relay on a dedicated
            // thread so the pool worker stays free. The lease moves into the
            // thread so the in-flight count is held for the whole exchange.
            std::thread::spawn(move || {
                relay_blocking(
                    conn,
                    req,
                    lease,
                    protocol,
                    &BlockingRelay {
                        client_ip: &client_ip,
                        scheme: &scheme,
                        sticky_header: sticky_header.as_ref(),
                        io_mode,
                    },
                );
                log_access();
            });
            return ConnectionResult::Take;
        }

        // The relay is a `'static` future: it owns the connection, the request,
        // the lease and the cloned state. The lease is held for the whole
        // exchange and drops with the future, even if the future is dropped
        // unfinished (shutdown).
        ConnectionResult::Async(Box::pin(async move {
            let backend = Arc::clone(lease.backend());
            let mut writer = ResponseWriter::new(conn, writer_config);
            let ctx = ForwardContext {
                client_ip: &client_ip,
                scheme: &scheme,
                client: &client,
                extra_headers: sticky_header.as_ref(),
                client_read_wake,
                max_request_body,
            };
            let result = forward_http(&mut writer, &backend, req, &ctx).await;
            if let Err(e) = result {
                if e.response_started {
                    // The client has part of a response: only close.
                    tracing::warn!(url = %backend.url(), "forward failed mid-response: {e}");
                } else {
                    // The upstream was unreachable or misbehaved: never close
                    // silently, answer a well-formed 502 so the client sees a
                    // real response.
                    tracing::warn!(url = %backend.url(), "forward failed, sending {}: {e}", e.answer_status);
                    let _ = write_error_page(&mut writer, e.answer_status).await;
                }
            }
            log_access();
            // `lease` drops here, decrementing the backend's in-flight count.
            drop(lease);
            ConnectionResult::Close(None)
        }))
    }
}

/// Whether this exchange must run on a dedicated thread: it ends in a byte
/// splice or drives a blocking client (see the module docs).
fn needs_blocking_relay(protocol: BackendProtocol, req: &SimpleIncomingRequest) -> bool {
    match protocol {
        BackendProtocol::Tcp => true,
        // UDP is answered before this point; it never relays.
        BackendProtocol::Udp => false,
        BackendProtocol::Grpc => {
            is_grpc_web_request(req) || is_connect_request(req) || is_upgrade_request(req)
        }
        BackendProtocol::Http
        | BackendProtocol::Https
        | BackendProtocol::Connect
        | BackendProtocol::GrpcWeb => is_upgrade_request(req),
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

/// What a blocking relay needs besides the connection, request and lease.
struct BlockingRelay<'a> {
    client_ip: &'a str,
    scheme: &'a str,
    sticky_header: Option<&'a foundation_netio::shared::http::SimpleHeaders>,
    io_mode: foundation_iogate::ServerIo,
}

/// Relay one taken connection on a dedicated thread: the exchanges
/// [`needs_blocking_relay`] selects.
fn relay_blocking(
    conn: SharedByteBufferStream<RawStream>,
    req: SimpleIncomingRequest,
    lease: BackendLease,
    protocol: BackendProtocol,
    ctx: &BlockingRelay<'_>,
) {
    let BlockingRelay {
        client_ip,
        scheme,
        sticky_header,
        io_mode,
    } = *ctx;
    let backend = Arc::clone(lease.backend());
    let mut conn = conn;
    if matches!(protocol, BackendProtocol::Tcp) {
        // HTTP-fronted raw backend: tunnel the connection through. The bytes
        // already parsed as the HTTP request are not replayed — this path is
        // meaningful after a takeover (e.g. a WebSocket→raw tunnel). The
        // dedicated raw entry point is `passthrough::TcpPassthrough`. The
        // connection is consumed by the splice, so a failure can only be
        // logged, not answered.
        if let Err(e) = tunnel_tcp(conn, &backend) {
            tracing::warn!(url = %backend.url(), "tcp tunnel failed: {e}");
        }
    } else if matches!(protocol, BackendProtocol::Grpc) && is_grpc_web_request(&req) {
        // D leg (spec 63): a `grpc://` backend speaks native gRPC over H2. An
        // inbound gRPC-Web request (H1) is translated at the frame level.
        if let Err(e) = crate::rpc_relay::relay_grpc_web_to_grpc(
            &mut conn,
            &backend,
            req,
            client_ip,
            scheme,
            sticky_header,
        ) {
            tracing::warn!(url = %backend.url(), "grpc-web relay failed, sending 502: {e}");
            let _ = respond::text(&mut conn, 502, "Bad Gateway");
            let _ = conn.flush();
        }
    } else if matches!(protocol, BackendProtocol::Grpc) && is_connect_request(&req) {
        if let Err(e) = crate::rpc_relay::relay_connect_to_grpc(
            &mut conn,
            &backend,
            req,
            client_ip,
            scheme,
            sticky_header,
        ) {
            tracing::warn!(url = %backend.url(), "connect relay failed, sending 502: {e}");
            let _ = respond::text(&mut conn, 502, "Bad Gateway");
            let _ = conn.flush();
        }
    } else {
        // A protocol upgrade: the splice consumes the connection; log on failure.
        if let Err(e) = forward_upgrade(conn, &backend, &req, client_ip, scheme, io_mode) {
            tracing::warn!(url = %backend.url(), "upgrade relay failed: {e}");
        }
    }
    // `lease` drops here, decrementing the backend's in-flight count.
    drop(lease);
}

/// Tunnel a taken connection to a raw TCP backend.
fn tunnel_tcp(
    conn: SharedByteBufferStream<RawStream>,
    backend: &Arc<crate::runtime::BackendRuntime>,
) -> Result<(), String> {
    let authority = backend.target().authority();
    let upstream = std::net::TcpStream::connect(&authority)
        .map_err(|e| format!("connect tcp backend {authority}: {e}"))?;
    upstream
        .set_nonblocking(true)
        .map_err(|e| format!("set upstream nonblocking: {e}"))?;
    splice_bidirectional(conn, upstream);
    Ok(())
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
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
