//! HTTP/3 connection and request mapping over the QUIC trait set (F34).
//!
//! WHY: this is where HTTP/3 stops being a codec and becomes a transport. It owns
//! the control stream, the SETTINGS exchange, and the rule that a request lives on
//! exactly one bidirectional stream.
//!
//! WHAT: [`H3Connection`] — open the control stream, send SETTINGS, accept
//! requests; [`H3Request`] — one request stream, readable as headers then body,
//! writable as a response.
//!
//! HOW: everything is progress-returning, exactly like the QUIC traits beneath it.
//! There is no `Poll`, no `Waker`, and no executor coupling: the *driving task*
//! turns a `Pending` into `TaskStatus::Pending`/`Depends`.
//!
//! ## SETTINGS must be the first frame on the control stream
//!
//! RFC 9114 §6.2.1: each side opens a unidirectional control stream, prefixes it
//! with stream type `0x00`, and sends SETTINGS as its **first** frame. Anything
//! else first is `H3_MISSING_SETTINGS`. The control stream is never closed while
//! the connection lives — closing it is `H3_CLOSED_CRITICAL_STREAM`.
//!
//! ## Why we advertise a zero-capacity QPACK table
//!
//! `QPACK_MAX_TABLE_CAPACITY = 0` and `QPACK_BLOCKED_STREAMS = 0` tell the peer it
//! may never send us a dynamic reference. RFC 9204 §3.2.3 allows this explicitly,
//! and it is what lets [`super::qpack`] omit the encoder and decoder streams
//! entirely. See that module for the trade.

use bytes::{Buf, Bytes};

use foundation_core::valtron::Stream;

use crate::quic::{
    QuicBidiStream, QuicConnError, QuicConnection, QuicRecvStream, QuicSendStream, QuicStreamError,
};

use super::frame::{setting, Frame};
use super::stream::{FramedRecv, StreamError};

/// Unidirectional stream types (RFC 9114 §11.2.4, RFC 9204 §4.2).
pub mod stream_type {
    /// The connection's control stream.
    pub const CONTROL: u64 = 0x00;
    /// A server push stream.
    pub const PUSH: u64 = 0x01;
    /// The QPACK encoder stream. Unused: our dynamic table is disabled.
    pub const QPACK_ENCODER: u64 = 0x02;
    /// The QPACK decoder stream. Unused: our dynamic table is disabled.
    pub const QPACK_DECODER: u64 = 0x03;
    /// WebTransport bidirectional stream (draft-ietf-webtrans-http3 §4.1, F06).
    pub const WEBTRANSPORT: u64 = 0x54;
}

/// HTTP/3 error codes (RFC 9114 §8.1).
pub mod error_code {
    /// No error.
    pub const H3_NO_ERROR: u64 = 0x0100;
    /// A frame appeared where it is not allowed.
    pub const H3_FRAME_UNEXPECTED: u64 = 0x0105;
    /// A stream required by the connection was closed.
    pub const H3_CLOSED_CRITICAL_STREAM: u64 = 0x0104;
    /// The first control frame was not SETTINGS.
    pub const H3_MISSING_SETTINGS: u64 = 0x010a;
    /// A malformed request or response.
    pub const H3_MESSAGE_ERROR: u64 = 0x010e;
    /// QPACK could not decode a field section.
    pub const QPACK_DECOMPRESSION_FAILED: u64 = 0x0200;
}

/// The SETTINGS we send: no QPACK dynamic table, no blocked streams.
fn our_settings(max_field_section_size: u64) -> Frame {
    Frame::Settings(vec![
        (setting::QPACK_MAX_TABLE_CAPACITY, 0),
        (setting::QPACK_BLOCKED_STREAMS, 0),
        (setting::MAX_FIELD_SECTION_SIZE, max_field_section_size),
        (setting::ENABLE_CONNECT_PROTOCOL, 1),   // RFC 9220 Extended CONNECT
        (setting::ENABLE_WEBTRANSPORT, 1),        // draft-ietf-webtrans-http3 (F06)
    ])
}

/// Why an HTTP/3 connection failed.
#[derive(Debug)]
pub enum H3Error {
    /// The QUIC connection ended.
    Connection(QuicConnError),
    /// A QUIC stream failed.
    Stream(QuicStreamError),
    /// HTTP/3 framing or sequencing was violated.
    Protocol {
        /// The RFC 9114 §8.1 error code to close with.
        code: u64,
        /// What went wrong.
        message: String,
    },
}

impl std::fmt::Display for H3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(e) => write!(f, "HTTP/3 connection error: {e}"),
            Self::Stream(e) => write!(f, "HTTP/3 stream error: {e}"),
            Self::Protocol { code, message } => {
                write!(f, "HTTP/3 protocol error {code:#06x}: {message}")
            }
        }
    }
}

impl std::error::Error for H3Error {}

impl From<QuicConnError> for H3Error {
    fn from(e: QuicConnError) -> Self {
        Self::Connection(e)
    }
}

impl From<QuicStreamError> for H3Error {
    fn from(e: QuicStreamError) -> Self {
        Self::Stream(e)
    }
}

impl From<StreamError> for H3Error {
    fn from(e: StreamError) -> Self {
        match e {
            StreamError::Quic(e) => Self::Stream(e),
            StreamError::Protocol(message) => Self::Protocol {
                code: error_code::H3_FRAME_UNEXPECTED,
                message,
            },
            StreamError::Truncated => Self::Protocol {
                code: error_code::H3_FRAME_UNEXPECTED,
                message: "stream ended mid-frame".into(),
            },
        }
    }
}

/// The default cap on a decoded field section, advertised as
/// `SETTINGS_MAX_FIELD_SECTION_SIZE`.
pub const DEFAULT_MAX_FIELD_SECTION_SIZE: u64 = 64 * 1024;

/// An HTTP/3 connection over a QUIC connection.
///
/// Drive it by calling [`H3Connection::poll_setup`] until it reports ready, then
/// [`H3Connection::poll_accept`] for each inbound request.
#[derive(Debug)]
pub struct H3Connection<C: QuicConnection> {
    conn: C,
    /// Our control stream. Opened once, never closed.
    control: Option<C::SendStream>,
    /// Bytes of the control-stream preamble still to be written.
    control_preamble: Bytes,
    /// The peer's control stream, once we have identified it. Wrapped in
    /// [`TypedRecv`] because a unidirectional stream's first bytes are its type
    /// varint, not a frame.
    peer_control: Option<FramedRecv<TypedRecv<C::RecvStream>>>,
    /// The peer's SETTINGS, once received.
    peer_settings: Option<Vec<(u64, u64)>>,
    max_field_section_size: u64,
}

impl<C: QuicConnection> H3Connection<C> {
    /// Wrap a QUIC connection as HTTP/3.
    ///
    /// Nothing is sent yet: call [`H3Connection::poll_setup`].
    pub fn new(conn: C) -> Self {
        Self::with_max_field_section_size(conn, DEFAULT_MAX_FIELD_SECTION_SIZE)
    }

    /// As [`H3Connection::new`], advertising a specific field-section cap.
    pub fn with_max_field_section_size(conn: C, max_field_section_size: u64) -> Self {
        // The control stream's preamble: its type varint, then SETTINGS. RFC 9114
        // §6.2.1 requires SETTINGS to be the *first* frame.
        let mut preamble = Vec::new();
        super::varint::VarInt::new(stream_type::CONTROL)
            .unwrap_or(super::varint::VarInt::MAX)
            .encode(&mut preamble);
        our_settings(max_field_section_size).encode(&mut preamble);

        Self {
            conn,
            control: None,
            control_preamble: Bytes::from(preamble),
            peer_control: None,
            peer_settings: None,
            max_field_section_size,
        }
    }

    /// The peer's SETTINGS, once its control stream has delivered them.
    pub fn peer_settings(&self) -> Option<&[(u64, u64)]> {
        self.peer_settings.as_deref()
    }

    /// The underlying QUIC connection.
    pub fn get_mut(&mut self) -> &mut C {
        &mut self.conn
    }

    /// WHY: HTTP/3 is not usable until our control stream carries SETTINGS.
    ///
    /// WHAT: open the control stream and flush its preamble.
    ///
    /// HOW: `Next(Ok(()))` once the whole preamble is written. `Pending` while the
    /// stream cannot be opened (peer's `MAX_STREAMS`) or the window is full.
    /// Idempotent: calling it after setup returns `Next(Ok(()))` immediately.
    pub fn poll_setup(&mut self) -> Stream<Result<(), H3Error>, ()> {
        if self.control.is_none() {
            match self.conn.open_send() {
                Stream::Next(Ok(s)) => self.control = Some(s),
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                _ => return Stream::Pending(()),
            }
        }

        let control = self.control.as_mut().expect("opened above");

        while self.control_preamble.has_remaining() {
            match control.send(&mut self.control_preamble) {
                Stream::Next(Ok(_)) => {}
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                // Flow-control window full; resume on the next call.
                _ => return Stream::Pending(()),
            }
        }

        Stream::Next(Ok(()))
    }

    /// WHY: the peer's control stream carries its SETTINGS, and RFC 9114 §6.2.1
    /// makes SETTINGS-first mandatory.
    ///
    /// WHAT: accept the peer's unidirectional streams and read its SETTINGS.
    ///
    /// HOW: call repeatedly. `Next(Ok(true))` once SETTINGS have arrived.
    /// A first control frame that is not SETTINGS is `H3_MISSING_SETTINGS`.
    /// A second control stream is `H3_FRAME_UNEXPECTED` — there is only one.
    pub fn poll_peer_settings(&mut self) -> Stream<Result<bool, H3Error>, ()> {
        if self.peer_settings.is_some() {
            return Stream::Next(Ok(true));
        }

        // Identify the peer's control stream, if we have not already.
        if self.peer_control.is_none() {
            match self.conn.accept_recv() {
                Stream::Next(Ok(recv)) => {
                    // Every unidirectional stream is prefixed by its type varint.
                    // We only care about the control stream; QPACK's encoder and
                    // decoder streams are never used, because our dynamic table
                    // is disabled, and push is not implemented.
                    self.peer_control = Some(FramedRecv::new(TypedRecv::new(recv)));
                }
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                _ => return Stream::Pending(()),
            }
        }

        let control = self.peer_control.as_mut().expect("set above");
        match control.poll_frame() {
            Stream::Next(Ok(Some(Frame::Settings(settings)))) => {
                self.peer_settings = Some(settings);
                Stream::Next(Ok(true))
            }
            Stream::Next(Ok(Some(other))) => Stream::Next(Err(H3Error::Protocol {
                code: error_code::H3_MISSING_SETTINGS,
                message: format!(
                    "the first control frame must be SETTINGS, got frame type {:#x}",
                    other.ty()
                ),
            })),
            Stream::Next(Ok(None)) => Stream::Next(Err(H3Error::Protocol {
                code: error_code::H3_CLOSED_CRITICAL_STREAM,
                message: "peer closed its control stream".into(),
            })),
            Stream::Next(Err(e)) => Stream::Next(Err(e.into())),
            _ => Stream::Pending(()),
        }
    }

    /// WHY: an HTTP/3 request is one bidirectional stream.
    ///
    /// WHAT: accept the next inbound request stream.
    ///
    /// HOW: `Pending` until one arrives. The returned [`H3Request`] has read
    /// nothing yet — call [`H3Request::poll_headers`].
    pub fn poll_accept(&mut self) -> Stream<Result<H3Request<C::BidiStream>, H3Error>, ()> {
        match self.conn.accept_bidi() {
            Stream::Next(Ok(stream)) => {
                Stream::Next(Ok(H3Request::new(stream, self.max_field_section_size)))
            }
            Stream::Next(Err(e)) => Stream::Next(Err(e.into())),
            _ => Stream::Pending(()),
        }
    }

    /// Open an outbound request stream (client role).
    pub fn poll_open_request(&mut self) -> Stream<Result<H3Request<C::BidiStream>, H3Error>, ()> {
        match self.conn.open_bidi() {
            Stream::Next(Ok(stream)) => {
                Stream::Next(Ok(H3Request::new(stream, self.max_field_section_size)))
            }
            Stream::Next(Err(e)) => Stream::Next(Err(e.into())),
            _ => Stream::Pending(()),
        }
    }

    /// Close the connection with an HTTP/3 error code.
    pub fn close(&mut self, code: u64, reason: &[u8]) {
        self.conn.close(code, reason);
    }
}

/// A unidirectional receive stream whose leading type varint has been stripped.
///
/// WHY: `FramedRecv` wants frames from byte one. A unidirectional stream's first
/// bytes are its type (RFC 9114 §6.2), which is not a frame.
#[derive(Debug)]
struct TypedRecv<R: QuicRecvStream> {
    inner: R,
    /// Bytes read while consuming the type varint but belonging to the frames.
    leftover: Option<Bytes>,
    type_consumed: bool,
    buffer: Vec<u8>,
}

impl<R: QuicRecvStream> TypedRecv<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            leftover: None,
            type_consumed: false,
            buffer: Vec::new(),
        }
    }
}

impl<R: QuicRecvStream> QuicRecvStream for TypedRecv<R> {
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()> {
        if let Some(bytes) = self.leftover.take() {
            return Stream::Next(Ok(Some(bytes)));
        }
        if self.type_consumed {
            return self.inner.read();
        }

        // Accumulate until the type varint is complete, then hand back whatever
        // followed it in the same chunk.
        loop {
            if let Ok(Some((_ty, used))) = super::varint::VarInt::decode(&self.buffer) {
                self.type_consumed = true;
                let rest = Bytes::copy_from_slice(&self.buffer[used..]);
                self.buffer.clear();
                return if rest.is_empty() {
                    self.inner.read()
                } else {
                    Stream::Next(Ok(Some(rest)))
                };
            }

            match self.inner.read() {
                Stream::Next(Ok(Some(bytes))) => self.buffer.extend_from_slice(&bytes),
                Stream::Next(Ok(None)) => return Stream::Next(Ok(None)),
                Stream::Next(Err(e)) => return Stream::Next(Err(e)),
                _ => return Stream::Pending(()),
            }
        }
    }

    fn stop_sending(&mut self, code: u64) {
        self.inner.stop_sending(code);
    }

    fn id(&self) -> crate::quic::StreamId {
        self.inner.id()
    }
}

/// One HTTP/3 request, on its own bidirectional stream.
#[derive(Debug)]
pub struct H3Request<S: QuicBidiStream> {
    framed: FramedRecv<S>,
    max_field_section_size: u64,
    headers_read: bool,
    /// The trailers section, if the peer sent one. A second HEADERS frame after
    /// the body *is* the trailers — and gRPC puts its status there, so discarding
    /// it would make every gRPC call over HTTP/3 fail to report its outcome.
    trailers: Option<Vec<(Bytes, Bytes)>>,
}

impl<S: QuicBidiStream> H3Request<S> {
    fn new(stream: S, max_field_section_size: u64) -> Self {
        Self {
            framed: FramedRecv::new(stream),
            max_field_section_size,
            headers_read: false,
            trailers: None,
        }
    }

    /// The trailers section, once [`Self::poll_body`] has reached the end of the
    /// body. `None` means the peer sent no trailers.
    ///
    /// gRPC's status lives here.
    #[must_use]
    pub fn trailers(&self) -> Option<&[(Bytes, Bytes)]> {
        self.trailers.as_deref()
    }

    /// WHY: a request begins with exactly one HEADERS frame (RFC 9114 §4.1).
    ///
    /// WHAT: read and QPACK-decode the request's field section.
    ///
    /// HOW: `Pending` until the HEADERS frame is complete. A DATA frame before
    /// HEADERS is `H3_FRAME_UNEXPECTED`; a control-only frame here is too.
    /// Unknown frame types are skipped, per §9.
    pub fn poll_headers(&mut self) -> Stream<Result<Vec<(Bytes, Bytes)>, H3Error>, ()> {
        loop {
            match self.framed.poll_frame() {
                Stream::Next(Ok(Some(Frame::Headers(encoded)))) => {
                    self.headers_read = true;
                    let max = usize::try_from(self.max_field_section_size).unwrap_or(usize::MAX);
                    return match super::qpack::decode_field_section(&encoded, max) {
                        Ok(fields) => Stream::Next(Ok(fields)),
                        Err(e) => Stream::Next(Err(H3Error::Protocol {
                            code: error_code::QPACK_DECOMPRESSION_FAILED,
                            message: e.to_string(),
                        })),
                    };
                }
                // RFC 9114 §9: ignore unknown frame types wherever they appear.
                Stream::Next(Ok(Some(Frame::Unknown { .. }))) => continue,
                Stream::Next(Ok(Some(other))) => {
                    return Stream::Next(Err(H3Error::Protocol {
                        code: error_code::H3_FRAME_UNEXPECTED,
                        message: format!(
                            "expected HEADERS to open the request, got frame type {:#x}",
                            other.ty()
                        ),
                    }))
                }
                Stream::Next(Ok(None)) => {
                    return Stream::Next(Err(H3Error::Protocol {
                        code: error_code::H3_MESSAGE_ERROR,
                        message: "request stream ended before HEADERS".into(),
                    }))
                }
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                _ => return Stream::Pending(()),
            }
        }
    }

    /// WHAT: the next chunk of request body.
    ///
    /// HOW: `Next(Ok(Some(bytes)))` per DATA frame, `Next(Ok(None))` at end of
    /// request. A trailing HEADERS frame is the trailers section; it ends the body
    /// just as a clean close does.
    pub fn poll_body(&mut self) -> Stream<Result<Option<Bytes>, H3Error>, ()> {
        loop {
            match self.framed.poll_frame() {
                Stream::Next(Ok(Some(Frame::Data(bytes)))) => return Stream::Next(Ok(Some(bytes))),
                // A second HEADERS frame is the trailers section; the body is over.
                Stream::Next(Ok(Some(Frame::Headers(encoded)))) if self.headers_read => {
                    let max = usize::try_from(self.max_field_section_size).unwrap_or(usize::MAX);
                    return match super::qpack::decode_field_section(&encoded, max) {
                        Ok(fields) => {
                            self.trailers = Some(fields);
                            Stream::Next(Ok(None))
                        }
                        Err(e) => Stream::Next(Err(H3Error::Protocol {
                            code: error_code::QPACK_DECOMPRESSION_FAILED,
                            message: e.to_string(),
                        })),
                    };
                }
                Stream::Next(Ok(Some(Frame::Unknown { .. }))) => continue,
                Stream::Next(Ok(Some(other))) => {
                    return Stream::Next(Err(H3Error::Protocol {
                        code: error_code::H3_FRAME_UNEXPECTED,
                        message: format!(
                            "frame type {:#x} is not allowed on a request stream",
                            other.ty()
                        ),
                    }))
                }
                Stream::Next(Ok(None)) => return Stream::Next(Ok(None)),
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                _ => return Stream::Pending(()),
            }
        }
    }

    /// Write a field section as a HEADERS frame. Drives to completion.
    ///
    /// `Pending` while the flow-control window is full.
    pub fn poll_send_headers(&mut self, pending: &mut Bytes) -> Stream<Result<(), H3Error>, ()> {
        self.poll_send_raw(pending)
    }

    /// Write body bytes as a DATA frame. Drives to completion.
    pub fn poll_send_data(&mut self, pending: &mut Bytes) -> Stream<Result<(), H3Error>, ()> {
        self.poll_send_raw(pending)
    }

    fn poll_send_raw(&mut self, pending: &mut Bytes) -> Stream<Result<(), H3Error>, ()> {
        while pending.has_remaining() {
            match self.framed.get_mut().send(pending) {
                Stream::Next(Ok(_)) => {}
                Stream::Next(Err(e)) => return Stream::Next(Err(e.into())),
                _ => return Stream::Pending(()),
            }
        }
        Stream::Next(Ok(()))
    }

    /// Encode `fields` as a HEADERS frame, ready for [`Self::poll_send_headers`].
    #[must_use]
    pub fn encode_headers<N: AsRef<[u8]>, V: AsRef<[u8]>>(fields: &[(N, V)]) -> Bytes {
        let encoded = super::qpack::encode_field_section(fields);
        Bytes::from(Frame::Headers(Bytes::from(encoded)).to_bytes())
    }

    /// Encode `body` as a DATA frame, ready for [`Self::poll_send_data`].
    #[must_use]
    pub fn encode_data(body: Bytes) -> Bytes {
        Bytes::from(Frame::Data(body).to_bytes())
    }

    /// Finish the send half: no more frames from us.
    pub fn poll_finish(&mut self) -> Stream<Result<(), H3Error>, ()> {
        match self.framed.get_mut().finish() {
            Stream::Next(Ok(())) => Stream::Next(Ok(())),
            Stream::Next(Err(e)) => Stream::Next(Err(e.into())),
            _ => Stream::Pending(()),
        }
    }

    /// Abort the stream with an HTTP/3 error code.
    pub fn reset(&mut self, code: u64) {
        self.framed.get_mut().reset(code);
    }

    /// This request's QUIC stream id.
    pub fn id(&self) -> crate::quic::StreamId {
        QuicSendStream::id(self.framed.get_ref())
    }
}
