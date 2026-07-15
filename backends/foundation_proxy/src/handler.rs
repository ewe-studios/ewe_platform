//! The proxy request handler — a `foundation_http` `Serve` implementation.
//!
//! WHY: The HTTP framework owns the accept loop and connection lifecycle and
//! calls `Serve::serve` per request, handing the handler the parsed request and
//! the raw connection. The handler routes by host+path, load-balances to a
//! backend, and relays the exchange.
//!
//! WHAT: [`ProxyHandler`], created from the `ContextBag` (which carries the
//! shared [`ProxyState`]). Matching failures answer `404`; no available backend
//! answers `503`; otherwise the connection is taken and relayed on a dedicated
//! thread.
//!
//! HOW: Relaying runs on its own OS thread and the handler returns
//! [`ConnectionResult::Take`]. This deliberately keeps upstream I/O off the
//! valtron worker pool: the pooled client drives the executor to completion, and
//! doing that from a pool worker risks a cross-executor stall. One thread per
//! proxied connection is the price; keep-alive on the client side is not offered
//! in stage 1 (responses carry `Connection: close`).

use std::io::Write;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::{RawStream, SocketAddr as NetcapSocketAddr};
use foundation_netio::simple_http::shared::{SimpleHeader, SimpleIncomingRequest};

use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};

use crate::config::BackendProtocol;
use crate::forward::{forward_http_with_headers, forward_upgrade, is_upgrade_request};
use crate::runtime::STICKY_COOKIE;
use crate::passthrough::splice_bidirectional;
use crate::runtime::BackendLease;
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

impl Serve for ProxyHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // Decision 22: during draining, respond 503 so clients retry elsewhere.
        if self.state.is_draining() {
            tracing::info!("proxy draining — returning 503");
            let _ = respond::text(&mut conn, 503, "Service Unavailable — draining");
            let _ = conn.flush();
            return ConnectionResult::Close(None);
        }

        let host = header_first(&req, &SimpleHeader::HOST).unwrap_or_default();
        let path = req.request_url.url.clone();

        let Some(service) = self.state.router().route(&host, &path) else {
            tracing::debug!(%host, %path, "no service matched host/path — 404");
            let _ = respond::not_found(&mut conn);
            let _ = conn.flush();
            return ConnectionResult::Close(None);
        };

        // Writer affinity (Decision 23): extract sticky cookie if present.
        let sticky_idx = extract_sticky_cookie(&req);

        let Some((lease, backend_idx)) = service.pick_sticky(sticky_idx) else {
            tracing::warn!(service = %service.config().name, "no available backend — 503");
            let _ = respond::text(&mut conn, 503, "Service Unavailable");
            let _ = conn.flush();
            return ConnectionResult::Close(None);
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

        // Relay on a dedicated thread; keep the pool worker free. The lease moves
        // into the thread so the in-flight count is held for the whole exchange.
        std::thread::spawn(move || {
            relay(conn, req, lease, &client_ip, &scheme, protocol, &client, sticky_header.as_ref());
        });

        ConnectionResult::Take
    }
}

/// Relay one taken connection to its backend and finish the lease.
fn relay(
    conn: SharedByteBufferStream<RawStream>,
    req: SimpleIncomingRequest,
    lease: BackendLease,
    client_ip: &str,
    scheme: &str,
    protocol: BackendProtocol,
    client: &crate::forward::SharedHttpClient,
    sticky_header: Option<&foundation_netio::shared::http::SimpleHeaders>,
) {
    let backend = Arc::clone(lease.backend());
    let mut conn = conn;
    match protocol {
        BackendProtocol::Tcp => {
            // HTTP-fronted raw backend: tunnel the connection through. The bytes
            // already parsed as the HTTP request are not replayed — this path is
            // meaningful after a takeover (e.g. a WebSocket→raw tunnel). The
            // dedicated raw entry point is `passthrough::TcpPassthrough`. The
            // connection is consumed by the splice, so a failure can only be
            // logged, not answered.
            if let Err(e) = tunnel_tcp(conn, &backend) {
                tracing::warn!(url = %backend.url(), "tcp tunnel failed: {e}");
            }
        }
        BackendProtocol::Udp => {
            // UDP is connectionless — traffic enters via `UdpPassthrough`, not
            // the HTTP front end. This arm exists for exhaustiveness; reaching
            // it means a UDP backend was somehow matched via HTTP routing.
            tracing::error!(url = %backend.url(), "UDP backend reached via HTTP handler — must use UdpPassthrough");
            let _ = respond::text(&mut conn, 502, "UDP backend not reachable via HTTP");
            let _ = conn.flush();
        }
        BackendProtocol::Http | BackendProtocol::Https => {
            if is_upgrade_request(&req) {
                // The upgrade splice consumes the connection; log on failure.
                if let Err(e) = forward_upgrade(conn, &backend, &req, client_ip, scheme) {
                    tracing::warn!(url = %backend.url(), "upgrade relay failed: {e}");
                }
            } else if let Err(e) = forward_http_with_headers(&mut conn, &backend, req, client_ip, scheme, client, sticky_header) {
                // The upstream was unreachable or misbehaved: never close silently,
                // answer a well-formed 502 so the client sees a real response.
                tracing::warn!(url = %backend.url(), "forward failed, sending 502: {e}");
                let _ = std::fs::write("/tmp/claude-1000/-home-darkvoid-Boxxed--dev-ewe-platform/964581c8-12f5-46a2-b31a-24a5ee839229/scratchpad/fwd_err.txt", format!("url={} err={e}", backend.url()));
                let _ = respond::text(&mut conn, 502, "Bad Gateway");
                let _ = conn.flush();
            }
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
    match &req.connection.peer_addr {
        Some(NetcapSocketAddr::Tcp(addr)) => addr.ip().to_string(),
        #[cfg(unix)]
        Some(NetcapSocketAddr::Unix(_)) => "unix".to_string(),
        None => "unknown".to_string(),
    }
}
