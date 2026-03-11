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

/// [`WebSocketState`] represents the state machine states.
#[allow(dead_code)] // Some fields used in Phase 2
enum WebSocketState {
    Init(Option<Box<WebSocketConnectInfo>>),
    Connecting {
        url: Uri,
        ws_key: String,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
        read_timeout: Duration,
    },
    HandshakeSending {
        connection: HttpClientConnection,
        request_bytes: Vec<Vec<u8>>,
        current_chunk: usize,
        ws_key: String,
        subprotocols: Option<String>,
        delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
        read_timeout: Duration,
    },
    HandshakeReading {
        connection: HttpClientConnection,
        reader: HttpResponseReader<SimpleHttpBody, RawStream>,
        ws_key: String,
        subprotocols: Option<String>,
        delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
        read_timeout: Duration,
    },
    HandshakeValidating {
        connection: HttpClientConnection,
        headers: crate::wire::simple_http::SimpleHeaders,
        ws_key: String,
        subprotocols: Option<String>,
        delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
        read_timeout: Duration,
    },
    Open {
        stream: crate::io::ioutils::SharedByteBufferStream<RawStream>,
        delivery_queue: Arc<ConcurrentQueue<WebSocketMessage>>,
        read_timeout: Duration,
    },
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
    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
                self.state = Some(WebSocketState::Connecting {
                    url,
                    ws_key,
                    subprotocols: info.subprotocols,
                    extra_headers: info.extra_headers,
                    delivery_queue: info.delivery_queue,
                    read_timeout: info.read_timeout,
                });
                Some(TaskStatus::Pending(WebSocketProgress::Connecting))
            }

            WebSocketState::Connecting {
                url,
                ws_key,
                subprotocols,
                extra_headers: _,
                delivery_queue,
                read_timeout,
            } => {
                debug!(
                    state = "Connecting",
                    host = %url.host_str().unwrap_or_else(|| "unknown".to_string()),
                    "Establishing connection via pool"
                );

                // Use HttpConnectionPool to establish connection (handles DNS + TLS)
                let Ok(connection) = self.pool.create_http_connection(&url, None) else {
                    error!("Failed to establish HTTP connection");
                    self.state = Some(WebSocketState::Closed(Some(
                        WebSocketError::ConnectionClosed,
                    )));
                    return None;
                };

                debug!(state = "Connecting", "Connection established");

                // Build upgrade request
                let host = url.host_str().unwrap_or_default();
                let path = url.path();
                let query = url.query();
                let path_query = match query {
                    Some(q) => format!("{path}?{q}"),
                    None => path.to_string(),
                };

                let Ok(request) =
                    build_upgrade_request(&host, &path_query, &ws_key, subprotocols.as_deref())
                else {
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
                self.state = Some(WebSocketState::HandshakeSending {
                    connection,
                    request_bytes,
                    current_chunk: 0,
                    ws_key,
                    subprotocols,
                    delivery_queue,
                    read_timeout,
                });
                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
            }

            WebSocketState::HandshakeSending {
                mut connection,
                request_bytes,
                mut current_chunk,
                ws_key,
                subprotocols,
                delivery_queue,
                read_timeout,
            } => {
                // Send ONE chunk per next() call
                if current_chunk < request_bytes.len() {
                    let chunk = &request_bytes[current_chunk];
                    let _ = connection.write_all(chunk);
                    current_chunk += 1;

                    self.state = Some(WebSocketState::HandshakeSending {
                        connection,
                        request_bytes,
                        current_chunk,
                        ws_key,
                        subprotocols,
                        delivery_queue,
                        read_timeout,
                    });
                    Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                } else {
                    // All chunks sent, flush
                    let _ = connection.flush();

                    debug!(state = "HandshakeReading", "Request sent, reading response");

                    // Create HttpResponseReader
                    let stream = connection.clone_stream();
                    let reader = HttpResponseReader::new(stream, SimpleHttpBody);

                    self.state = Some(WebSocketState::HandshakeReading {
                        connection,
                        reader,
                        ws_key,
                        subprotocols,
                        delivery_queue,
                        read_timeout,
                    });
                    Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                }
            }

            WebSocketState::HandshakeReading {
                connection,
                mut reader,
                ws_key,
                subprotocols,
                delivery_queue,
                read_timeout,
            } => {
                // Read ONE IncomingResponseParts per next() call
                match reader.next() {
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
                                self.state = Some(WebSocketState::HandshakeReading {
                                    connection,
                                    reader,
                                    ws_key,
                                    subprotocols,
                                    delivery_queue,
                                    read_timeout,
                                });
                                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                            }
                            crate::wire::simple_http::IncomingResponseParts::Headers(headers) => {
                                debug!("Received headers, transitioning to validation");
                                self.state = Some(WebSocketState::HandshakeValidating {
                                    connection,
                                    headers,
                                    ws_key,
                                    subprotocols,
                                    delivery_queue,
                                    read_timeout,
                                });
                                Some(TaskStatus::Pending(WebSocketProgress::Handshaking))
                            }
                            _ => {
                                // Skip unexpected parts
                                self.state = Some(WebSocketState::HandshakeReading {
                                    connection,
                                    reader,
                                    ws_key,
                                    subprotocols,
                                    delivery_queue,
                                    read_timeout,
                                });
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

            WebSocketState::HandshakeValidating {
                connection,
                headers,
                ws_key,
                subprotocols: _subprotocols,
                delivery_queue,
                read_timeout,
            } => {
                debug!(state = "HandshakeValidating", "Validating upgrade response");

                // Compute expected accept key
                let expected_accept = compute_accept_key(&ws_key);

                // Validate Sec-WebSocket-Accept header
                let accept_values = headers
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
                                let stream = connection.clone_stream();

                                // delivery_queue must always be provided - panic if missing as this
                                // is a programming error (queue should be created before connect)
                                let queue =
                                    delivery_queue.expect("delivery_queue must be provided");

                                self.state = Some(WebSocketState::Open {
                                    stream,
                                    delivery_queue: queue,
                                    read_timeout,
                                });

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

            WebSocketState::Open {
                mut stream,
                ref delivery_queue,
                ref read_timeout,
            } => {
                trace!(state = "Open", "Reading WebSocket frame");
                debug!("Delivery queue len: {}", delivery_queue.len());

                // Check for outgoing messages in delivery queue first
                match delivery_queue.pop() {
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

                        if let Some(frame) = frame {
                            let encoded = frame.encode();

                            debug!("Sending {} bytes to stream", encoded.len());
                            let _ = stream.write_all(&encoded);

                            if let Err(err) = stream.flush() {
                                tracing::error!("Failed to flush stream due to: {:?}", err);
                            }
                            debug!("Sent frame to server: opcode={:?}", frame.opcode);
                        }

                        // Stay in Open state
                        self.state = Some(WebSocketState::Open {
                            stream,
                            delivery_queue: delivery_queue.clone(),
                            read_timeout: *read_timeout,
                        });
                        return Some(TaskStatus::Pending(WebSocketProgress::Reading));
                    }
                    Err(err) => {
                        debug!("Delivery queue empty or disconnected due to: {:?}", err);
                    }
                }

                if let Err(err) = stream.flush() {
                    tracing::error!("Failed to flush stream due to: {:?}", err);
                }

                // Read ONE frame per next() call
                debug!("Attempting to decode WebSocket frame from stream");

                // Set read timeout before reading
                let _ = stream.set_read_timeout_as(*read_timeout);
                debug!(
                    "Read timeout set to {:?} as {:?}",
                    read_timeout,
                    stream.get_current_read_timeout()
                );

                match WebSocketFrame::decode(&mut stream) {
                    Ok(frame) => {
                        // Validate frame
                        debug!(
                            "Successfully decoded WebSocket frame: opcode={:?}, len={}",
                            frame.opcode,
                            frame.payload.len()
                        );
                        if let Err(e) = frame.validate() {
                            error!(error = ?e, "Invalid frame");
                            self.state = Some(WebSocketState::Open {
                                stream,
                                delivery_queue: delivery_queue.clone(),
                                read_timeout: *read_timeout,
                            });
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
                                    let _ = stream.write_all(&pong_bytes);
                                    let _ = stream.flush();
                                    self.state = Some(WebSocketState::Open {
                                        stream,
                                        delivery_queue: delivery_queue.clone(),
                                        read_timeout: *read_timeout,
                                    });
                                    return Some(TaskStatus::Ready(Ok(WebSocketMessage::Ping(
                                        frame.payload,
                                    ))));
                                }
                                Opcode::Pong => {
                                    debug!("Received Pong");
                                    self.state = Some(WebSocketState::Open {
                                        stream,
                                        delivery_queue: delivery_queue.clone(),
                                        read_timeout: *read_timeout,
                                    });
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
                                    self.state = Some(WebSocketState::Open {
                                        stream,
                                        delivery_queue: delivery_queue.clone(),
                                        read_timeout: *read_timeout,
                                    });
                                    return Some(TaskStatus::Ready(Err(
                                        WebSocketError::ProtocolError(
                                            "Unknown control frame".to_string(),
                                        ),
                                    )));
                                }
                            }
                        }

                        // Data frame - assemble message
                        // For simplicity, we assume non-fragmented messages for now
                        // TODO: Handle fragmentation with MessageAssembler
                        if !frame.fin {
                            warn!("Received fragmented message (not yet supported)");
                            self.state = Some(WebSocketState::Open {
                                stream,
                                delivery_queue: delivery_queue.clone(),
                                read_timeout: *read_timeout,
                            });
                            return Some(TaskStatus::Ready(Err(WebSocketError::ProtocolError(
                                "Fragmented messages not yet supported".to_string(),
                            ))));
                        }

                        match frame.opcode {
                            Opcode::Text => {
                                debug!("Received Text frame: {} bytes", frame.payload.len());
                                match String::from_utf8(frame.payload.clone()) {
                                    Ok(text) => {
                                        self.state = Some(WebSocketState::Open {
                                            stream,
                                            delivery_queue: delivery_queue.clone(),
                                            read_timeout: *read_timeout,
                                        });
                                        Some(TaskStatus::Ready(Ok(WebSocketMessage::Text(text))))
                                    }
                                    Err(e) => {
                                        self.state = Some(WebSocketState::Open {
                                            stream,
                                            delivery_queue: delivery_queue.clone(),
                                            read_timeout: *read_timeout,
                                        });
                                        Some(TaskStatus::Ready(Err(WebSocketError::InvalidUtf8(e))))
                                    }
                                }
                            }
                            Opcode::Binary => {
                                self.state = Some(WebSocketState::Open {
                                    stream,
                                    delivery_queue: delivery_queue.clone(),
                                    read_timeout: *read_timeout,
                                });
                                Some(TaskStatus::Ready(Ok(WebSocketMessage::Binary(
                                    frame.payload,
                                ))))
                            }
                            Opcode::Continuation => {
                                warn!("Unexpected continuation frame");
                                self.state = Some(WebSocketState::Open {
                                    stream,
                                    delivery_queue: delivery_queue.clone(),
                                    read_timeout: *read_timeout,
                                });
                                Some(TaskStatus::Ready(Err(WebSocketError::ProtocolError(
                                    "Unexpected continuation frame".to_string(),
                                ))))
                            }
                            _ => {
                                warn!(opcode = ?frame.opcode, "Unknown data frame");
                                self.state = Some(WebSocketState::Open {
                                    stream,
                                    delivery_queue: delivery_queue.clone(),
                                    read_timeout: *read_timeout,
                                });
                                Some(TaskStatus::Ready(Err(WebSocketError::ProtocolError(
                                    "Unknown data frame opcode".to_string(),
                                ))))
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
                        self.state = Some(WebSocketState::Open {
                            stream,
                            delivery_queue: delivery_queue.clone(),
                            read_timeout: *read_timeout,
                        });
                        Some(TaskStatus::Delayed(Duration::from_millis(5)))
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
