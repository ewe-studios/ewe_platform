//! WebSocket client [`TaskIterator`](crate::valtron::TaskIterator) implementation.
//!
//! WHY: Clients need a non-blocking, state-machine-based WebSocket consumer that
//! integrates with the valtron executor system. Enables async-like event handling
//! without async/await.
//!
//! WHAT: Implements [`WebSocketTask`] which processes WebSocket connections through
//! a series of states (connecting, handshake, open, closed). Uses `TaskIterator`
//! trait to yield `TaskStatus` variants for each WebSocket message.
//!
//! HOW: State machine where each `next()` call advances through states.
//! Uses `HttpConnectionPool` for connection management with pooling support.
//! Uses WebSocket frame decoding for message parsing.

use crate::io::ioutils::ReadTimeoutOperations;
use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::client::HttpClientConnection;
use crate::wire::simple_http::client::HttpConnectionPool;
use crate::wire::simple_http::url::Uri;
use crate::wire::simple_http::{
    Http11, HttpResponseReader, RenderHttp, SimpleHeader, SimpleHttpBody, Status,
};
use concurrent_queue::ConcurrentQueue;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, instrument, trace, warn};

use super::error::WebSocketError;
use super::frame::{generate_mask, Opcode, WebSocketFrame};
use super::handshake::{build_upgrade_request, compute_accept_key, generate_websocket_key};
use super::message::WebSocketMessage;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);
const SLEEP_BETWEEN_WORK: Duration = Duration::from_millis(15);

/// [`WebSocketProgress`] indicates the current state of WebSocket connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketProgress {
    Connecting,
    Handshaking,
    Reading,
}

/// [`WebSocketConnectInfo`] holds the configuration for a WebSocket connection.
pub struct WebSocketConnectInfo {
    pub url: String,
    pub subprotocols: Option<String>,
    pub extra_headers: Vec<(SimpleHeader, String)>,
    pub delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
    pub read_timeout: Duration,
}

/// Open state data for WebSocket connection.
///
/// WHY: Extract Open state data into a separate struct to avoid cloning
/// when transitioning Open→Open. Uses Option.take() for zero-copy moves.
///
/// Added buffer pool and reusable buffer for zero-copy frame parsing.
pub struct WebSocketOpenState {
    pub stream: crate::io::ioutils::SharedByteBufferStream<RawStream>,
    pub delivery_queue: Arc<ConcurrentQueue<WebSocketMessage>>,
    pub read_timeout: Duration,
    pub assembler: super::assembler::MessageAssembler,
    /// Buffer pool for zero-copy frame reading
    pub buffer_pool: Arc<crate::io::buffer_pool::BytesPool>,
    /// Reusable buffer for frame payload reading
    pub frame_buffer: bytes::BytesMut,
}

/// Connecting state data.
pub struct WebSocketConnectingState {
    pub url: Uri,
    pub ws_key: String,
    pub subprotocols: Option<String>,
    pub extra_headers: Vec<(SimpleHeader, String)>,
    pub delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
    pub read_timeout: Duration,
}

/// HandshakeSending state data.
pub struct WebSocketHandshakeSendingState {
    pub connection: HttpClientConnection,
    pub request_bytes: Vec<Vec<u8>>,
    pub current_chunk: usize,
    pub ws_key: String,
    pub subprotocols: Option<String>,
    pub delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
    pub read_timeout: Duration,
}

/// HandshakeReading state data.
pub struct WebSocketHandshakeReadingState {
    pub connection: HttpClientConnection,
    pub reader: HttpResponseReader<SimpleHttpBody, RawStream>,
    pub ws_key: String,
    pub subprotocols: Option<String>,
    pub delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
    pub read_timeout: Duration,
}

/// HandshakeValidating state data.
pub struct WebSocketHandshakeValidatingState {
    pub connection: HttpClientConnection,
    pub headers: crate::wire::simple_http::SimpleHeaders,
    pub ws_key: String,
    pub subprotocols: Option<String>,
    pub delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
    pub read_timeout: Duration,
}

/// [`WebSocketState`] represents the state machine states.
///
/// WHY: Using Option<Box<T>> for each state allows zero-copy state transitions
/// via Option::take() instead of cloning large data structures.
#[allow(dead_code)]
enum WebSocketState {
    Init(Option<Box<WebSocketConnectInfo>>),
    Connecting(Option<Box<WebSocketConnectingState>>),
    HandshakeSending(Option<Box<WebSocketHandshakeSendingState>>),
    HandshakeReading(Option<Box<WebSocketHandshakeReadingState>>),
    HandshakeValidating(Option<Box<WebSocketHandshakeValidatingState>>),
    Open(Option<Box<WebSocketOpenState>>),
    Closed(Option<WebSocketError>),
}

/// WebSocket task that implements the TaskIterator pattern.
///
/// WHY: Provides state-machine-based WebSocket client that integrates with
/// valtron executor system.
///
/// WHAT: Manages WebSocket connection lifecycle from initial connection through
/// handshake to message reading.
///
/// HOW: State machine with states: Init → Connecting → HandshakeSending →
/// HandshakeReading → HandshakeValidating → Open → Closed
pub struct WebSocketTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<WebSocketState>,
    pool: Arc<HttpConnectionPool<R>>,
    sleep_between_work: Duration,
}

impl<R> WebSocketTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Connect to a WebSocket endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    #[instrument(skip(resolver, url), err)]
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, WebSocketError> {
        let url_str = url.into();
        info!(url = %url_str, "Connecting to WebSocket endpoint");

        // Validate URL upfront - must be a valid URI with ws/wss scheme
        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            WebSocketError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
        })?;

        // Check scheme is ws or wss
        if !uri.scheme().is_ws() && !uri.scheme().is_wss() {
            return Err(WebSocketError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only ws:// and wss:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        let pool = Arc::new(HttpConnectionPool::new(
            crate::wire::simple_http::client::ConnectionPool::default(),
            resolver,
        ));

        Ok(Self {
            sleep_between_work: SLEEP_BETWEEN_WORK,
            state: Some(WebSocketState::Init(Some(Box::new(WebSocketConnectInfo {
                url: url_str,
                subprotocols: None,
                extra_headers: Vec::new(),
                delivery_queue: None,
                read_timeout: DEFAULT_READ_TIMEOUT,
            })))),
            pool,
        })
    }

    /// Connect to a WebSocket endpoint using an existing connection pool.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    #[instrument(skip(pool, url), err)]
    pub fn connect_with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<Self, WebSocketError> {
        let url_str = url.into();
        info!(url = %url_str, "Connecting to WebSocket endpoint with pool");

        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            WebSocketError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
        })?;

        if !uri.scheme().is_ws() && !uri.scheme().is_wss() {
            return Err(WebSocketError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only ws:// and wss:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        Ok(Self {
            sleep_between_work: SLEEP_BETWEEN_WORK,
            state: Some(WebSocketState::Init(Some(Box::new(WebSocketConnectInfo {
                url: url_str,
                subprotocols: None,
                extra_headers: Vec::new(),
                delivery_queue: None,
                read_timeout: DEFAULT_READ_TIMEOUT,
            })))),
            pool,
        })
    }

    /// Connect to a WebSocket endpoint with delivery queue for sending messages.
    ///
    /// # Arguments
    ///
    /// * `resolver` - DNS resolver
    /// * `url` - WebSocket URL
    /// * `subprotocols` - Optional comma-separated subprotocols
    /// * `extra_headers` - Additional headers for handshake
    /// * `delivery` - MessageDelivery queue for sending messages
    /// * `read_timeout` - Timeout for read operations
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    #[instrument(skip(resolver, extra_headers, delivery, url), err, fields(url_string = %url))]
    pub fn connect_with_delivery(
        resolver: R,
        url: String,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        delivery: Arc<ConcurrentQueue<WebSocketMessage>>,
        read_timeout: Duration,
        sleep_between: Duration,
    ) -> Result<Self, WebSocketError> {
        let url_str = url;
        info!(url = %url_str, "Connecting to WebSocket endpoint with delivery queue");

        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            WebSocketError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
        })?;

        if !uri.scheme().is_ws() && !uri.scheme().is_wss() {
            return Err(WebSocketError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only ws:// and wss:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        let pool = Arc::new(HttpConnectionPool::new(
            crate::wire::simple_http::client::ConnectionPool::default(),
            resolver,
        ));

        Ok(Self {
            sleep_between_work: sleep_between,
            state: Some(WebSocketState::Init(Some(Box::new(WebSocketConnectInfo {
                url: url_str,
                subprotocols,
                extra_headers,
                delivery_queue: Some(delivery),
                read_timeout,
            })))),
            pool,
        })
    }

    /// Connect to a WebSocket endpoint with delivery queue using existing pool.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    #[instrument(skip(pool, extra_headers, delivery, _url), err, fields(url = %_url))]
    pub fn connect_with_pool_and_delivery(
        _url: String,
        pool: Arc<HttpConnectionPool<R>>,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        delivery: Arc<ConcurrentQueue<WebSocketMessage>>,
        read_timeout: Duration,
        sleep_between: Duration,
    ) -> Result<Self, WebSocketError> {
        let url_str = _url;
        info!(url = %url_str, "Connecting to WebSocket endpoint with pool and delivery queue");

        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            WebSocketError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
        })?;

        if !uri.scheme().is_ws() && !uri.scheme().is_wss() {
            return Err(WebSocketError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only ws:// and wss:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        Ok(Self {
            sleep_between_work: sleep_between,
            state: Some(WebSocketState::Init(Some(Box::new(WebSocketConnectInfo {
                url: url_str,
                subprotocols,
                extra_headers,
                delivery_queue: Some(delivery),
                read_timeout,
            })))),
            pool,
        })
    }

    #[must_use]
    pub fn with_sleep_between(mut self, sleep_duration: Duration) -> Self {
        self.sleep_between_work = sleep_duration;
        self
    }

    /// Add a subprotocol to the connection.
    #[must_use]
    pub fn with_subprotocol(mut self, subprotocol: impl Into<String>) -> Self {
        let protocol_str = subprotocol.into();
        debug!(subprotocol = %protocol_str, "Adding subprotocol");
        if let Some(WebSocketState::Init(ref mut info_opt)) = self.state {
            if let Some(ref mut info) = *info_opt {
                info.subprotocols = Some(protocol_str);
            }
        }
        self
    }

    /// Add subprotocols to the connection.
    #[must_use]
    pub fn with_subprotocols(mut self, subprotocols: &[impl AsRef<str>]) -> Self {
        let protocols: Vec<String> = subprotocols
            .iter()
            .map(|s| s.as_ref().to_string())
            .collect();
        let protocols_str = protocols.join(", ");
        debug!(protocols = %protocols_str, "Adding subprotocols");
        if let Some(WebSocketState::Init(ref mut info_opt)) = self.state {
            if let Some(ref mut info) = *info_opt {
                info.subprotocols = Some(protocols_str);
            }
        }
        self
    }

    /// Add an extra header to the connection.
    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        debug!(?name, "Adding custom header");
        if let Some(WebSocketState::Init(ref mut info_opt)) = self.state {
            if let Some(ref mut info) = *info_opt {
                info.extra_headers.push((name, value.into()));
            }
        }
        self
    }
}

impl<R> TaskIterator for WebSocketTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = Result<WebSocketMessage, WebSocketError>;
    type Pending = WebSocketProgress;
    type Spawner = BoxedSendExecutionAction;

    #[allow(clippy::too_many_lines)]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            WebSocketState::Init(mut info_opt) => {
                debug!(state = "Init", "Preparing WebSocket connection");

                let info = info_opt.take()?;
                let url = Uri::parse(&info.url).ok()?;

                // Generate WebSocket key for this connection
                let ws_key = generate_websocket_key();

                debug!(ws_key = %ws_key, "Generated WebSocket key");

                // Transition to Connecting state, carrying delivery_queue and read_timeout
                self.state = Some(WebSocketState::Connecting(Some(Box::new(
                    WebSocketConnectingState {
                        url,
                        ws_key,
                        subprotocols: info.subprotocols,
                        extra_headers: info.extra_headers,
                        delivery_queue: info.delivery_queue,
                        read_timeout: info.read_timeout,
                    },
                ))));
                Some(TaskStatus::Pending(WebSocketProgress::Connecting))
            }

            WebSocketState::Connecting(mut state_opt) => {
                let state = state_opt.take()?;
                debug!(
                    state = "Connecting",
                    host = %state.url.host_str().unwrap_or_else(|| "unknown".to_string()),
                    "Establishing connection via pool"
                );

                // Use HttpConnectionPool to establish connection (handles DNS + TLS)
                let Ok(connection) = self.pool.create_http_connection(&state.url, None) else {
                    error!("Failed to establish HTTP connection");
                    self.state = Some(WebSocketState::Closed(Some(
                        WebSocketError::ConnectionClosed,
                    )));
                    return None;
                };

                debug!(state = "Connecting", "Connection established");

                // Build upgrade request
                let host = state.url.host_str().unwrap_or_default();
                let path = state.url.path();
                let query = state.url.query();
                let path_query = match query {
                    Some(q) => format!("{path}?{q}"),
                    None => path.to_string(),
                };

                let Ok(request) = build_upgrade_request(
                    &host,
                    &path_query,
                    &state.ws_key,
                    state.subprotocols.as_deref(),
                ) else {
                    error!("Failed to build upgrade request");
                    self.state = Some(WebSocketState::Closed(Some(WebSocketError::InvalidUrl(
                        "Failed to build upgrade request".to_string(),
                    ))));
                    return None;
                };

                // Render request to bytes
                let Ok(request_bytes_vec) = Http11::request(request).http_render_string() else {
                    error!("Failed to render upgrade request");
                    self.state = Some(WebSocketState::Closed(Some(WebSocketError::InvalidUrl(
                        "Failed to render upgrade request".to_string(),
                    ))));
                    return None;
                };
                let request_bytes: Vec<Vec<u8>> = request_bytes_vec
                    .into_bytes()
                    .into_iter()
                    .map(|b| vec![b])
                    .collect();

                // Transition to HandshakeSending, carrying delivery_queue and read_timeout
                self.state = Some(WebSocketState::HandshakeSending(Some(Box::new(
                    WebSocketHandshakeSendingState {
                        connection,
                        request_bytes,
                        current_chunk: 0,
                        ws_key: state.ws_key,
                        subprotocols: state.subprotocols,
                        delivery_queue: state.delivery_queue,
                        read_timeout: state.read_timeout,
                    },
                ))));
                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
            }

            WebSocketState::HandshakeSending(mut state_opt) => {
                let mut state = state_opt.take()?;
                // Send ONE chunk per next() call
                if state.current_chunk < state.request_bytes.len() {
                    let chunk = &state.request_bytes[state.current_chunk];
                    let _ = state.connection.write_all(chunk);
                    state.current_chunk += 1;

                    self.state = Some(WebSocketState::HandshakeSending(Some(state)));
                    return Some(TaskStatus::Pending(WebSocketProgress::Handshaking));
                }

                // All chunks sent, flush
                let _ = state.connection.flush();

                debug!(state = "HandshakeReading", "Request sent, reading response");

                // Create HttpResponseReader
                let stream = state.connection.clone_stream();
                let reader = HttpResponseReader::new(stream, SimpleHttpBody::default());

                self.state = Some(WebSocketState::HandshakeReading(Some(Box::new(
                    WebSocketHandshakeReadingState {
                        connection: state.connection,
                        reader,
                        ws_key: state.ws_key,
                        subprotocols: state.subprotocols,
                        delivery_queue: state.delivery_queue,
                        read_timeout: state.read_timeout,
                    },
                ))));
                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
            }

            WebSocketState::HandshakeReading(mut state_opt) => {
                let mut state = state_opt.take()?;
                // Read ONE IncomingResponseParts per next() call
                match state.reader.next() {
                    Some(Ok(part)) => {
                        match part {
                            crate::wire::simple_http::IncomingResponseParts::Intro(
                                status,
                                _proto,
                                _text,
                            ) => {
                                trace!(?status, "Received response status");
                                if status != Status::SwitchingProtocols {
                                    error!(?status, "Upgrade failed - not 101 Switching Protocols");
                                    self.state = Some(WebSocketState::Closed(Some(
                                        WebSocketError::UpgradeFailed(
                                            status.clone().into_usize() as u16
                                        ),
                                    )));
                                    return None;
                                }
                                // Continue reading headers
                                self.state = Some(WebSocketState::HandshakeReading(Some(state)));
                                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                            }
                            crate::wire::simple_http::IncomingResponseParts::Headers(headers) => {
                                debug!("Received headers, transitioning to validation");
                                self.state = Some(WebSocketState::HandshakeValidating(Some(
                                    Box::new(WebSocketHandshakeValidatingState {
                                        connection: state.connection,
                                        headers,
                                        ws_key: state.ws_key,
                                        subprotocols: state.subprotocols,
                                        delivery_queue: state.delivery_queue,
                                        read_timeout: state.read_timeout,
                                    }),
                                )));
                                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                            }
                            _ => {
                                // Skip unexpected parts
                                self.state = Some(WebSocketState::HandshakeReading(Some(state)));
                                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "Failed to read response");
                        self.state = Some(WebSocketState::Closed(Some(
                            WebSocketError::ProtocolError(format!("HTTP response read error: {e}")),
                        )));
                        None
                    }
                    None => {
                        error!("Response stream ended unexpectedly");
                        self.state = Some(WebSocketState::Closed(Some(
                            WebSocketError::ConnectionClosed,
                        )));
                        None
                    }
                }
            }

            WebSocketState::HandshakeValidating(mut state_opt) => {
                let state = state_opt.take()?;
                debug!(state = "HandshakeValidating", "Validating upgrade response");

                // Compute expected accept key
                let expected_accept = compute_accept_key(&state.ws_key);

                // Validate Sec-WebSocket-Accept header
                let accept_values = state
                    .headers
                    .get(&SimpleHeader::SEC_WEBSOCKET_ACCEPT)
                    .ok_or(WebSocketError::MissingAcceptKey);

                match accept_values {
                    Ok(accept_list) => {
                        let accept_value =
                            accept_list.first().ok_or(WebSocketError::MissingAcceptKey);
                        match accept_value {
                            Ok(accept) => {
                                if accept != &expected_accept {
                                    error!("Invalid accept key");
                                    self.state = Some(WebSocketState::Closed(Some(
                                        WebSocketError::InvalidAcceptKey,
                                    )));
                                    return None;
                                }

                                debug!("Accept key validated, WebSocket connection established");

                                // Success - transition to Open state with delivery queue and read_timeout
                                let stream = state.connection.clone_stream();

                                // delivery_queue must always be provided - panic if missing as this
                                // is a programming error (queue should be created before connect)
                                let queue = state
                                    .delivery_queue
                                    .expect("delivery_queue must be provided");

                                // Create buffer pool for zero-copy frame reading (8KB buffers, 4 pre-allocated)
                                let buffer_pool =
                                    Arc::new(crate::io::buffer_pool::BytesPool::new(8192, 4));

                                self.state = Some(WebSocketState::Open(Some(Box::new(
                                    WebSocketOpenState {
                                        stream,
                                        delivery_queue: queue,
                                        read_timeout: state.read_timeout,
                                        assembler: super::assembler::MessageAssembler::default(),
                                        buffer_pool,
                                        frame_buffer: bytes::BytesMut::new(),
                                    },
                                ))));

                                // Return connection established message
                                Some(TaskStatus::Ready(Ok(
                                    WebSocketMessage::ConnectionEstablished,
                                )))
                            }
                            Err(e) => {
                                error!(error = ?e, "Missing accept key");
                                self.state = Some(WebSocketState::Closed(Some(e)));
                                None
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = ?e, "Missing accept key header");
                        self.state = Some(WebSocketState::Closed(Some(e)));
                        None
                    }
                }
            }

            WebSocketState::Open(mut open_state_opt) => {
                debug!("OpenState: Processing messages");

                let mut open_state = open_state_opt.take()?;
                trace!(state = "Open", "Reading WebSocket frame");
                debug!("Delivery queue len: {}", open_state.delivery_queue.len());

                // Check for outgoing messages in delivery queue first
                match open_state.delivery_queue.pop() {
                    Ok(outgoing) => {
                        debug!("Popped message from delivery queue: {:?}", outgoing);
                        // Send outgoing message - client MUST mask frames per RFC 6455
                        let frame = match outgoing {
                            WebSocketMessage::ConnectionEstablished => {
                                debug!("ConnectionEstablished and sending None");
                                // No frame to send
                                None
                            }
                            WebSocketMessage::Text(text) => Some(WebSocketFrame {
                                fin: true,
                                opcode: Opcode::Text,
                                mask: Some(generate_mask()),
                                payload: text.into_bytes(),
                            }),
                            WebSocketMessage::Binary(data) => Some(WebSocketFrame {
                                fin: true,
                                opcode: Opcode::Binary,
                                mask: Some(generate_mask()),
                                payload: data,
                            }),
                            WebSocketMessage::Ping(data) => Some(WebSocketFrame {
                                fin: true,
                                opcode: Opcode::Ping,
                                mask: Some(generate_mask()),
                                payload: data,
                            }),
                            WebSocketMessage::Pong(data) => Some(WebSocketFrame {
                                fin: true,
                                opcode: Opcode::Pong,
                                mask: Some(generate_mask()),
                                payload: data,
                            }),
                            WebSocketMessage::Close(code, reason) => {
                                tracing::debug!(
                                    "Received closed signal from incoming messages: {}",
                                    &code
                                );

                                let mut payload = code.to_be_bytes().to_vec();
                                payload.extend_from_slice(reason.as_bytes());
                                Some(WebSocketFrame {
                                    fin: true,
                                    opcode: Opcode::Close,
                                    mask: Some(generate_mask()),
                                    payload,
                                })
                            }
                        };

                        debug!("Frame generated for outgoing message: {:?}", &frame);
                        if let Some(frame) = frame {
                            let encoded = frame.encode();

                            debug!("Sending {} bytes to stream", encoded.len());
                            if let Err(err) = open_state.stream.write_all(&encoded) {
                                tracing::error!("Failed to write data to stream: {:?}", err);
                            }

                            if let Err(err) = open_state.stream.flush() {
                                tracing::error!("Failed to flush stream due to: {:?}", err);
                            }
                            debug!("Sent frame to server: opcode={:?}", frame.opcode);
                        } else {
                            debug!("No message sent to server");
                        }

                        // Stay in Open state - no cloning needed, just put back
                        self.state = Some(WebSocketState::Open(Some(open_state)));
                        return Some(TaskStatus::Pending(WebSocketProgress::Reading));
                    }
                    Err(err) => {
                        debug!(
                            "Outgoing Delivery queue empty or disconnected due to: {:?}",
                            err
                        );
                    }
                }

                if let Err(err) = open_state.stream.flush() {
                    tracing::error!("Failed to flush stream due to: {:?}", err);
                }

                // Read ONE frame per next() call
                debug!("Attempting to decode WebSocket frame from stream");

                // Set read timeout before reading
                let _ = open_state
                    .stream
                    .set_read_timeout_as(open_state.read_timeout);
                debug!(
                    "Read timeout set to {:?} as {:?}",
                    open_state.read_timeout,
                    open_state.stream.get_current_read_timeout()
                );

                // Use pooled buffer for zero-copy frame reading
                match WebSocketFrame::decode_with_buffer(
                    &mut open_state.stream,
                    &mut open_state.frame_buffer,
                ) {
                    Ok(frame) => {
                        // Validate frame
                        debug!(
                            "Successfully decoded WebSocket frame: opcode={:?}, len={}",
                            frame.opcode,
                            frame.payload.len()
                        );
                        if let Err(e) = frame.validate() {
                            error!(error = ?e, "Invalid frame");
                            self.state = Some(WebSocketState::Open(Some(open_state)));
                            return Some(TaskStatus::Ready(Err(e)));
                        }

                        // Validate masking based on client role (client receives unmasked frames)
                        if frame.mask.is_some() {
                            error!("Received masked frame from server (protocol violation)");
                            self.state =
                                Some(WebSocketState::Closed(Some(WebSocketError::ProtocolError(
                                    "Received masked frame from server".to_string(),
                                ))));
                            return None;
                        }

                        // Validate RSV bits (no extensions negotiated)
                        // Note: frame.rs doesn't expose rsv bits in public API yet
                        // For now, we assume they are 0

                        // Handle control frames immediately
                        if frame.opcode.is_control() {
                            match frame.opcode {
                                Opcode::Ping => {
                                    debug!("Received Ping, will auto-respond with Pong");
                                    // Auto-respond with Pong (same payload)
                                    let pong_frame = WebSocketFrame {
                                        fin: true,
                                        opcode: Opcode::Pong,
                                        mask: None, // Server doesn't mask
                                        payload: frame.payload.clone(),
                                    };
                                    let pong_bytes = pong_frame.encode();
                                    let _ = open_state.stream.write_all(&pong_bytes);
                                    let _ = open_state.stream.flush();
                                    self.state = Some(WebSocketState::Open(Some(open_state)));
                                    return Some(TaskStatus::Ready(Ok(WebSocketMessage::Ping(
                                        frame.payload,
                                    ))));
                                }
                                Opcode::Pong => {
                                    debug!("Received Pong");
                                    self.state = Some(WebSocketState::Open(Some(open_state)));
                                    return Some(TaskStatus::Ready(Ok(WebSocketMessage::Pong(
                                        frame.payload,
                                    ))));
                                }
                                Opcode::Close => {
                                    debug!("Received Close");
                                    // Parse close payload
                                    let (code, reason) = parse_close_payload(&frame.payload);
                                    self.state = Some(WebSocketState::Closed(None));
                                    return Some(TaskStatus::Ready(Ok(WebSocketMessage::Close(
                                        code, reason,
                                    ))));
                                }
                                _ => {
                                    warn!(opcode = ?frame.opcode, "Unknown control frame");
                                    self.state = Some(WebSocketState::Open(Some(open_state)));
                                    return Some(TaskStatus::Ready(Err(
                                        WebSocketError::ProtocolError(
                                            "Unknown control frame".to_string(),
                                        ),
                                    )));
                                }
                            }
                        }

                        // Data frame - use MessageAssembler for fragmented messages
                        match open_state.assembler.process_frame(frame) {
                            Ok(Some(message)) => {
                                // Complete message assembled (or control frame passed through)
                                self.state = Some(WebSocketState::Open(Some(open_state)));

                                debug!("Received new message from server: {:?}", &message);
                                Some(TaskStatus::Ready(Ok(message)))
                            }
                            Ok(None) => {
                                // More fragments needed, stay in Open state
                                self.state = Some(WebSocketState::Open(Some(open_state)));
                                debug!("Received no message from server");
                                Some(TaskStatus::Pending(WebSocketProgress::Reading))
                            }
                            Err(e) => {
                                // Protocol error during assembly
                                error!("Error occurred assembly message: {:?}", &e);
                                self.state = Some(WebSocketState::Open(Some(open_state)));
                                Some(TaskStatus::Ready(Err(e)))
                            }
                        }
                    }
                    Err(WebSocketError::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        error!("Connection closed (EOF) - error details: {:?}", e);
                        self.state = Some(WebSocketState::Closed(None));
                        None
                    }
                    Err(WebSocketError::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Read timeout - not an error, just no data available yet
                        // Stay in Open state and delay before retrying to avoid busy-spinning
                        debug!("Read timeout - no data available yet, will retry after delay");
                        self.state = Some(WebSocketState::Open(Some(open_state)));
                        Some(TaskStatus::Delayed(self.sleep_between_work))
                    }
                    Err(e) => {
                        error!(error = ?e, "Frame decode error");
                        self.state = Some(WebSocketState::Closed(Some(
                            WebSocketError::ProtocolError(format!("Frame decode error: {e}")),
                        )));
                        Some(TaskStatus::Ready(Err(e)))
                    }
                }
            }

            WebSocketState::Closed(_) => {
                trace!(state = "Closed", "Task complete");
                None
            }
        }
    }
}

/// Parse close frame payload into status code and reason.
fn parse_close_payload(payload: &[u8]) -> (u16, String) {
    if payload.is_empty() {
        return (1005, String::new()); // No status code present (1005 = No Status Received)
    }
    if payload.len() == 1 {
        return (1002, String::from("Invalid close payload")); // Protocol error
    }

    let code = u16::from_be_bytes([payload[0], payload[1]]);
    let reason = if payload.len() > 2 {
        String::from_utf8_lossy(&payload[2..]).to_string()
    } else {
        String::new()
    };

    (code, reason)
}
