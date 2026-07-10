//! Native HTTP/3 `Transport` implementation (Feature 35).
//!
//! WHY: the third transport, closing the any-protocol-on-any-transport matrix for
//! QUIC. Connect, gRPC and gRPC-Web all run over it unchanged, because HTTP/3
//! carries the same pseudo-headers and the same trailing HEADERS that gRPC's
//! status needs.
//!
//! WHAT: [`H3Transport`] — a factory. [`H3Pump`] is the valtron `TaskIterator`
//! that owns the QUIC driver, the HTTP/3 connection, and the request stream.
//!
//! HOW: mirrors `H2Transport`. `open()` performs the QUIC handshake on the
//! caller's thread (it must complete before the pump can send anything), then
//! spawns the pump and hands back the four `TransportStream` halves.
//!
//! ## Why one task drives both QUIC and HTTP/3
//!
//! `quinn-proto` is sans-IO: somebody must move datagrams. `QuicDriver` is itself
//! a `TaskIterator`, so the pump steps it once per poll before doing HTTP/3 work.
//! Splitting them into two tasks would need the driver and the request handles to
//! coordinate wake-ups; stepping the driver inline costs one call and keeps the
//! whole exchange in a single scheduling unit.
//!
//! ## Full duplex is real here
//!
//! Every poll advances the request direction *and* the response direction. A bidi
//! call therefore makes progress in both at once, which is what
//! `TransportCapabilities::full_duplex` promises — and, unlike HTTP/1.1, HTTP/3
//! has independent flow control per direction so neither starves the other.

use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::{
    self, BoxedSendExecutionAction, Pipe, PipeReceiver, PipeSender, Stream as VStream, TaskIterator,
    TaskStatus, TryRecvError,
};
use foundation_netio::http3::connection::{error_code, H3Connection, H3Request};
use foundation_netio::http3::H3Error;
use foundation_netio::quic::{ClientConfig, QuicDriver, QuinnBidiStream, QuinnConnection};
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, Status,
};

use super::{
    body_stream_from_pipe, head_stream_from_pipe, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

/// How long the pump yields for when neither direction can make progress.
const POLL_DELAY: Duration = Duration::from_millis(1);

/// HTTP/3 over QUIC — one connection per call.
#[derive(Clone)]
pub struct H3Transport {
    client_config: ClientConfig,
    /// The name the server certificate is validated against.
    server_name: Option<String>,
}

impl H3Transport {
    /// WHY: QUIC is always encrypted. There is no cleartext HTTP/3, so unlike
    /// `H2Transport` this cannot be constructed without a TLS configuration.
    ///
    /// WHAT: an HTTP/3 transport using `client_config` for every connection.
    ///
    /// HOW: pair with [`foundation_netio::quic::client_config_trusting_pem`] for a
    /// pinned certificate.
    #[must_use]
    pub fn new(client_config: ClientConfig) -> Self {
        Self { client_config, server_name: None }
    }

    /// Validate the server certificate against `name` rather than the URI host.
    #[must_use]
    pub fn with_server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = Some(name.into());
        self
    }
}

impl Transport for H3Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            // Every poll advances both directions, and QUIC gives each its own
            // flow-control window.
            full_duplex: true,
            // HTTP/3's trailers are a second HEADERS frame — the same capability
            // gRPC needs from h2's trailing HEADERS.
            h2_trailers: true,
            http_versions: &[Proto::HTTP30],
            // One QUIC connection per call today. HTTP/3 multiplexes streams
            // natively, so this becomes `true` once the client pools connections.
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let host = request.request_uri.host_str().unwrap_or_else(|| "localhost".into());
        let port = request.request_uri.port_or_default();
        let path = request.request_uri.path().to_string();
        // QUIC is always encrypted, so an HTTP/3 request is always `https`.
        let scheme = request.request_uri.scheme().to_string();
        let method = request.method.to_string();
        let req_headers = request.headers.clone();

        let addr: SocketAddr = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| TransportError::Connect(Arc::new(e)))?
            .next()
            .ok_or_else(|| {
                TransportError::Connect(Arc::new(io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    format!("no address for {host}:{port}"),
                )))
            })?;

        let server_name = self.server_name.clone().unwrap_or_else(|| host.clone());
        let (driver, quic_conn) =
            QuicDriver::connect(addr, self.client_config.clone(), &server_name)
                .map_err(|e| TransportError::Connect(Arc::new(e)))?;

        let (send_tx, send_rx) = Pipe::<Bytes>::new();
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::new();
        let (body_tx, body_rx) = Pipe::<Bytes>::new();
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);

        let pump = H3Pump {
            driver,
            conn: H3Connection::new(quic_conn),
            req: None,
            phase: Phase::Setup,
            pending_write: Bytes::new(),
            send_rx,
            head_tx,
            body_tx,
            trailer_tx,
            authority: format!("{host}:{port}"),
            scheme,
            method,
            path,
            req_headers,
            request_ended: false,
            head_delivered: false,
            pending_body: VecDeque::new(),
            response_ended: false,
        };

        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Arc::new(io::Error::other(e.to_string())))
        })?;

        Ok(TransportStream {
            send_body: Arc::new(send_tx),
            head: head_stream_from_pipe(head_rx),
            recv_body: body_stream_from_pipe(body_rx),
            trailers: trailer_rx,
        })
    }
}

/// Where the pump is in the exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Opening the control stream and sending SETTINGS.
    Setup,
    /// Opening the request's bidirectional stream.
    OpenRequest,
    /// Flushing the request HEADERS frame.
    SendHeaders,
    /// Both directions live.
    Streaming,
    /// Everything delivered.
    Done,
}

/// Drives QUIC, HTTP/3, and the four pipes of one exchange.
struct H3Pump {
    driver: QuicDriver,
    conn: H3Connection<QuinnConnection>,
    req: Option<H3Request<QuinnBidiStream>>,
    phase: Phase,
    /// Bytes of the current frame still to be written.
    pending_write: Bytes,

    send_rx: PipeReceiver<Bytes>,
    head_tx: PipeSender<(Status, SimpleHeaders)>,
    body_tx: PipeSender<Bytes>,
    trailer_tx: PipeSender<SimpleHeaders>,

    authority: String,
    scheme: String,
    method: String,
    path: String,
    req_headers: SimpleHeaders,

    /// `send_rx` closed and the FIN has gone out.
    request_ended: bool,
    /// The response head has been pushed to `head_tx`.
    head_delivered: bool,
    /// Response chunks the body pipe was too full to accept. They must be
    /// delivered in order: dropping a `Full` chunk silently truncates the body.
    pending_body: VecDeque<Bytes>,
    /// The response stream ended; close `body_tx` once `pending_body` drains.
    response_ended: bool,
}

impl H3Pump {
    /// The request's field section, pseudo-headers first (RFC 9114 §4.3).
    fn request_fields(&self) -> Vec<(Bytes, Bytes)> {
        let mut fields = vec![
            (Bytes::from_static(b":method"), Bytes::from(self.method.clone())),
            (Bytes::from_static(b":scheme"), Bytes::from(self.scheme.clone())),
            (Bytes::from_static(b":authority"), Bytes::from(self.authority.clone())),
            (Bytes::from_static(b":path"), Bytes::from(self.path.clone())),
        ];

        for (name, values) in &self.req_headers {
            // §4.1.1: lowercase on the wire. §4.2 forbids the connection-specific
            // fields; `response_to_fields` drops them on the way out and the same
            // rule applies to a request we originate.
            let lowered = name.to_string().to_ascii_lowercase();
            if matches!(
                lowered.as_str(),
                "connection" | "keep-alive" | "proxy-connection" | "transfer-encoding" | "upgrade"
            ) {
                continue;
            }
            for value in values {
                fields.push((Bytes::from(lowered.clone()), Bytes::from(value.clone())));
            }
        }
        fields
    }

    /// Flush `pending_write`. `true` when it is empty.
    fn flush(&mut self) -> Result<bool, H3Error> {
        if self.pending_write.is_empty() {
            return Ok(true);
        }
        let req = self.req.as_mut().expect("request open");
        match req.poll_send_data(&mut self.pending_write) {
            VStream::Next(Ok(())) => Ok(true),
            VStream::Next(Err(e)) => Err(e),
            _ => Ok(false),
        }
    }

    /// Move request bytes out. Returns `Err` on a fatal stream error.
    fn pump_request(&mut self) -> Result<(), H3Error> {
        if self.request_ended || !self.flush()? {
            return Ok(());
        }

        loop {
            match self.send_rx.try_recv() {
                Ok(chunk) => {
                    if chunk.is_empty() {
                        continue;
                    }
                    self.pending_write = H3Request::<QuinnBidiStream>::encode_data(chunk);
                    if !self.flush()? {
                        return Ok(());
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                // The caller finished the request body.
                Err(TryRecvError::Closed) => {
                    let req = self.req.as_mut().expect("request open");
                    return match req.poll_finish() {
                        VStream::Next(Ok(())) => {
                            self.request_ended = true;
                            Ok(())
                        }
                        VStream::Next(Err(e)) => Err(e),
                        _ => Ok(()),
                    };
                }
            }
        }
    }

    /// Move response bytes in. Returns `Err` on a fatal stream error.
    fn pump_response(&mut self) -> Result<(), H3Error> {
        let req = self.req.as_mut().expect("request open");

        if !self.head_delivered {
            match req.poll_headers() {
                VStream::Next(Ok(fields)) => {
                    let (status, headers) = split_response_fields(&fields);
                    // A full head pipe would mean nobody is reading the response;
                    // there is exactly one head, so a blocking send is wrong here —
                    // retry next poll instead.
                    if self.head_tx.try_send((status, headers)).is_ok() {
                        self.head_delivered = true;
                    }
                    return Ok(());
                }
                VStream::Next(Err(e)) => return Err(e),
                _ => return Ok(()),
            }
        }

        // Deliver anything the body pipe refused last time, in order.
        while let Some(front) = self.pending_body.front().cloned() {
            if self.body_tx.try_send(front).is_ok() {
                self.pending_body.pop_front();
            } else {
                return Ok(());
            }
        }

        if self.response_ended {
            return Ok(());
        }

        match req.poll_body() {
            VStream::Next(Ok(Some(chunk))) => {
                if self.body_tx.try_send(chunk.clone()).is_err() {
                    self.pending_body.push_back(chunk);
                }
                Ok(())
            }
            VStream::Next(Ok(None)) => {
                self.response_ended = true;
                // HTTP/3 trailers are a second HEADERS frame. gRPC's status is
                // there, so it must reach the caller.
                if let Some(trailers) = req.trailers() {
                    let mut headers = SimpleHeaders::new();
                    for (name, value) in trailers {
                        headers
                            .entry(SimpleHeader::from(
                                String::from_utf8_lossy(name).to_string(),
                            ))
                            .or_default()
                            .push(String::from_utf8_lossy(value).to_string());
                    }
                    let _ = self.trailer_tx.try_send(headers);
                }
                Ok(())
            }
            VStream::Next(Err(e)) => Err(e),
            _ => Ok(()),
        }
    }

    /// Tear the exchange down, reporting `error` to whoever is still listening.
    fn fail(&mut self, error: &H3Error) {
        tracing::debug!(error = %error, "HTTP/3 exchange failed");
        if let Some(req) = self.req.as_mut() {
            req.reset(error_code::H3_NO_ERROR);
        }
        self.head_tx.close();
        self.body_tx.close();
        self.trailer_tx.close();
        self.phase = Phase::Done;
    }

    /// Everything has been delivered; close the pipes.
    fn finish(&mut self) {
        self.body_tx.close();
        self.head_tx.close();
        self.trailer_tx.close();
        self.conn.close(error_code::H3_NO_ERROR, b"done");
        self.phase = Phase::Done;
    }
}

/// Split a response field section into a `Status` and the regular headers.
fn split_response_fields(fields: &[(Bytes, Bytes)]) -> (Status, SimpleHeaders) {
    let mut status = Status::OK;
    let mut headers = SimpleHeaders::new();

    for (name, value) in fields {
        if &name[..] == b":status" {
            status = Status::from(String::from_utf8_lossy(value).to_string());
            continue;
        }
        headers
            .entry(SimpleHeader::from(String::from_utf8_lossy(name).to_string()))
            .or_default()
            .push(String::from_utf8_lossy(value).to_string());
    }

    (status, headers)
}

impl TaskIterator for H3Pump {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.phase == Phase::Done {
            return None;
        }

        // `quinn-proto` is sans-IO: nothing moves unless the driver runs.
        let _ = self.driver.next_status();

        match self.phase {
            Phase::Setup => match self.conn.poll_setup() {
                VStream::Next(Ok(())) => self.phase = Phase::OpenRequest,
                VStream::Next(Err(e)) => {
                    self.fail(&e);
                    return None;
                }
                _ => return Some(TaskStatus::Delayed(POLL_DELAY)),
            },

            Phase::OpenRequest => match self.conn.poll_open_request() {
                VStream::Next(Ok(req)) => {
                    let fields = self.request_fields();
                    let pairs: Vec<(&[u8], &[u8])> =
                        fields.iter().map(|(n, v)| (&n[..], &v[..])).collect();
                    self.pending_write = H3Request::<QuinnBidiStream>::encode_headers(&pairs);
                    self.req = Some(req);
                    self.phase = Phase::SendHeaders;
                }
                VStream::Next(Err(e)) => {
                    self.fail(&e);
                    return None;
                }
                _ => return Some(TaskStatus::Delayed(POLL_DELAY)),
            },

            Phase::SendHeaders => match self.flush() {
                Ok(true) => self.phase = Phase::Streaming,
                Ok(false) => return Some(TaskStatus::Delayed(POLL_DELAY)),
                Err(e) => {
                    self.fail(&e);
                    return None;
                }
            },

            Phase::Streaming => {
                // Both directions, every poll: that is what full duplex means.
                if let Err(e) = self.pump_request() {
                    self.fail(&e);
                    return None;
                }
                if let Err(e) = self.pump_response() {
                    self.fail(&e);
                    return None;
                }

                if self.response_ended && self.pending_body.is_empty() && self.head_delivered {
                    self.finish();
                    return None;
                }
            }

            Phase::Done => return None,
        }

        Some(TaskStatus::Delayed(POLL_DELAY))
    }
}
