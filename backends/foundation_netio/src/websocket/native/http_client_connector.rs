//! [`WebSocketConnector`] impl for [`NativeHttpClient`] (F51 Stage 6).
//!
//! WHY: F51 unifies the client — the same concrete type that serves HTTP also
//! opens `WebSocket` connections through the [`WebSocketConnector`] trait. This impl reuses
//! the client's connection pool (DNS, TCP, TLS) for the HTTP/1.1 Upgrade
//! handshake.
//!
//! WHAT: Implements `WebSocketConnector::open_websocket` synchronously:
//! 1. Create a fresh connection via the pool
//! 2. Build and send the Upgrade request over that connection
//! 3. Read the 101 Switching Protocols response
//! 4. Extract the stream, wrapping it in `WebSocketConnection`
//!
//! HOW: Uses `HttpConnectionPool::create_http_connection` for transport
//! establishment, `build_upgrade_request` for the request, `Http11::request`
//! for rendering, `std::io::Write` (via Deref to `SharedByteBufferStream`)
//! for sending, and `HttpResponseReader` for parsing the response.

use std::io::Write;
use std::time::Duration;

use foundation_core::url::Uri;

use crate::http::NativeHttpClient;
use crate::shared::client::DnsResolver;
use crate::shared::http::{
    Http11, HttpResponseReader, RenderHttp, SimpleHeaders, SimpleHttpBody,
};
use crate::websocket::native::connection::WebSocketConnection;
use crate::websocket::shared::connector::WebSocketConnector;
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::handshake::{
    build_upgrade_request, compute_accept_key, generate_websocket_key, validate_upgrade_response,
};

/// Default connect timeout for WebSocket handshake.
const DEFAULT_WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

impl<R: DnsResolver + Clone + Send + Sync + 'static> WebSocketConnector for NativeHttpClient<R> {
    fn open_websocket(
        &self,
        url: &str,
        _headers: SimpleHeaders,
    ) -> Result<WebSocketConnection, WebSocketError> {
        let uri =
            Uri::parse(url).map_err(|e| WebSocketError::InvalidUrl(format!("invalid URL: {e}")))?;

        let pool = self
            .client_pool()
            .ok_or_else(|| WebSocketError::ProtocolError("no connection pool configured".into()))?;

        // 1. Establish transport (DNS → TCP → TLS) through the pool.
        let mut conn = pool
            .create_http_connection(&uri, Some(DEFAULT_WS_CONNECT_TIMEOUT))
            .map_err(|e| WebSocketError::ProtocolError(format!("connection failed: {e}")))?;

        // 2. Build the Upgrade request.
        let host_only = uri.host_str().unwrap_or_else(|| "localhost".to_string());
        let host = match uri.port() {
            Some(p) => format!("{host_only}:{p}"),
            None => host_only.clone(),
        };
        let path = uri.path();
        let query = uri.query();
        let path_query = match query {
            Some(q) => format!("{path}?{q}"),
            None => path.to_string(),
        };
        let ws_key = generate_websocket_key();
        let expected_accept = compute_accept_key(&ws_key);

        let request = build_upgrade_request(&host, &path_query, &ws_key, None)?;

        // Render the request to wire bytes.
        let request_bytes = Http11::request(request)
            .http_render_string()
            .map_err(|e| {
                WebSocketError::ProtocolError(format!("failed to render upgrade request: {e:?}"))
            })?
            .into_bytes();

        // 3. Send the Upgrade request on the connection.
        conn.write_all(&request_bytes)
            .map_err(|e| WebSocketError::ProtocolError(format!("write failed: {e}")))?;
        conn.flush()
            .map_err(|e| WebSocketError::ProtocolError(format!("flush failed: {e}")))?;

        // 4. Read the response.
        let stream = conn.clone_stream();
        let mut reader = HttpResponseReader::new(stream, SimpleHttpBody::default());

        // Drain response parts — expect Intro (101) then Headers.
        let mut response_status = None;
        let mut response_headers = SimpleHeaders::new();
        for part in reader.by_ref() {
            match part {
                Ok(crate::shared::http::IncomingResponseParts::Intro(
                    status,
                    _proto,
                    _reason,
                )) => {
                    response_status = Some(status);
                }
                Ok(crate::shared::http::IncomingResponseParts::Headers(headers)) => {
                    response_headers = headers;
                }
                Ok(_) => {}
                Err(e) => {
                    return Err(WebSocketError::ProtocolError(format!(
                        "response read failed: {e}"
                    )));
                }
            }
        }

        let status = response_status.ok_or_else(|| {
            WebSocketError::ProtocolError("no status line in upgrade response".into())
        })?;

        // 5. Validate the 101 Switching Protocols response.
        validate_upgrade_response(&status, &response_headers, &expected_accept)?;

        // 6. Extract the stream — the connection is now a WebSocket.
        let raw_stream = conn.take_stream();
        Ok(WebSocketConnection::new(raw_stream))
    }
}
