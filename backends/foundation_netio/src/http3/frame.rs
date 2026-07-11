//! HTTP/3 frame types and their incremental codec (RFC 9114 §7).
//!
//! WHY: HTTP/3 frames ride on QUIC streams, which deliver bytes in arbitrary
//! chunks. A frame's header can arrive split across two datagrams, and a DATA
//! frame's payload routinely spans many. So the codec has to be *resumable*, which
//! is exactly what Decision 12 §11's [`IncrementalDecoder`] is for.
//!
//! WHAT: [`Frame`] — the frames HTTP/3 defines — and [`FrameDecoder`], an
//! `IncrementalDecoder` that yields one frame at a time.
//!
//! HOW: every frame is `Type (varint) Length (varint) Payload[Length]`. The
//! decoder buffers until the header is complete, then until the payload is, then
//! emits. `Pending` means "call me again with more bytes", never an error.
//!
//! ## Unknown frame types are not errors
//!
//! RFC 9114 §9 requires an endpoint to **ignore** frame types it does not
//! recognise, so the protocol can be extended. A decoder that errors on an unknown
//! type breaks against any peer that speaks a newer draft. [`Frame::Unknown`]
//! carries the type and payload so the caller can skip it.
//!
//! ## Reserved types must not be treated as unknown-but-harmless
//!
//! RFC 9114 §7.2.8 reserves `0x1f * N + 0x21` to exercise exactly that ignore path
//! ("grease"). They decode as [`Frame::Unknown`] like any other.
//!
//! ## Frames that must never appear on a request stream
//!
//! `SETTINGS`, `GOAWAY`, `MAX_PUSH_ID` and `CANCEL_PUSH` belong on the control
//! stream. Receiving one on a request stream is `H3_FRAME_UNEXPECTED`. The decoder
//! surfaces the frame; enforcing where it is legal is the connection's job, because
//! only the connection knows which stream this is.

use std::io::Read;

use bytes::Bytes;

use foundation_core::io::incremental_decoder::AccumulatingBuffer;
use foundation_core::io::{DecodeError, DecodeStep, IncrementalDecoder};

use super::varint::VarInt;

/// Frame type codes (RFC 9114 §11.2.1).
pub mod ty {
    /// `DATA` — request or response body bytes.
    pub const DATA: u64 = 0x00;
    /// `HEADERS` — a QPACK-encoded field section.
    pub const HEADERS: u64 = 0x01;
    /// `CANCEL_PUSH` — abandon a server push.
    pub const CANCEL_PUSH: u64 = 0x03;
    /// `SETTINGS` — connection configuration; control stream only, exactly once.
    pub const SETTINGS: u64 = 0x04;
    /// `PUSH_PROMISE` — a promised server push.
    pub const PUSH_PROMISE: u64 = 0x05;
    /// `GOAWAY` — graceful shutdown.
    pub const GOAWAY: u64 = 0x07;
    /// `MAX_PUSH_ID` — raise the push id limit.
    pub const MAX_PUSH_ID: u64 = 0x0d;
}

/// Setting identifiers (RFC 9114 §11.2.2, RFC 9204 §5).
pub mod setting {
    /// `QPACK_MAX_TABLE_CAPACITY` — 0 disables the QPACK dynamic table.
    pub const QPACK_MAX_TABLE_CAPACITY: u64 = 0x01;
    /// `MAX_FIELD_SECTION_SIZE` — cap on decoded header bytes.
    pub const MAX_FIELD_SECTION_SIZE: u64 = 0x06;
    /// `QPACK_BLOCKED_STREAMS` — 0 means we never block on the dynamic table.
    pub const QPACK_BLOCKED_STREAMS: u64 = 0x07;
    /// `ENABLE_CONNECT_PROTOCOL` (RFC 9220).
    pub const ENABLE_CONNECT_PROTOCOL: u64 = 0x08;
}

/// One HTTP/3 frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// Body bytes.
    Data(Bytes),
    /// A QPACK-encoded field section. Decoding it needs the QPACK decoder.
    Headers(Bytes),
    /// Connection settings, as `(identifier, value)` pairs.
    Settings(Vec<(u64, u64)>),
    /// Graceful shutdown: the last stream id (server) or push id (client) that
    /// will be processed.
    GoAway(VarInt),
    /// Raise the maximum push id.
    MaxPushId(VarInt),
    /// Abandon a push.
    CancelPush(VarInt),
    /// A promised push: its id and its QPACK-encoded field section.
    PushPromise {
        /// The push id being promised.
        push_id: VarInt,
        /// The QPACK-encoded field section of the promised request.
        encoded: Bytes,
    },
    /// A frame type this build does not recognise. RFC 9114 §9 says ignore it.
    Unknown {
        /// The frame type code.
        ty: u64,
        /// The payload, uninterpreted.
        payload: Bytes,
    },
}

impl Frame {
    /// The wire type code for this frame.
    #[must_use]
    pub fn ty(&self) -> u64 {
        match self {
            Frame::Data(_) => ty::DATA,
            Frame::Headers(_) => ty::HEADERS,
            Frame::Settings(_) => ty::SETTINGS,
            Frame::GoAway(_) => ty::GOAWAY,
            Frame::MaxPushId(_) => ty::MAX_PUSH_ID,
            Frame::CancelPush(_) => ty::CANCEL_PUSH,
            Frame::PushPromise { .. } => ty::PUSH_PROMISE,
            Frame::Unknown { ty, .. } => *ty,
        }
    }

    /// WHY: RFC 9114 §7.1 — these frames are only legal on the control stream.
    /// Seeing one on a request stream is `H3_FRAME_UNEXPECTED`.
    ///
    /// WHAT: whether this frame belongs exclusively to the control stream.
    #[must_use]
    pub fn is_control_only(&self) -> bool {
        matches!(
            self,
            Frame::Settings(_) | Frame::GoAway(_) | Frame::MaxPushId(_) | Frame::CancelPush(_)
        )
    }

    /// Whether this frame is legal on a request stream.
    #[must_use]
    pub fn is_request_stream_frame(&self) -> bool {
        matches!(
            self,
            Frame::Data(_) | Frame::Headers(_) | Frame::PushPromise { .. } | Frame::Unknown { .. }
        )
    }

    /// WHY: writing a frame is the mirror of decoding one, and every HTTP/3
    /// sender needs it.
    ///
    /// WHAT: append this frame's wire encoding to `out`.
    ///
    /// HOW: `Type (varint) Length (varint) Payload`. The payload is built first so
    /// the length is known before it is written.
    ///
    /// # Panics
    /// Never panics: every payload we build is well below the varint maximum.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut payload = Vec::new();

        let ty_code = match self {
            Frame::Data(bytes) | Frame::Headers(bytes) => {
                payload.extend_from_slice(bytes);
                self.ty()
            }
            Frame::Settings(pairs) => {
                for (id, value) in pairs {
                    push_varint(*id, &mut payload);
                    push_varint(*value, &mut payload);
                }
                ty::SETTINGS
            }
            Frame::GoAway(v) | Frame::MaxPushId(v) | Frame::CancelPush(v) => {
                v.encode(&mut payload);
                self.ty()
            }
            Frame::PushPromise { push_id, encoded } => {
                push_id.encode(&mut payload);
                payload.extend_from_slice(encoded);
                ty::PUSH_PROMISE
            }
            Frame::Unknown { ty, payload: raw } => {
                payload.extend_from_slice(raw);
                *ty
            }
        };

        push_varint(ty_code, out);
        push_varint(payload.len() as u64, out);
        out.extend_from_slice(&payload);
    }

    /// Encode into a fresh `Vec`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }
}

/// Encode `value` as a varint, saturating at the varint maximum.
///
/// Values above `2^62 - 1` cannot occur here: frame lengths are bounded by the
/// QUIC stream, and every setting/id we produce is a `VarInt` already.
fn push_varint(value: u64, out: &mut Vec<u8>) {
    let v = VarInt::new(value).unwrap_or(VarInt::MAX);
    v.encode(out);
}

/// Where the decoder is in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Reading the type and length varints.
    Header,
    /// Reading `remaining` payload bytes for the frame of type `ty`.
    Payload { ty: u64, len: usize },
}

/// A resumable HTTP/3 frame decoder.
///
/// Feed it whatever bytes a QUIC stream produced; it yields one [`Frame`] per
/// `step` once enough have arrived, and holds partial state otherwise.
#[derive(Debug)]
pub struct FrameDecoder {
    buffer: AccumulatingBuffer,
    state: State,
    /// Refuse absurd frame lengths rather than trying to buffer them.
    max_frame_size: usize,
}

/// The default cap on a single frame's payload: 16 MiB.
///
/// A DATA frame larger than this is a protocol abuse, not a legitimate body — a
/// body is split across many DATA frames precisely so the receiver need not buffer
/// it whole.
pub const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    /// A decoder with the default frame-size cap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: AccumulatingBuffer::new(),
            state: State::Header,
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }

    /// A decoder that refuses frames larger than `max_frame_size` bytes.
    #[must_use]
    pub fn with_max_frame_size(mut self, max_frame_size: usize) -> Self {
        self.max_frame_size = max_frame_size;
        self
    }

    /// Try to parse one frame out of whatever is buffered.
    fn parse(&mut self) -> Result<DecodeStep<Frame>, DecodeError> {
        loop {
            match self.state {
                State::Header => {
                    let view = self.buffer.view();

                    let Some((ty, ty_len)) = VarInt::decode(view)? else {
                        return Ok(DecodeStep::Pending);
                    };
                    let Some((len, len_len)) = VarInt::decode(&view[ty_len..])? else {
                        return Ok(DecodeStep::Pending);
                    };

                    let len = usize::try_from(len.value()).map_err(|_| {
                        DecodeError::protocol("HTTP/3 frame length exceeds this platform's usize")
                    })?;
                    if len > self.max_frame_size {
                        return Err(DecodeError::protocol(format!(
                            "HTTP/3 frame length {len} exceeds the {} byte limit",
                            self.max_frame_size
                        )));
                    }

                    self.buffer.advance(ty_len + len_len);
                    self.state = State::Payload {
                        ty: ty.value(),
                        len,
                    };
                }
                State::Payload { ty, len } => {
                    if self.buffer.len() < len {
                        return Ok(DecodeStep::Pending);
                    }
                    let payload = self.buffer.split_to(len);
                    self.state = State::Header;
                    return decode_payload(ty, payload).map(DecodeStep::Frame);
                }
            }
        }
    }
}

/// Turn a frame's type and payload into a [`Frame`].
fn decode_payload(ty: u64, payload: Bytes) -> Result<Frame, DecodeError> {
    match ty {
        ty::DATA => Ok(Frame::Data(payload)),
        ty::HEADERS => Ok(Frame::Headers(payload)),
        ty::SETTINGS => decode_settings(&payload),
        ty::GOAWAY => single_varint(&payload, "GOAWAY").map(Frame::GoAway),
        ty::MAX_PUSH_ID => single_varint(&payload, "MAX_PUSH_ID").map(Frame::MaxPushId),
        ty::CANCEL_PUSH => single_varint(&payload, "CANCEL_PUSH").map(Frame::CancelPush),
        ty::PUSH_PROMISE => {
            let Some((push_id, used)) = VarInt::decode(&payload)? else {
                return Err(DecodeError::protocol("PUSH_PROMISE frame is truncated"));
            };
            Ok(Frame::PushPromise {
                push_id,
                encoded: payload.slice(used..),
            })
        }
        // RFC 9114 §9: ignore unknown frame types rather than failing. This is
        // what lets a newer peer's grease frames pass through.
        other => Ok(Frame::Unknown { ty: other, payload }),
    }
}

/// A frame whose payload is exactly one varint, and nothing else.
fn single_varint(payload: &[u8], name: &str) -> Result<VarInt, DecodeError> {
    let Some((value, used)) = VarInt::decode(payload)? else {
        return Err(DecodeError::protocol(format!("{name} frame is truncated")));
    };
    if used != payload.len() {
        return Err(DecodeError::protocol(format!(
            "{name} frame has {} trailing byte(s)",
            payload.len() - used
        )));
    }
    Ok(value)
}

/// SETTINGS is a sequence of `(identifier, value)` varint pairs.
fn decode_settings(mut payload: &[u8]) -> Result<Frame, DecodeError> {
    let mut settings = Vec::new();

    while !payload.is_empty() {
        let Some((id, id_len)) = VarInt::decode(payload)? else {
            return Err(DecodeError::protocol(
                "SETTINGS frame truncated mid-identifier",
            ));
        };
        payload = &payload[id_len..];

        let Some((value, value_len)) = VarInt::decode(payload)? else {
            return Err(DecodeError::protocol("SETTINGS frame truncated mid-value"));
        };
        payload = &payload[value_len..];

        // RFC 9114 §7.2.4: a repeated identifier is a connection error.
        if settings.iter().any(|(existing, _)| *existing == id.value()) {
            return Err(DecodeError::protocol(format!(
                "SETTINGS frame repeats identifier {id}"
            )));
        }

        settings.push((id.value(), value.value()));
    }

    Ok(Frame::Settings(settings))
}

impl IncrementalDecoder for FrameDecoder {
    type Frame = Frame;

    fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Self::Frame>, DecodeError> {
        // Try what is already buffered first: a previous step may have left a
        // whole frame behind.
        if let DecodeStep::Frame(frame) = self.parse()? {
            return Ok(DecodeStep::Frame(frame));
        }

        self.buffer.fill_from(src)?;
        self.parse()
    }

    fn has_partial(&self) -> bool {
        !self.buffer.is_empty() || matches!(self.state, State::Payload { .. })
    }
}
