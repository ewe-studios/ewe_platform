//! Connect, send the request, and follow redirects: the first half of an HTTP/1.1
//! exchange, as a valtron `TaskIterator` state machine.
//!
//! WHY: the client must connect, write the request head, and negotiate
//! `Expect: 100-continue`. It writes the request body, reads enough of the
//! response to follow redirects, and then hands the connection to the
//! response reader. Each step can wait on the network. Done with blocking reads,
//! every wait held a valtron worker. When the server ran on the same pool, the
//! 100-continue wait starved the very handler that would answer it, and each POST
//! sat out the full 3s timeout (#76).
//!
//! WHAT: [`GetHttpRequestRedirectTask`] yields an [`HttpRequestRedirectResponse`]:
//! the connection plus a response reader, and any response head it already read.
//!
//! HOW: once connected, a connection that can report readiness (kernel TCP/Unix
//! sockets, rustls over them, the overlay) is switched to non-blocking. Each step
//! then moves bytes until the socket would block, and returns
//! `TaskStatus::Depends` on "socket ready, or deadline passed". Writes go through
//! [`PendingBytes`], so a partial write resumes exactly where it stopped. The
//! 100-continue wait is a readiness wait bounded by
//! `ClientConfig::expect_continue_read_timeout`. The body goes out when
//! `100 Continue` arrives or the timer fires, whichever comes first (RFC 9110
//! §10.1.1). Transports that cannot report readiness (completion sockets, the
//! openssl and native-tls backends) keep the blocking path, with kernel read
//! timeouts.

use crate::http::{HttpClientConnection, HttpConnectionPool};
use crate::native::readiness::{DeadlineReadiness, IoInterest};
use crate::netcap::{Connection, RawStream};
#[cfg(unix)]
use crate::native::readiness::SocketReadiness;
use crate::native::readiness::io_or_deadline;
#[cfg(unix)]
use crate::netcap::PendingConnect;
#[cfg(feature = "ssl-rustls")]
use crate::netcap::ssl::{ClientSSLStream, SSLConnector};
use crate::shared::client::{redirects, ClientConfig, DnsResolver};
use crate::shared::http::{
    ensure_chunked_transfer_encoding, BodyReadiness, Http11, Http11RequestBodyIterator,
    HttpClientError, HttpResponseReader, IncomingResponseParts, RenderHttp, RequestDescriptor,
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleHttpBody, SimpleIncomingRequest, Status,
};
use foundation_core::io::ioutils::{ReadTimeoutOperations, SharedByteBufferStream};
use foundation_core::valtron::{Ctx, AnyReadiness, TaskIterator, TaskStatus};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use super::nonblocking::{self, IdleDeadline, IoStep, Park, PendingBytes};
use super::HttpOperationState;

// Type aliases for complex enum variant data
type InitData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::shared::client::ClientConfig,
    u8,
)>;

type TryingData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::shared::client::ClientConfig,
    RequestDescriptor,
    u8,
)>;

/// Blocking-transport body write (the path for transports without readiness).
type WriteBodyData<R> = Box<(
    Option<[IncomingResponseParts; 2]>,
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    HttpClientConnection,
    HttpResponseReader<SimpleHttpBody, RawStream>,
)>;

/// One step of the state machine.
type Step = TaskStatus<HttpRequestRedirectResponse, HttpOperationState>;

/// Most body chunks written in one poll before yielding back to the executor, so
/// a large in-memory body cannot monopolise a worker.
const MAX_CHUNKS_PER_POLL: usize = 32;

/// The request being sent and the connection carrying it.
pub struct Exchange<R: DnsResolver + Send + 'static> {
    data: SimpleIncomingRequest,
    pool: Arc<HttpConnectionPool<R>>,
    config: ClientConfig,
    descriptor: RequestDescriptor,
    remaining_redirects: u8,
    connection: HttpClientConnection,
    reader: HttpResponseReader<SimpleHttpBody, RawStream>,
}

/// The request being sent, before the connection exists.
///
/// WHY: the non-blocking connect and TLS handshake states (feature 04) need
/// everything [`Exchange`] carries except the connection and reader, so the
/// connect can happen across `Depends` polls instead of in one blocking call.
struct PendingExchange<R: DnsResolver + Send + 'static> {
    data: SimpleIncomingRequest,
    pool: Arc<HttpConnectionPool<R>>,
    config: ClientConfig,
    descriptor: RequestDescriptor,
    remaining_redirects: u8,
    host: String,
    port: u16,
    /// The target is HTTPS/WSS: a TLS handshake must follow the TCP connect.
    use_tls: bool,
}

/// What follows once the request head is on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterHead {
    /// No body: read the response head to check for redirects.
    ReadHead,
    /// Body without `Expect`: write it straight away.
    WriteBody,
    /// Body with `Expect: 100-continue`: wait for `100` or the timer first.
    AwaitContinue,
}

/// Writing the request head on a non-blocking socket.
pub struct SendingHead<R: DnsResolver + Send + 'static> {
    exchange: Exchange<R>,
    pending: PendingBytes,
    idle: IdleDeadline,
    next: AfterHead,
}

/// Waiting for `100 Continue` (or an early final response) before the body.
pub struct AwaitingContinue<R: DnsResolver + Send + 'static> {
    exchange: Exchange<R>,
    /// When the wait gives up and sends the body anyway (RFC 9110 §10.1.1).
    send_body_at: Instant,
    /// A status line already read whose headers have not arrived yet.
    intro: Option<IncomingResponseParts>,
    idle: IdleDeadline,
}

/// Reading a bodyless request's response head to check for redirects.
pub struct ReadingHead<R: DnsResolver + Send + 'static> {
    exchange: Exchange<R>,
    /// A final status line already read whose headers have not arrived yet.
    intro: Option<IncomingResponseParts>,
    idle: IdleDeadline,
}

/// Streaming the request body on a non-blocking socket.
///
/// WHY: the body renderer yields chunks one at a time. A pushable body
/// (client-streaming RPCs) yields an empty chunk while its producer has not
/// pushed anything yet. Socket stalls park on writability. Producer stalls park
/// on the pipe's [`BodyReadiness`] (the task wakes the moment the producer
/// pushes), bounded by the idle deadline. A one-shot renderer transition (for
/// example chunked trailers) is retried at once. Either way the worker is
/// released. The blocking writer used to spin on those empty chunks instead.
pub struct WritingBody {
    starters: Option<[IncomingResponseParts; 2]>,
    connection: HttpClientConnection,
    reader: HttpResponseReader<SimpleHttpBody, RawStream>,
    chunks: Http11RequestBodyIterator,
    pending: PendingBytes,
    rendered_all: bool,
    idle: IdleDeadline,
    /// Consumer-side pipe readiness for a pushable body; `None` for other bodies.
    body_readiness: Option<BodyReadiness>,
}

/// A TCP connect in flight on a non-blocking socket (feature 04 §2.1).
///
/// WHY: `TcpStream::connect_timeout` holds a valtron worker for the whole kernel
/// connect. Instead, this state creates the socket non-blocking, starts
/// `connect(2)`, and parks on write readiness of the raw fd. When the fd turns
/// writable it reads `SO_ERROR` and either hands over the connected socket or
/// tries the next resolved address.
#[cfg(unix)]
pub struct Connecting<R: DnsResolver + Send + 'static> {
    exchange: PendingExchange<R>,
    addrs: std::vec::IntoIter<core::net::SocketAddr>,
    current: Option<PendingConnect>,
    /// Fixed connect deadline, shared across all addresses (remaining time).
    deadline: Instant,
    /// The last address's connect error, so the caller sees the real `io::ErrorKind`
    /// (refused, unreachable, …) — feature 04 §2.4 rule 3.
    last_error: Option<std::io::Error>,
}

/// A TLS handshake in flight on a non-blocking socket (feature 04 §2.2).
///
/// WHY: driving the handshake lazily inside the first request write made the
/// handshake's wait use the body's idle deadline instead of the connect deadline.
/// This state drives `complete_io` until the handshake finishes or the socket
/// would block, and parks on the direction rustls asks for.
#[cfg(all(unix, feature = "ssl-rustls"))]
pub struct TlsHandshake<R: DnsResolver + Send + 'static> {
    exchange: PendingExchange<R>,
    conn: ClientSSLStream,
    /// Fixed handshake deadline (the connect deadline).
    deadline: Instant,
}

pub enum HttpRequestRedirectState<R: DnsResolver + Send + 'static> {
    Init(Option<InitData<R>>),
    Trying(Option<TryingData<R>>),
    /// Non-blocking: the TCP connect is in flight.
    #[cfg(unix)]
    Connecting(Option<Box<Connecting<R>>>),
    /// Non-blocking: the TLS handshake is in flight.
    #[cfg(all(unix, feature = "ssl-rustls"))]
    TlsHandshake(Option<Box<TlsHandshake<R>>>),
    /// Non-blocking: writing the request head.
    SendingHead(Option<Box<SendingHead<R>>>),
    /// Non-blocking: the `Expect: 100-continue` wait.
    AwaitingContinue(Option<Box<AwaitingContinue<R>>>),
    /// Reading a no-body request's response head (parks on non-blocking sockets).
    ReadingHead(Option<Box<ReadingHead<R>>>),
    /// Blocking transports: write the whole body in one go.
    WriteBody(Option<WriteBodyData<R>>),
    /// Non-blocking: streaming the request body.
    WritingBody(Option<Box<WritingBody>>),
    Done,
}

pub enum HttpRequestRedirectResponse {
    Done(
        HttpClientConnection,
        HttpResponseReader<SimpleHttpBody, RawStream>,
        Box<Option<[IncomingResponseParts; 2]>>,
    ),
    Error(HttpClientError),
    FlushFailed(HttpClientConnection, std::io::Error),
}

/// What a received response head means for redirect handling.
enum RedirectDecision {
    /// Not a followable redirect: hand the response on.
    NotRedirect,
    /// Re-issue the request to this descriptor.
    Follow(Box<RequestDescriptor>),
    /// A redirect that cannot be followed.
    Fail(HttpClientError),
}

fn redirect_decision(
    descriptor: &RequestDescriptor,
    config: &ClientConfig,
    remaining_redirects: u8,
    status: &Status,
    headers: &SimpleHeaders,
) -> RedirectDecision {
    let is_redirect = (300..400).contains(&status.clone().into_usize());
    let Some(location) = headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first()) else {
        return RedirectDecision::NotRedirect;
    };
    if !is_redirect {
        return RedirectDecision::NotRedirect;
    }
    if remaining_redirects == 0 {
        tracing::error!("Redirect limit exceeded");
        return RedirectDecision::Fail(HttpClientError::TooManyRedirects);
    }
    tracing::info!(
        "Redirect detected: status {} with Location {:?}",
        status,
        location
    );
    let new_url = match redirects::resolve_location(&descriptor.request_uri, location) {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Failed to resolve redirect location: {}", e);
            return RedirectDecision::Fail(HttpClientError::InvalidLocation(location.clone()));
        }
    };
    match redirects::build_followup_request_from_request_descriptor(
        descriptor,
        new_url,
        config.redirect.preserve_auth_on_redirect,
        config.redirect.preserve_cookies_on_redirect,
    ) {
        Ok(desc) => RedirectDecision::Follow(Box::new(desc)),
        Err(e) => {
            tracing::error!("Failed to build follow-up request: {}", e);
            RedirectDecision::Fail(HttpClientError::InvalidState)
        }
    }
}

/// Redirect-capable variant: small task wrapper that can be spawned in place of the
/// stream-only task to perform connect/send/probe/redirect-loop behavior.
/// It mirrors the shape of `GetHttpRequestStreamTask` so it can be constructed
/// from the same `GetHttpRequestStreamInner` data when needed.
pub struct GetHttpRequestRedirectTask<R: DnsResolver + Send + 'static>(
    Option<HttpRequestRedirectState<R>>,
);

impl<R: DnsResolver + Send + 'static> GetHttpRequestRedirectTask<R> {
    /// Create a new redirect-capable task from the provided inner data.
    #[must_use]
    pub fn new(
        data: SimpleIncomingRequest,
        pool: Arc<HttpConnectionPool<R>>,
        config: ClientConfig,
        max_redirects: u8,
    ) -> Self {
        Self(Some(HttpRequestRedirectState::Init(Some(Box::new((
            data,
            pool,
            config,
            max_redirects,
        ))))))
    }

    fn fail(&mut self, err: HttpClientError) -> Step {
        self.0 = Some(HttpRequestRedirectState::Done);
        TaskStatus::Ready(HttpRequestRedirectResponse::Error(err))
    }

    fn pending(&mut self, next: HttpRequestRedirectState<R>) -> Step {
        self.0 = Some(next);
        TaskStatus::Pending(HttpOperationState::Connecting)
    }

    fn follow(&mut self, exchange: Exchange<R>, descriptor: RequestDescriptor) -> Step {
        tracing::debug!("Following redirect to {}", descriptor.request_uri);
        let Exchange {
            data,
            pool,
            config,
            remaining_redirects,
            ..
        } = exchange;
        self.pending(HttpRequestRedirectState::Trying(Some(Box::new((
            data,
            pool,
            config,
            descriptor,
            remaining_redirects - 1,
        )))))
    }

    /// Park after a `WouldBlock`, storing `resume` as the next state.
    ///
    /// Terminal cases end the task (`Done`) instead of re-driving the I/O.
    /// One is a blocking socket whose kernel timeout already elapsed (the #76
    /// unbounded-retry hang). The other is an inactivity deadline that has passed.
    fn park(
        &mut self,
        connection: &HttpClientConnection,
        interest: IoInterest,
        idle: &mut IdleDeadline,
        resume: impl FnOnce(IdleDeadline) -> HttpRequestRedirectState<R>,
    ) -> Step {
        match nonblocking::park(connection, interest, idle) {
            Park::On(signal) => {
                tracing::trace!(%interest, "socket not ready; parking");
                self.0 = Some(resume(*idle));
                TaskStatus::Depends(signal)
            }
            Park::Expired => {
                tracing::error!(timeout = ?idle.timeout(), %interest, "no progress before the deadline");
                self.fail(HttpClientError::Timeout)
            }
            Park::Unsupported => {
                tracing::error!(%interest, "would block on a blocking transport; treating as timeout");
                self.fail(HttpClientError::Timeout)
            }
        }
    }

    /// Connect and send the request head; branch on blocking vs non-blocking.
    #[allow(clippy::too_many_lines)]
    fn trying(&mut self, state: TryingData<R>) -> Step {
        let (data, pool, config, descriptor, remaining_redirects) = *state;

        // Unix socket transport (Docker daemon, BuildKitd) bypasses
        // DNS and proxy — dial the configured socket directly (blocking).
        #[cfg(unix)]
        let unix_socket = config.unix_socket.clone();
        #[cfg(not(unix))]
        let unix_socket: Option<std::path::PathBuf> = None;

        if let Some(socket_path) = unix_socket {
            #[cfg(unix)]
            {
                let connection =
                    match pool.create_connection_unix(&descriptor.request_uri, &socket_path) {
                        Ok(c) => c,
                        Err(err) => {
                            tracing::error!("Failed to connect unix: {:?}", err);
                            return self.fail(HttpClientError::ConnectionError);
                        }
                    };
                let host = descriptor
                    .request_uri
                    .host_str()
                    .unwrap_or_else(|| "localhost".to_string());
                let pe = PendingExchange {
                    data,
                    pool,
                    config,
                    descriptor,
                    remaining_redirects,
                    host,
                    port: 0,
                    use_tls: false,
                };
                return self.finish_connected(pe, connection);
            }
            #[cfg(not(unix))]
            {
                let _ = socket_path;
                unreachable!("unix_socket is always None on non-unix targets")
            }
        }

        let target_host = match descriptor.request_uri.host_str() {
            Some(host) => host.to_string(),
            None => {
                tracing::error!("Missing host in request URI");
                return self.fail(HttpClientError::InvalidUrl("Missing host".to_string()));
            }
        };
        let target_port = descriptor.request_uri.port_or_default();
        let use_tls = descriptor.request_uri.scheme().is_https()
            || descriptor.request_uri.scheme().is_wss();

        // Determine effective proxy configuration.
        let env_proxy = if config.proxy_from_env {
            crate::shared::client::ProxyConfig::from_env(descriptor.request_uri.scheme())
        } else {
            None
        };
        let proxy_config = env_proxy.as_ref().or(config.proxy.as_ref());

        // A proxy is in the way unless NO_PROXY bypasses it for this host.
        let proxy_applies = proxy_config
            .is_some_and(|_| !crate::shared::client::ProxyConfig::should_bypass(&target_host));

        // Direct TCP (no proxy, no custom connector, no unix): non-blocking connect
        // (feature 04). TLS needs the rustls backend to drive the handshake without
        // blocking; other TLS backends keep the blocking path.
        #[cfg(unix)]
        let can_nonblocking = !proxy_applies
            && pool.connector.is_none()
            && (!use_tls || cfg!(feature = "ssl-rustls"));

        #[cfg(unix)]
        if can_nonblocking {
            return self.start_connecting(PendingExchange {
                data,
                pool,
                config,
                descriptor,
                remaining_redirects,
                host: target_host,
                port: target_port,
                use_tls,
            });
        }

        // Blocking fallback (proxy, custom connector, non-unix, non-rustls TLS).
        let connection = match pool.create_connection_with_proxy(
            &descriptor.request_uri,
            proxy_config,
            None,
        ) {
            Ok(connection) => connection,
            Err(err) => {
                tracing::error!("Failed to connect: {:?}", err);
                return self.fail(HttpClientError::ConnectionError);
            }
        };

        self.finish_connected(
            PendingExchange {
                data,
                pool,
                config,
                descriptor,
                remaining_redirects,
                host: target_host,
                port: target_port,
                use_tls,
            },
            connection,
        )
    }

    /// The post-connection half of `trying`: render the request head and branch to
    /// `SendingHead` (non-blocking) or `trying_blocking`.
    #[allow(clippy::too_many_lines)]
    fn finish_connected(
        &mut self,
        pe: PendingExchange<R>,
        connection: HttpClientConnection,
    ) -> Step {
        let PendingExchange {
            data,
            pool,
            config,
            mut descriptor,
            remaining_redirects,
            ..
        } = pe;

        // Only send Expect: 100-continue when the request has a body AND the
        // handshake is enabled in config. Bodyless requests (e.g. GET) must never
        // carry it — an interim `100 Continue` from the server would otherwise be
        // mistaken for the final response. The toggle lets callers disable the
        // handshake for servers/CDNs that handle it poorly.
        let has_body = !matches!(data.body, None | Some(SendSafeBody::None));
        let use_expect = has_body && config.expect_continue_enabled;
        if use_expect {
            descriptor
                .headers
                .insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);
        } else {
            descriptor.headers.remove(&SimpleHeader::EXPECT);
        }

        // RFC 7230 §3.3.1 — the body renderer frames all three streaming variants
        // (Stream/ChunkedStream/LineFeedStream) as chunked transfer encoding, so
        // the head MUST advertise `Transfer-Encoding: chunked` (and carry no
        // Content-Length). Without it a receiver finds neither framing header,
        // treats the request as bodyless, drops the chunked bytes, and mishandles
        // the Expect: 100-continue handshake above. The builders inject this too;
        // do it here to cover descriptors assembled by other paths.
        if matches!(
            data.body,
            Some(
                SendSafeBody::Stream(_)
                    | SendSafeBody::ChunkedStream(_)
                    | SendSafeBody::LineFeedStream(_)
            )
        ) {
            ensure_chunked_transfer_encoding(&mut descriptor.headers);
        }

        let Ok(request_string) =
            Http11::request_descriptor(descriptor.clone()).http_render_string()
        else {
            tracing::error!("Failed to render request");
            return self.fail(HttpClientError::InvalidState);
        };

        let next = match (has_body, use_expect) {
            (false, _) => AfterHead::ReadHead,
            (true, false) => AfterHead::WriteBody,
            (true, true) => AfterHead::AwaitContinue,
        };

        if connection.supports_nonblocking_io() {
            if let Err(err) = connection.set_nonblocking(true) {
                tracing::error!("Failed to make the connection non-blocking: {}", err);
                return self.fail(HttpClientError::IoError(err));
            }
            let reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                connection.clone_stream(),
                config.into_simple_http_body(),
            );
            let idle = IdleDeadline::new(config.io_idle_timeout());
            return self.pending(HttpRequestRedirectState::SendingHead(Some(Box::new(
                SendingHead {
                    exchange: Exchange {
                        data,
                        pool,
                        config,
                        descriptor,
                        remaining_redirects,
                        connection,
                        reader,
                    },
                    pending: PendingBytes::new(request_string.into_bytes()),
                    idle,
                    next,
                },
            ))));
        }

        self.trying_blocking(
            Exchange {
                reader: HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                    connection.clone_stream(),
                    config.into_simple_http_body(),
                ),
                data,
                pool,
                config,
                descriptor,
                remaining_redirects,
                connection,
            },
            &request_string,
            next,
        )
    }

    /// Begin a non-blocking direct connect: reuse a pooled connection if one is
    /// idle, otherwise resolve DNS (still blocking — out of scope) and enter
    /// [`Connecting`].
    #[cfg(unix)]
    fn start_connecting(&mut self, exchange: PendingExchange<R>) -> Step {
        if let Some(connection) = exchange
            .pool
            .checkout_connection(&exchange.host, exchange.port)
        {
            return self.finish_connected(exchange, connection);
        }

        let addrs = match exchange.pool.resolve_host(&exchange.host, exchange.port) {
            Ok(addrs) => addrs,
            Err(err) => {
                tracing::error!("Failed to resolve {}: {:?}", exchange.host, err);
                return self.fail(err);
            }
        };

        let deadline = Instant::now() + exchange.config.timeout_config().connect_timeout;

        self.0 = Some(HttpRequestRedirectState::Connecting(Some(Box::new(
            Connecting {
                exchange,
                addrs: addrs.into_iter(),
                current: None,
                deadline,
                last_error: None,
            },
        ))));
        TaskStatus::Pending(HttpOperationState::Connecting)
    }

    /// Drive the non-blocking connect across the resolved addresses (feature 04
    /// §2.1). Immediate failures try the next address; an in-flight connect parks
    /// on write readiness of the raw fd, bounded by the connect deadline.
    #[cfg(unix)]
    fn connecting(&mut self, mut st: Box<Connecting<R>>) -> Step {
        loop {
            let pending = match st.current.take() {
                Some(pending) => pending,
                None => {
                    let Some(addr) = st.addrs.next() else {
                        let error = st.last_error.take().map(HttpClientError::IoError).unwrap_or_else(
                            || {
                                HttpClientError::ConnectionFailed(format!(
                                    "Failed to connect to {}:{}",
                                    st.exchange.host, st.exchange.port
                                ))
                            },
                        );
                        return self.fail(error);
                    };
                    match PendingConnect::start(addr) {
                        Ok(pending) => pending,
                        Err(err) => {
                            // Immediate failure (no route, refused): try the next.
                            st.last_error = Some(err);
                            continue;
                        }
                    }
                }
            };

            // Zero-timeout poll: is the fd writable now?
            let writable = SocketReadiness::new(pending.as_raw_fd(), IoInterest::Writable)
                .poll_now()
                .unwrap_or(false);

            if writable {
                // The connect result comes from SO_ERROR, never "writable" alone.
                match pending.check() {
                    Ok(tcp) => {
                        let connection = Connection::Tcp(tcp);
                        return self.after_connect(st.exchange, connection, st.deadline);
                    }
                    Err(err) => {
                        st.last_error = Some(err);
                        continue;
                    }
                }
            }

            // Not writable yet: park, unless the deadline already passed.
            if Instant::now() >= st.deadline {
                return self.fail(HttpClientError::ConnectionTimeout(format!(
                    "Connection to {}:{} timed out",
                    st.exchange.host, st.exchange.port
                )));
            }
            st.current = Some(pending);
            let fd = st
                .current
                .as_ref()
                .expect("current was just set")
                .as_raw_fd();
            let io = std::sync::Arc::new(SocketReadiness::new(fd, IoInterest::Writable));
            let signal = io_or_deadline(io, st.deadline);
            self.0 = Some(HttpRequestRedirectState::Connecting(Some(st)));
            return TaskStatus::Depends(signal);
        }
    }

    /// A TCP connect finished: wrap the socket, or (for TLS) start the handshake.
    #[cfg(unix)]
    fn after_connect(
        &mut self,
        exchange: PendingExchange<R>,
        connection: Connection,
        deadline: Instant,
    ) -> Step {
        if !exchange.use_tls {
            let raw = match RawStream::from_connection(connection) {
                Ok(raw) => raw,
                Err(err) => return self.fail(HttpClientError::ConnectionFailed(err.to_string())),
            };
            let connection = HttpClientConnection::from_stream(
                SharedByteBufferStream::rwrite(raw),
                exchange.host.clone(),
                exchange.port,
            );
            return self.finish_connected(exchange, connection);
        }

        // TLS target: build the rustls stream and drive the handshake (feature 04
        // §2.2). This branch is only reached when `ssl-rustls` is enabled (see the
        // `can_nonblocking` gate in `trying`).
        #[cfg(feature = "ssl-rustls")]
        {
            let connector = exchange
                .pool
                .tls_connector()
                .cloned()
                .unwrap_or_else(|| Arc::new(SSLConnector::new()));
            let sni = exchange.host.clone();
            let (tls_stream, _addr) = match connector.from_tcp_stream(sni, connection) {
                Ok(parts) => parts,
                Err(err) => {
                    return self.fail(HttpClientError::TlsHandshakeFailed(err.to_string()));
                }
            };
            self.0 = Some(HttpRequestRedirectState::TlsHandshake(Some(Box::new(
                TlsHandshake {
                    exchange,
                    conn: tls_stream,
                    deadline,
                },
            ))));
            TaskStatus::Pending(HttpOperationState::Connecting)
        }
        #[cfg(not(feature = "ssl-rustls"))]
        {
            let _ = (exchange, connection, deadline);
            unreachable!("non-blocking TLS requires the ssl-rustls backend")
        }
    }

    /// Drive the rustls handshake on the non-blocking socket (feature 04 §2.2).
    /// Parks on the direction rustls asks for, bounded by the connect deadline.
    #[cfg(all(unix, feature = "ssl-rustls"))]
    fn tls_handshake(&mut self, st: Box<TlsHandshake<R>>) -> Step {
        match st.conn.drive_handshake() {
            Ok(()) => {
                let raw = match RawStream::from_client_tls(st.conn) {
                    Ok(raw) => raw,
                    Err(err) => {
                        return self.fail(HttpClientError::TlsHandshakeFailed(err.to_string()));
                    }
                };
                let connection = HttpClientConnection::from_stream(
                    SharedByteBufferStream::rwrite(raw),
                    st.exchange.host.clone(),
                    st.exchange.port,
                );
                self.finish_connected(st.exchange, connection)
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= st.deadline {
                    return self.fail(HttpClientError::ConnectionTimeout(format!(
                        "TLS handshake with {}:{} timed out",
                        st.exchange.host, st.exchange.port
                    )));
                }
                let Some(io) = st.conn.readiness(IoInterest::Readable) else {
                    tracing::error!("TLS handshake lost its readiness support");
                    return self.fail(HttpClientError::TlsHandshakeFailed(
                        "connection lost readiness support".to_string(),
                    ));
                };
                let signal = io_or_deadline(io, st.deadline);
                self.0 = Some(HttpRequestRedirectState::TlsHandshake(Some(st)));
                TaskStatus::Depends(signal)
            }
            Err(err) => {
                tracing::error!("TLS handshake failed: {}", err);
                self.fail(HttpClientError::TlsHandshakeFailed(err.to_string()))
            }
        }
    }

    /// The blocking path, for transports that cannot report readiness. Kernel read
    /// timeouts bound every wait, as before.
    fn trying_blocking(
        &mut self,
        mut exchange: Exchange<R>,
        request_string: &str,
        next: AfterHead,
    ) -> Step {
        if let Err(err) = exchange
            .connection
            .stream_mut()
            .write_all(request_string.as_bytes())
        {
            tracing::error!("Failed to write request: {}", err);
            return self.fail(HttpClientError::WriteFailed);
        }
        if let Err(err) = exchange.connection.stream_mut().flush() {
            tracing::error!("Failed to flush request: {}", err);
            return self.fail(HttpClientError::WriteFailed);
        }

        let read_timeout = exchange.config.get_expect_continue_read_timeout();
        if let Err(err) = exchange
            .connection
            .stream_mut()
            .set_read_timeout_as(read_timeout)
        {
            tracing::error!("Failed to set read timeout: {}", err);
            return self.fail(HttpClientError::Timeout);
        }

        match next {
            AfterHead::WriteBody => {
                let Exchange {
                    data,
                    pool,
                    connection,
                    reader,
                    ..
                } = exchange;
                self.pending(HttpRequestRedirectState::WriteBody(Some(Box::new((
                    None, data, pool, connection, reader,
                )))))
            }
            AfterHead::ReadHead => {
                let idle = IdleDeadline::new(exchange.config.response_head_timeout());
                self.pending(HttpRequestRedirectState::ReadingHead(Some(Box::new(
                    ReadingHead {
                        exchange,
                        intro: None,
                        idle,
                    },
                ))))
            }
            AfterHead::AwaitContinue => self.await_continue_blocking(exchange),
        }
    }

    /// Blocking `Expect: 100-continue`: one read bounded by the socket timeout.
    fn await_continue_blocking(&mut self, mut exchange: Exchange<R>) -> Step {
        tracing::trace!("[100-continue] blocking wait for the interim response");
        let intro_result = exchange.reader.next();
        let Some(Ok(IncomingResponseParts::Intro(status, proto, text))) = intro_result else {
            tracing::trace!("No interim response before the timeout; writing the body");
            return self.write_body_blocking(exchange, None);
        };
        let Some(Ok(IncomingResponseParts::Headers(headers))) = exchange.reader.next() else {
            tracing::error!("Headers not received, returning timeout");
            return self.fail(HttpClientError::Timeout);
        };

        if status == Status::Continue {
            return self.write_body_blocking(exchange, None);
        }

        let content_length = headers
            .get(&SimpleHeader::CONTENT_LENGTH)
            .and_then(|v| v.first())
            .and_then(|s| s.parse::<usize>().ok());
        let body_read_timeout = exchange
            .config
            .calculate_read_timeout(content_length, false);
        if let Err(err) = exchange
            .connection
            .stream_mut()
            .set_read_timeout_as(body_read_timeout)
        {
            tracing::error!("Failed to set body read timeout: {}", err);
            return self.fail(HttpClientError::IoError(err));
        }

        match redirect_decision(
            &exchange.descriptor,
            &exchange.config,
            exchange.remaining_redirects,
            &status,
            &headers,
        ) {
            RedirectDecision::Follow(descriptor) => self.follow(exchange, *descriptor),
            RedirectDecision::Fail(err) => self.fail(err),
            RedirectDecision::NotRedirect => self.write_body_blocking(
                exchange,
                Some([
                    IncomingResponseParts::Intro(status, proto, text),
                    IncomingResponseParts::Headers(headers),
                ]),
            ),
        }
    }

    fn write_body_blocking(
        &mut self,
        exchange: Exchange<R>,
        starters: Option<[IncomingResponseParts; 2]>,
    ) -> Step {
        let Exchange {
            data,
            pool,
            connection,
            reader,
            ..
        } = exchange;
        self.pending(HttpRequestRedirectState::WriteBody(Some(Box::new((
            starters, data, pool, connection, reader,
        )))))
    }

    /// Hand the exchange to the non-blocking body writer.
    fn start_writing_body(
        &mut self,
        exchange: Exchange<R>,
        starters: Option<[IncomingResponseParts; 2]>,
    ) -> Step {
        let Exchange {
            data,
            config,
            connection,
            reader,
            ..
        } = exchange;
        let idle = IdleDeadline::new(config.io_idle_timeout());
        // The pushable body's pipe readiness travels on the request; take it out
        // before the request is moved into the renderer, so the writer can park
        // on it when the producer stalls.
        let body_readiness = data.body_readiness.clone();
        self.pending(HttpRequestRedirectState::WritingBody(Some(Box::new(
            WritingBody {
                starters,
                connection,
                reader,
                chunks: Http11RequestBodyIterator::new(data),
                pending: PendingBytes::default(),
                rendered_all: false,
                idle,
                body_readiness,
            },
        ))))
    }

    fn sending_head(&mut self, mut st: Box<SendingHead<R>>) -> Step {
        let step = match st.pending.write_to(st.exchange.connection.stream_mut()) {
            Ok((moved, IoStep::Done)) => {
                if moved > 0 {
                    st.idle.progress();
                }
                nonblocking::flush(st.exchange.connection.stream_mut())
            }
            Ok((moved, IoStep::Blocked)) => {
                if moved > 0 {
                    st.idle.progress();
                }
                Ok(IoStep::Blocked)
            }
            Err(err) => Err(err),
        };

        match step {
            Ok(IoStep::Blocked) => {
                let connection = st.exchange.connection.clone();
                let mut idle = st.idle;
                self.park(&connection, IoInterest::Writable, &mut idle, move |idle| {
                    st.idle = idle;
                    HttpRequestRedirectState::SendingHead(Some(st))
                })
            }
            Err(err) => {
                tracing::error!("Failed to write request head: {}", err);
                self.fail(HttpClientError::WriteFailed)
            }
            Ok(IoStep::Done) => {
                let SendingHead { exchange, next, .. } = *st;
                tracing::trace!(?next, "request head sent");
                match next {
                    AfterHead::WriteBody => self.start_writing_body(exchange, None),
                    AfterHead::ReadHead => {
                        let idle = IdleDeadline::new(exchange.config.response_head_timeout());
                        self.pending(HttpRequestRedirectState::ReadingHead(Some(Box::new(
                            ReadingHead {
                                exchange,
                                intro: None,
                                idle,
                            },
                        ))))
                    }
                    AfterHead::AwaitContinue => {
                        let wait = exchange.config.get_expect_continue_read_timeout();
                        let idle = IdleDeadline::new(exchange.config.response_head_timeout());
                        self.pending(HttpRequestRedirectState::AwaitingContinue(Some(Box::new(
                            AwaitingContinue {
                                exchange,
                                send_body_at: Instant::now() + wait,
                                intro: None,
                                idle,
                            },
                        ))))
                    }
                }
            }
        }
    }

    /// The `Expect: 100-continue` wait as a readiness wait with a timer.
    fn awaiting_continue(&mut self, mut st: Box<AwaitingContinue<R>>) -> Step {
        if st.intro.is_none() {
            return match st.exchange.reader.next() {
                Some(Err(ref e)) if e.is_would_block() => {
                    if Instant::now() >= st.send_body_at {
                        // RFC 9110 §10.1.1: a client that sent `Expect: 100-continue`
                        // need not wait indefinitely; it sends the body after a
                        // reasonable wait.
                        tracing::trace!("[100-continue] timer elapsed; sending the body");
                        let AwaitingContinue { exchange, .. } = *st;
                        return self.start_writing_body(exchange, None);
                    }
                    self.park_for_continue(st)
                }
                Some(Ok(intro @ IncomingResponseParts::Intro(..))) => {
                    st.idle.progress();
                    st.intro = Some(intro);
                    self.pending(HttpRequestRedirectState::AwaitingContinue(Some(st)))
                }
                Some(Ok(IncomingResponseParts::SKIP)) => {
                    st.idle.progress();
                    self.pending(HttpRequestRedirectState::AwaitingContinue(Some(st)))
                }
                Some(Ok(other)) => {
                    tracing::error!("[100-continue] expected a status line, got {:?}", other);
                    self.fail(HttpClientError::ReaderError(
                        crate::shared::http::HttpReaderError::ReadFailed,
                    ))
                }
                Some(Err(err)) => {
                    tracing::error!(
                        "[100-continue] reading the interim response failed: {:?}",
                        err
                    );
                    self.fail(HttpClientError::ReaderError(err))
                }
                None => {
                    tracing::error!(
                        "[100-continue] server closed the connection before responding"
                    );
                    self.fail(HttpClientError::ConnectionError)
                }
            };
        }

        // A status line arrived; its headers are on their way.
        let headers = match st.exchange.reader.next() {
            Some(Ok(IncomingResponseParts::Headers(h))) => h,
            Some(Err(ref e)) if e.is_would_block() => {
                let connection = st.exchange.connection.clone();
                let mut idle = st.idle;
                return self.park(&connection, IoInterest::Readable, &mut idle, move |idle| {
                    st.idle = idle;
                    HttpRequestRedirectState::AwaitingContinue(Some(st))
                });
            }
            other => {
                tracing::error!(
                    "[100-continue] headers not received: {:?}",
                    other.map(|r| r.map(|_| ()))
                );
                return self.fail(HttpClientError::Timeout);
            }
        };

        let Some(IncomingResponseParts::Intro(status, proto, text)) = st.intro.take() else {
            unreachable!("AwaitingContinue::intro only ever holds an Intro")
        };

        if status == Status::Continue {
            tracing::trace!("[100-continue] got 100 Continue; sending the body");
            let AwaitingContinue { exchange, .. } = *st;
            return self.start_writing_body(exchange, None);
        }

        let code = status.clone().into_usize();
        if (100..200).contains(&code) && code != 101 {
            // Another interim response (102, 103): skip it and keep waiting for
            // `100` or the final response, on the same timer.
            tracing::trace!("[100-continue] skipping interim {}", status);
            st.exchange.reader = st.exchange.reader.branch_reader();
            st.idle.progress();
            return self.pending(HttpRequestRedirectState::AwaitingContinue(Some(st)));
        }

        // A final response before the body (e.g. 417, 4xx, a redirect).
        let AwaitingContinue { exchange, .. } = *st;
        match redirect_decision(
            &exchange.descriptor,
            &exchange.config,
            exchange.remaining_redirects,
            &status,
            &headers,
        ) {
            RedirectDecision::Follow(descriptor) => self.follow(exchange, *descriptor),
            RedirectDecision::Fail(err) => self.fail(err),
            RedirectDecision::NotRedirect => self.start_writing_body(
                exchange,
                Some([
                    IncomingResponseParts::Intro(status, proto, text),
                    IncomingResponseParts::Headers(headers),
                ]),
            ),
        }
    }

    /// Park the 100-continue wait on "readable, or the send-body timer fired".
    fn park_for_continue(&mut self, st: Box<AwaitingContinue<R>>) -> Step {
        let Some(io) = st.exchange.connection.park_readiness(IoInterest::Readable) else {
            tracing::error!("[100-continue] connection lost its readiness support");
            return self.fail(HttpClientError::Timeout);
        };
        let timer = Arc::new(DeadlineReadiness::at(st.send_body_at));
        self.0 = Some(HttpRequestRedirectState::AwaitingContinue(Some(st)));
        TaskStatus::Depends(Arc::new(AnyReadiness::either(io, timer)))
    }

    fn reading_head(&mut self, mut st: Box<ReadingHead<R>>) -> Step {
        // Step 1: read the final status line, skipping interim 1xx responses
        // (RFC 9110 §15.2). `intro` remembers a final status already read so a
        // park between the status line and the headers is re-entrant.
        if st.intro.is_none() {
            match st.exchange.reader.next() {
                Some(Err(ref e)) if e.is_would_block() => return self.park_head(st),
                Some(Ok(IncomingResponseParts::Intro(status, proto, text))) => {
                    st.idle.progress();
                    let code = status.clone().into_usize();
                    if (100..200).contains(&code) && code != 101 {
                        // Interim: consume its (empty) head, then branch and re-read.
                        match st.exchange.reader.next() {
                            Some(Err(ref e)) if e.is_would_block() => {
                                // Its headers are still in flight. Resume reading them
                                // (step 2 sees them and branches) on the next poll.
                                return self.park_head(st);
                            }
                            Some(Err(e)) => {
                                tracing::error!("ReadingHead: interim head failed: {:?}", e);
                                return self.fail(HttpClientError::Timeout);
                            }
                            _ => {}
                        }
                        st.exchange.reader = st.exchange.reader.branch_reader();
                        return self.pending(HttpRequestRedirectState::ReadingHead(Some(st)));
                    }
                    st.intro = Some(IncomingResponseParts::Intro(status, proto, text));
                }
                Some(Ok(IncomingResponseParts::Headers(_))) => {
                    // Mid-interim after a park: these are the interim's headers.
                    // Branch and re-read the next status line.
                    st.idle.progress();
                    st.exchange.reader = st.exchange.reader.branch_reader();
                    return self.pending(HttpRequestRedirectState::ReadingHead(Some(st)));
                }
                Some(Ok(IncomingResponseParts::SKIP)) => {
                    return self.pending(HttpRequestRedirectState::ReadingHead(Some(st)));
                }
                Some(Ok(_) | Err(_)) | None => {
                    tracing::error!("ReadingHead: no intro received");
                    return self.fail(HttpClientError::Timeout);
                }
            }
        }

        // Step 2: read headers (parks on `WouldBlock`).
        let headers = match st.exchange.reader.next() {
            Some(Ok(IncomingResponseParts::Headers(h))) => h,
            Some(Err(ref e)) if e.is_would_block() => return self.park_head(st),
            _ => {
                tracing::error!("ReadingHead: headers not received");
                return self.fail(HttpClientError::Timeout);
            }
        };

        let Some(intro) = st.intro.take() else {
            unreachable!("ReadingHead::intro is set before headers are read")
        };
        let IncomingResponseParts::Intro(ref status, _, _) = intro else {
            unreachable!("ReadingHead::intro only ever holds an Intro")
        };

        let ReadingHead { exchange, .. } = *st;
        match redirect_decision(
            &exchange.descriptor,
            &exchange.config,
            exchange.remaining_redirects,
            status,
            &headers,
        ) {
            RedirectDecision::Follow(descriptor) => self.follow(exchange, *descriptor),
            RedirectDecision::Fail(err) => self.fail(err),
            RedirectDecision::NotRedirect => {
                tracing::debug!("No redirect for no-body request — handing off connection");
                self.0 = Some(HttpRequestRedirectState::Done);
                TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                    exchange.connection,
                    exchange.reader,
                    Box::new(Some([intro, IncomingResponseParts::Headers(headers)])),
                ))
            }
        }
    }

    fn park_head(&mut self, mut st: Box<ReadingHead<R>>) -> Step {
        let connection = st.exchange.connection.clone();
        let mut idle = st.idle;
        self.park(&connection, IoInterest::Readable, &mut idle, move |idle| {
            st.idle = idle;
            HttpRequestRedirectState::ReadingHead(Some(st))
        })
    }

    fn writing_body(&mut self, mut st: Box<WritingBody>) -> Step {
        for _ in 0..MAX_CHUNKS_PER_POLL {
            if !st.pending.is_empty() {
                match st.pending.write_to(st.connection.stream_mut()) {
                    Ok((moved, step)) => {
                        if moved > 0 {
                            st.idle.progress();
                        }
                        if step == IoStep::Blocked {
                            return self.park_body(st);
                        }
                    }
                    Err(err) => {
                        tracing::error!("Failed to write request body: {}", err);
                        return self.fail(HttpClientError::WriteFailed);
                    }
                }
            }

            if st.rendered_all {
                return match nonblocking::flush(st.connection.stream_mut()) {
                    Ok(IoStep::Done) => {
                        tracing::trace!("request body written and flushed");
                        self.0 = Some(HttpRequestRedirectState::Done);
                        let WritingBody {
                            starters,
                            connection,
                            reader,
                            ..
                        } = *st;
                        TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                            connection,
                            reader,
                            Box::new(starters),
                        ))
                    }
                    Ok(IoStep::Blocked) => self.park_body(st),
                    Err(err) => {
                        tracing::error!("Failed to flush request body: {}", err);
                        self.0 = Some(HttpRequestRedirectState::Done);
                        TaskStatus::Ready(HttpRequestRedirectResponse::FlushFailed(
                            st.connection,
                            err,
                        ))
                    }
                };
            }

            match st.chunks.next() {
                Some(Ok(bytes)) if bytes.is_empty() => {
                    // A state transition in the renderer, or a pushable body whose
                    // producer has nothing yet. Push out what was written so the
                    // server sees it, then wait for more.
                    match nonblocking::flush(st.connection.stream_mut()) {
                        Ok(IoStep::Done) => {}
                        Ok(IoStep::Blocked) => return self.park_body(st),
                        Err(err) => {
                            tracing::error!("Failed to flush request body: {}", err);
                            return self.fail(HttpClientError::WriteFailed);
                        }
                    }

                    // A pushable body parks on its pipe readiness, bounded by the
                    // idle deadline: the task wakes the moment the producer pushes
                    // (or the pipe closes), with no back-off timer.
                    if let Some(readiness) = st.body_readiness.clone() {
                        if readiness.is_ready(None) {
                            // Data is already queued (or the pipe closed): retry
                            // at once instead of returning an already-ready signal.
                            return self.pending(HttpRequestRedirectState::WritingBody(Some(st)));
                        }
                        if st.idle.expired() {
                            tracing::error!("request body producer stalled past the idle deadline");
                            return self.fail(HttpClientError::Timeout);
                        }
                        let deadline = st.idle.arm();
                        self.0 = Some(HttpRequestRedirectState::WritingBody(Some(st)));
                        return TaskStatus::Depends(io_or_deadline(readiness, deadline));
                    }

                    // No pipe readiness: a one-shot renderer transition (chunked
                    // trailers, …). Retry at once.
                    return self.pending(HttpRequestRedirectState::WritingBody(Some(st)));
                }
                Some(Ok(bytes)) => {
                    st.pending = PendingBytes::new(bytes);
                }
                Some(Err(err)) => {
                    tracing::error!("Failed to render request body: {}", err);
                    return self.fail(HttpClientError::WriteFailed);
                }
                None => st.rendered_all = true,
            }
        }
        self.pending(HttpRequestRedirectState::WritingBody(Some(st)))
    }

    fn park_body(&mut self, mut st: Box<WritingBody>) -> Step {
        let connection = st.connection.clone();
        let mut idle = st.idle;
        self.park(&connection, IoInterest::Writable, &mut idle, move |idle| {
            st.idle = idle;
            HttpRequestRedirectState::WritingBody(Some(st))
        })
    }
}

impl<R: DnsResolver + Send + 'static> TaskIterator for GetHttpRequestRedirectTask<R> {
    type Pending = HttpOperationState;
    type Ready = HttpRequestRedirectResponse;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let trace_span = tracing::info_span!("GetHttpRequestRedirectTask");
        trace_span.in_scope(|| match self.0.take()? {
            HttpRequestRedirectState::Init(mut inner_opt) => {
                tracing::trace!("HttpRequestRedirectState::Init");
                let Some(inner) = inner_opt.take() else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return None;
                };
                let (data, pool, config, remaining_redirects) = *inner;
                let request_descriptor = data.descriptor();
                tracing::info!(
                    "HttpRequestRedirectState::Init -> Connecting to URL: {} with headers: {:?}",
                    &request_descriptor.request_uri,
                    crate::shared::http::RedactedHeaders(&request_descriptor.headers)
                );
                Some(
                    self.pending(HttpRequestRedirectState::Trying(Some(Box::new((
                        data,
                        pool,
                        config,
                        request_descriptor,
                        remaining_redirects,
                    ))))),
                )
            }
            HttpRequestRedirectState::Trying(inner_opt) => {
                tracing::trace!("HttpRequestRedirectState::Trying");
                let Some(state) = inner_opt else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return None;
                };
                Some(self.trying(state))
            }
            HttpRequestRedirectState::SendingHead(inner) => {
                tracing::trace!("HttpRequestRedirectState::SendingHead");
                Some(self.sending_head(inner?))
            }
            HttpRequestRedirectState::AwaitingContinue(inner) => {
                tracing::trace!("HttpRequestRedirectState::AwaitingContinue");
                Some(self.awaiting_continue(inner?))
            }
            HttpRequestRedirectState::ReadingHead(inner) => {
                tracing::trace!("HttpRequestRedirectState::ReadingHead");
                Some(self.reading_head(inner?))
            }
            HttpRequestRedirectState::WritingBody(inner) => {
                tracing::trace!("HttpRequestRedirectState::WritingBody");
                Some(self.writing_body(inner?))
            }
            HttpRequestRedirectState::WriteBody(mut inner_opt) => {
                tracing::trace!("HttpRequestRedirectState::WriteBody (blocking)");
                let Some(inner) = inner_opt.take() else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Pending(HttpOperationState::Done));
                };
                let (optional_starters, data, _pool, mut connection, reader) = *inner;
                self.0 = Some(HttpRequestRedirectState::Done);

                // Guard: no body to write — skip rendering and go straight to Done.
                if matches!(data.body, None | Some(SendSafeBody::None)) {
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                        connection,
                        reader,
                        Box::new(optional_starters),
                    )));
                }

                let body_renderer = Http11::request_body(data);
                if let Err(err) = body_renderer.http_render_to_writer(connection.stream_mut()) {
                    tracing::error!("Failed to write request body: {}", err);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        HttpClientError::WriteFailed,
                    )));
                }

                Some(match connection.stream_mut().flush() {
                    Ok(()) => TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                        connection,
                        reader,
                        Box::new(optional_starters),
                    )),
                    Err(e) => {
                        tracing::trace!("WriteBody: failed to flush body: {:?}", e);
                        TaskStatus::Ready(HttpRequestRedirectResponse::FlushFailed(connection, e))
                    }
                })
            }
            #[cfg(unix)]
            HttpRequestRedirectState::Connecting(inner_opt) => {
                tracing::trace!("HttpRequestRedirectState::Connecting");
                let Some(state) = inner_opt else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return None;
                };
                Some(self.connecting(state))
            }
            #[cfg(all(unix, feature = "ssl-rustls"))]
            HttpRequestRedirectState::TlsHandshake(inner_opt) => {
                tracing::trace!("HttpRequestRedirectState::TlsHandshake");
                let Some(state) = inner_opt else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return None;
                };
                Some(self.tls_handshake(state))
            }
            HttpRequestRedirectState::Done => None,
        })
    }
}
