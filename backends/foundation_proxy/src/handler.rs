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
use std::net::ToSocketAddrs;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_iogate::ServerIo;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest};

use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};

use crate::config::BackendProtocol;
use crate::forward::{forward_http, forward_upgrade, is_upgrade_request};
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
        let host = header_first(&req, &SimpleHeader::HOST).unwrap_or_default();
        let path = req.request_url.url.clone();

        let Some(service) = self.state.router().route(&host, &path) else {
            tracing::debug!(%host, %path, "no service matched host/path — 404");
            let _ = respond::not_found(&mut conn);
            let _ = conn.flush();
            return ConnectionResult::Close(None);
        };

        let Some(lease) = service.pick() else {
            tracing::warn!(service = %service.config().name, "no available backend — 503");
            let _ = respond::text(&mut conn, 503, "Service Unavailable");
            let _ = conn.flush();
            return ConnectionResult::Close(None);
        };

        let client_ip = client_ip(&req);
        let scheme = self.state.scheme().to_string();
        let client = self.state.client().clone();
        let protocol = lease.backend().protocol();
        let io_mode = self.state.io_mode();

        // Relay on a dedicated thread; keep the pool worker free. The lease moves
        // into the thread so the in-flight count is held for the whole exchange.
        std::thread::spawn(move || {
            relay(conn, req, lease, &client_ip, &scheme, protocol, &client, io_mode);
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
    io_mode: ServerIo,
) {
    let backend = Arc::clone(lease.backend());
    let mut conn = conn;
    match protocol {
        BackendProtocol::Tcp => {
            if let Err(e) = tunnel_tcp(conn, &backend, io_mode) {
                tracing::warn!(url = %backend.url(), "tcp tunnel failed: {e}");
            }
        }
        BackendProtocol::Udp => {
            tracing::error!(url = %backend.url(), "UDP backend reached via HTTP — use UdpPassthrough");
            let _ = respond::text(&mut conn, 502, "UDP not reachable via HTTP");
            let _ = conn.flush();
        }
        BackendProtocol::Http | BackendProtocol::Https => {
            if is_upgrade_request(&req) {
                // The upgrade splice consumes the connection; log on failure.
                if let Err(e) = forward_upgrade(conn, &backend, &req, client_ip, scheme, io_mode) {
                    tracing::warn!(url = %backend.url(), "upgrade relay failed: {e}");
                }
            } else if let Err(e) = forward_http(&mut conn, &backend, req, client_ip, scheme, client) {
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
///
/// The upstream is dialed through [`foundation_iogate::connect_completion`] under
/// `io_mode` (F50 Part A/B): in `Completion` mode both legs read from the io_uring
/// inbox and write via `IORING_OP_SEND`; in `Std` mode it is an ordinary
/// non-blocking `TcpStream`, so the splice is byte-for-byte unchanged.
fn tunnel_tcp(
    conn: SharedByteBufferStream<RawStream>,
    backend: &Arc<crate::runtime::BackendRuntime>,
    io_mode: ServerIo,
) -> Result<(), String> {
    let authority = backend.target().authority();
    let addr = authority
        .to_socket_addrs()
        .map_err(|e| format!("resolve tcp backend {authority}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for tcp backend {authority}"))?;
    let upstream = foundation_iogate::connect_completion(addr, io_mode)
        .map_err(|e| format!("connect tcp backend {authority}: {e}"))?;
    splice_bidirectional(conn, upstream);
    Ok(())
}

/// First value of a header, if present.
fn header_first(req: &SimpleIncomingRequest, name: &SimpleHeader) -> Option<String> {
    req.headers.get(name).and_then(|v| v.first()).cloned()
}

/// Best-effort client IP from the connection's peer address.
fn client_ip(req: &SimpleIncomingRequest) -> String {
    match &req.connection.peer_addr {
        Some(addr) => addr.ip().to_string(),
        None => "unknown".to_string(),
    }
}
