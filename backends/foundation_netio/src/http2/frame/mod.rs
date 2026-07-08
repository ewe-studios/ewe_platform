//! HTTP/2 frame codec — 9-byte header + all 10 frame types (RFC 7540 §4–6).
//!
//! WHY: Every HTTP/2 message travels as a typed frame with a common 9-byte
//! header (3-byte payload length, 1-byte type, 1-byte flags, 4-byte stream
//! identifier). This module provides typed encode/decode for all frame kinds.
//!
//! WHAT: [`Head`] (the 9-byte header), [`Kind`] (the 10 frame types), and
//! typed payload structs for each kind: [`Data`], [`Headers`], [`Priority`],
//! [`Settings`], [`Ping`], [`GoAway`], [`WindowUpdate`], [`Continuation`],
//! [`Reset`] (RST_STREAM), [`PushPromise`].
//!
//! HOW: Pure encode/decode — no I/O, no state. Composes with
//! [`IncrementalDecoder`] for resumable reads via the `parse` functions.
//!
//! [`IncrementalDecoder`]: foundation_core::io::IncrementalDecoder

use bytes::{BufMut, Bytes, BytesMut};

// ── Constants ───────────────────────────────────────────────────────────────

/// The fixed 9-byte frame header length (RFC 7540 §4.1).
pub const HEADER_LEN: usize = 9;

/// Default maximum frame payload size (RFC 7540 §6.5.2).
pub const DEFAULT_MAX_FRAME_SIZE: u32 = 16_384;

/// Maximum settable frame payload size via SETTINGS_MAX_FRAME_SIZE.
pub const MAX_MAX_FRAME_SIZE: u32 = 16_777_215; // 2^24 - 1

// ── Frame kind ──────────────────────────────────────────────────────────────

/// The 10 HTTP/2 frame types (RFC 7540 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Data = 0,
    Headers = 1,
    Priority = 2,
    Reset = 3,
    Settings = 4,
    PushPromise = 5,
    Ping = 6,
    GoAway = 7,
    WindowUpdate = 8,
    Continuation = 9,
}

impl Kind {
    /// Parse a frame type from its wire byte.
    #[must_use]
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Kind::Data,
            1 => Kind::Headers,
            2 => Kind::Priority,
            3 => Kind::Reset,
            4 => Kind::Settings,
            5 => Kind::PushPromise,
            6 => Kind::Ping,
            7 => Kind::GoAway,
            8 => Kind::WindowUpdate,
            9 => Kind::Continuation,
            _ => Kind::Data, // Unknown types treated as DATA per spec (but flagged by validator)
        }
    }
}

// ── Frame header ────────────────────────────────────────────────────────────

/// The 9-byte HTTP/2 frame header (RFC 7540 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    /// Frame type.
    pub kind: Kind,
    /// Per-frame-type flags.
    pub flag: u8,
    /// Stream identifier (0 for connection-scoped frames).
    pub stream_id: u32,
}

impl Head {
    /// Parse a frame header from the first 9 bytes of a buffer.
    #[must_use]
    pub fn parse(header: &[u8; HEADER_LEN]) -> Self {
        let payload_len = u32::from_be_bytes([0, header[0], header[1], header[2]]);
        let kind = Kind::from_byte(header[3]);
        let flag = header[4];
        let stream_id = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) & 0x7FFF_FFFF;
        // Store payload_len in the unused bits of stream_id representation.
        // Actually, we need to track payload length for encoding. Store it
        // implicitly — the caller reads it via parse_with_len.
        let _ = payload_len;
        Self { kind, flag, stream_id }
    }

    /// Parse a frame header, returning `(head, payload_length)`.
    #[must_use]
    pub fn parse_with_len(header: &[u8; HEADER_LEN]) -> (Self, u32) {
        let payload_len = u32::from_be_bytes([0, header[0], header[1], header[2]]);
        (Self::parse(header), payload_len)
    }

    /// Encode this header into a 9-byte buffer for the given payload length.
    pub fn encode(&self, payload_len: u32, dst: &mut BytesMut) {
        dst.put_uint(payload_len as u64, 3);
        dst.put_u8(self.kind as u8);
        dst.put_u8(self.flag);
        dst.put_u32(self.stream_id);
    }

    /// The encoded length of this header (always 9).
    #[must_use]
    pub const fn encode_len() -> usize {
        HEADER_LEN
    }
}

// ── DATA frame ──────────────────────────────────────────────────────────────

/// DATA frame flags (RFC 7540 §6.1).
pub mod data_flags {
    pub const END_STREAM: u8 = 0x01;
    pub const PADDED: u8 = 0x08;
}

/// A DATA frame (RFC 7540 §6.1).
#[derive(Debug, Clone)]
pub struct DataFrame {
    pub stream_id: u32,
    pub flags: u8,
    pub data: Bytes,
    pub pad_len: Option<u8>,
}

impl DataFrame {
    pub fn new(stream_id: u32, data: impl Into<Bytes>) -> Self {
        Self { stream_id, flags: 0, data: data.into(), pad_len: None }
    }

    pub fn with_end_stream(mut self) -> Self {
        self.flags |= data_flags::END_STREAM;
        self
    }

    /// Parse a DATA frame from header + payload buffer.
    ///
    /// # Errors
    /// Returns an error string on malformed input.
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        let mut data = payload;
        let mut pad_len = None;

        if head.flag & data_flags::PADDED != 0 {
            if data.is_empty() {
                return Err("DATA: PADDED flag set but no pad-length byte");
            }
            pad_len = Some(data[0]);
            data = &data[1..];
            let pl = pad_len.unwrap() as usize;
            if data.len() < pl {
                return Err("DATA: pad length exceeds payload");
            }
            data = &data[..data.len() - pl];
        }

        Ok(Self {
            stream_id: head.stream_id,
            flags: head.flag,
            data: Bytes::copy_from_slice(data),
            pad_len,
        })
    }

    /// Encode this DATA frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let mut payload_len = self.data.len();
        if let Some(pl) = self.pad_len {
            payload_len += 1 + pl as usize;
        }
        let head = Head { kind: Kind::Data, flag: self.flags, stream_id: self.stream_id };
        head.encode(payload_len as u32, dst);
        if let Some(pl) = self.pad_len {
            dst.put_u8(pl);
        }
        dst.put_slice(&self.data);
        if let Some(pl) = self.pad_len {
            dst.put_bytes(0, pl as usize);
        }
    }
}

// ── HEADERS frame ───────────────────────────────────────────────────────────

/// HEADERS frame flags (RFC 7540 §6.2).
pub mod headers_flags {
    pub const END_STREAM: u8 = 0x01;
    pub const END_HEADERS: u8 = 0x04;
    pub const PADDED: u8 = 0x08;
    pub const PRIORITY: u8 = 0x20;
}

/// A HEADERS frame (RFC 7540 §6.2).
#[derive(Debug, Clone)]
pub struct HeadersFrame {
    pub stream_id: u32,
    pub flags: u8,
    pub header_block: Bytes,
    pub pad_len: Option<u8>,
    /// Priority fields (only present when PRIORITY flag is set).
    pub priority: Option<StreamPriority>,
}

/// Stream priority info carried in HEADERS/PRIORITY frames.
#[derive(Debug, Clone, Copy)]
pub struct StreamPriority {
    pub stream_dependency: u32,
    pub exclusive: bool,
    pub weight: u8,
}

impl HeadersFrame {
    pub fn new(stream_id: u32, header_block: impl Into<Bytes>) -> Self {
        Self {
            stream_id,
            flags: headers_flags::END_HEADERS,
            header_block: header_block.into(),
            pad_len: None,
            priority: None,
        }
    }

    pub fn with_end_stream(mut self) -> Self {
        self.flags |= headers_flags::END_STREAM;
        self
    }

    /// Parse a HEADERS frame from header + payload buffer.
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        let mut data = payload;
        let mut pad_len = None;

        if head.flag & headers_flags::PADDED != 0 {
            if data.is_empty() { return Err("HEADERS: PADDED but no pad-length"); }
            pad_len = Some(data[0]);
            data = &data[1..];
        }

        let priority = if head.flag & headers_flags::PRIORITY != 0 {
            if data.len() < 5 { return Err("HEADERS: PRIORITY flag but < 5 bytes"); }
            let dep = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            let exclusive = dep & 0x8000_0000 != 0;
            let stream_dependency = dep & 0x7FFF_FFFF;
            let weight = data[4];
            data = &data[5..];
            Some(StreamPriority { stream_dependency, exclusive, weight })
        } else {
            None
        };

        let pl = pad_len.unwrap_or(0) as usize;
        if data.len() < pl {
            return Err("HEADERS: pad length exceeds payload");
        }
        let header_block = Bytes::copy_from_slice(&data[..data.len() - pl]);

        Ok(Self {
            stream_id: head.stream_id,
            flags: head.flag,
            header_block,
            pad_len,
            priority,
        })
    }

    /// Encode this HEADERS frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let mut payload_len = self.header_block.len();
        let pad = self.pad_len.unwrap_or(0) as usize;
        if pad > 0 { payload_len += 1 + pad; }
        if self.priority.is_some() { payload_len += 5; }

        let head = Head { kind: Kind::Headers, flag: self.flags, stream_id: self.stream_id };
        head.encode(payload_len as u32, dst);

        if pad > 0 { dst.put_u8(pad as u8); }
        if let Some(ref pri) = self.priority {
            let dep = if pri.exclusive { pri.stream_dependency | 0x8000_0000 } else { pri.stream_dependency };
            dst.put_u32(dep);
            dst.put_u8(pri.weight);
        }
        dst.put_slice(&self.header_block);
        if pad > 0 { dst.put_bytes(0, pad); }
    }
}

// ── PRIORITY frame ─────────────────────────────────────────────────────────

/// A PRIORITY frame (RFC 7540 §6.3).
#[derive(Debug, Clone, Copy)]
pub struct PriorityFrame {
    pub stream_id: u32,
    pub priority: StreamPriority,
}

impl PriorityFrame {
    /// Parse a PRIORITY frame (always 5 bytes payload).
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() < 5 {
            return Err("PRIORITY: payload < 5 bytes");
        }
        let dep = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        Ok(Self {
            stream_id: head.stream_id,
            priority: StreamPriority {
                exclusive: dep & 0x8000_0000 != 0,
                stream_dependency: dep & 0x7FFF_FFFF,
                weight: payload[4],
            },
        })
    }

    /// Encode this PRIORITY frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let head = Head { kind: Kind::Priority, flag: 0, stream_id: self.stream_id };
        head.encode(5, dst);
        let dep = if self.priority.exclusive {
            self.priority.stream_dependency | 0x8000_0000
        } else {
            self.priority.stream_dependency
        };
        dst.put_u32(dep);
        dst.put_u8(self.priority.weight);
    }
}

// ── RST_STREAM frame ────────────────────────────────────────────────────────

/// Error codes for RST_STREAM and GOAWAY frames (RFC 7540 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    NoError = 0,
    ProtocolError = 1,
    InternalError = 2,
    FlowControlError = 3,
    SettingsTimeout = 4,
    StreamClosed = 5,
    FrameSizeError = 6,
    RefusedStream = 7,
    Cancel = 8,
    CompressionError = 9,
    ConnectError = 10,
    EnhanceYourCalm = 11,
    InadequateSecurity = 12,
    Http11Required = 13,
}

impl ErrorCode {
    #[must_use]
    pub fn from_u32(code: u32) -> Self {
        match code {
            0 => Self::NoError,
            1 => Self::ProtocolError,
            2 => Self::InternalError,
            3 => Self::FlowControlError,
            4 => Self::SettingsTimeout,
            5 => Self::StreamClosed,
            6 => Self::FrameSizeError,
            7 => Self::RefusedStream,
            8 => Self::Cancel,
            9 => Self::CompressionError,
            10 => Self::ConnectError,
            11 => Self::EnhanceYourCalm,
            12 => Self::InadequateSecurity,
            13 => Self::Http11Required,
            _ => Self::ProtocolError, // Unknown codes = protocol error
        }
    }
}

/// A RST_STREAM frame (RFC 7540 §6.4).
#[derive(Debug, Clone, Copy)]
pub struct ResetFrame {
    pub stream_id: u32,
    pub error_code: ErrorCode,
}

impl ResetFrame {
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() < 4 { return Err("RST_STREAM: payload < 4 bytes"); }
        Ok(Self {
            stream_id: head.stream_id,
            error_code: ErrorCode::from_u32(u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])),
        })
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        let head = Head { kind: Kind::Reset, flag: 0, stream_id: self.stream_id };
        head.encode(4, dst);
        dst.put_u32(self.error_code as u32);
    }
}

// ── SETTINGS frame ──────────────────────────────────────────────────────────

/// SETTINGS parameter identifiers (RFC 7540 §6.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SettingId {
    HeaderTableSize = 1,
    EnablePush = 2,
    MaxConcurrentStreams = 3,
    InitialWindowSize = 4,
    MaxFrameSize = 5,
    MaxHeaderListSize = 6,
}

impl SettingId {
    #[must_use]
    pub fn from_u16(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::HeaderTableSize),
            2 => Some(Self::EnablePush),
            3 => Some(Self::MaxConcurrentStreams),
            4 => Some(Self::InitialWindowSize),
            5 => Some(Self::MaxFrameSize),
            6 => Some(Self::MaxHeaderListSize),
            _ => None,
        }
    }
}

/// SETTINGS frame flags.
pub mod settings_flags {
    pub const ACK: u8 = 0x01;
}

/// A single SETTINGS parameter (identifier + value).
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    pub id: SettingId,
    pub value: u32,
}

/// A SETTINGS frame (RFC 7540 §6.5).
#[derive(Debug, Clone)]
pub struct SettingsFrame {
    pub flags: u8,
    pub settings: Vec<Setting>,
}

impl SettingsFrame {
    pub fn new(settings: Vec<Setting>) -> Self {
        Self { flags: 0, settings }
    }

    pub fn ack() -> Self {
        Self { flags: settings_flags::ACK, settings: Vec::new() }
    }

    pub fn is_ack(&self) -> bool {
        self.flags & settings_flags::ACK != 0
    }

    /// Parse a SETTINGS frame from header + payload.
    /// Each setting is 6 bytes: 2-byte id + 4-byte value.
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if head.flag & settings_flags::ACK != 0 {
            if !payload.is_empty() {
                return Err("SETTINGS: ACK must have empty payload");
            }
            return Ok(Self::ack());
        }
        if payload.len() % 6 != 0 {
            return Err("SETTINGS: payload not multiple of 6");
        }
        let mut settings = Vec::with_capacity(payload.len() / 6);
        for chunk in payload.chunks_exact(6) {
            let id = u16::from_be_bytes([chunk[0], chunk[1]]);
            let value = u32::from_be_bytes([chunk[2], chunk[3], chunk[4], chunk[5]]);
            if let Some(sid) = SettingId::from_u16(id) {
                settings.push(Setting { id: sid, value });
            }
            // Unknown setting IDs are ignored per RFC 7540 §6.5.2.
        }
        Ok(Self { flags: head.flag, settings })
    }

    /// Encode this SETTINGS frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let payload_len = self.settings.len() * 6;
        let head = Head { kind: Kind::Settings, flag: self.flags, stream_id: 0 };
        head.encode(payload_len as u32, dst);
        for s in &self.settings {
            dst.put_u16(s.id as u16);
            dst.put_u32(s.value);
        }
    }
}

// ── PUSH_PROMISE frame ──────────────────────────────────────────────────────

/// PUSH_PROMISE frame flags.
pub mod push_promise_flags {
    pub const END_HEADERS: u8 = 0x04;
    pub const PADDED: u8 = 0x08;
}

/// A PUSH_PROMISE frame (RFC 7540 §6.6).
#[derive(Debug, Clone)]
pub struct PushPromiseFrame {
    pub stream_id: u32,
    pub promised_stream_id: u32,
    pub flags: u8,
    pub header_block: Bytes,
    pub pad_len: Option<u8>,
}

impl PushPromiseFrame {
    /// Parse a PUSH_PROMISE frame.
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        let mut data = payload;
        let mut pad_len = None;

        if head.flag & push_promise_flags::PADDED != 0 {
            if data.is_empty() { return Err("PUSH_PROMISE: PADDED but no pad-length"); }
            pad_len = Some(data[0]);
            data = &data[1..];
        }

        if data.len() < 4 { return Err("PUSH_PROMISE: no promised stream ID"); }
        let promised_stream_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) & 0x7FFF_FFFF;
        data = &data[4..];

        let pl = pad_len.unwrap_or(0) as usize;
        if data.len() < pl { return Err("PUSH_PROMISE: pad exceeds payload"); }
        let header_block = Bytes::copy_from_slice(&data[..data.len() - pl]);

        Ok(Self {
            stream_id: head.stream_id,
            promised_stream_id,
            flags: head.flag,
            header_block,
            pad_len,
        })
    }

    /// Encode this PUSH_PROMISE frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let mut payload_len = 4 + self.header_block.len();
        let pad = self.pad_len.unwrap_or(0) as usize;
        if pad > 0 { payload_len += 1 + pad; }

        let head = Head { kind: Kind::PushPromise, flag: self.flags, stream_id: self.stream_id };
        head.encode(payload_len as u32, dst);
        if pad > 0 { dst.put_u8(pad as u8); }
        dst.put_u32(self.promised_stream_id);
        dst.put_slice(&self.header_block);
        if pad > 0 { dst.put_bytes(0, pad); }
    }
}

// ── PING frame ──────────────────────────────────────────────────────────────

/// PING frame flags.
pub mod ping_flags {
    pub const ACK: u8 = 0x01;
}

/// A PING frame (RFC 7540 §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingFrame {
    pub flags: u8,
    pub opaque_data: [u8; 8],
}

impl PingFrame {
    pub fn new(opaque_data: [u8; 8]) -> Self {
        Self { flags: 0, opaque_data }
    }

    pub fn ack(opaque_data: [u8; 8]) -> Self {
        Self { flags: ping_flags::ACK, opaque_data }
    }

    pub fn is_ack(&self) -> bool {
        self.flags & ping_flags::ACK != 0
    }

    /// Parse a PING frame (always 8 bytes).
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() != 8 { return Err("PING: payload != 8 bytes"); }
        let mut data = [0u8; 8];
        data.copy_from_slice(payload);
        Ok(Self { flags: head.flag, opaque_data: data })
    }

    /// Encode this PING frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let head = Head { kind: Kind::Ping, flag: self.flags, stream_id: 0 };
        head.encode(8, dst);
        dst.put_slice(&self.opaque_data);
    }
}

// ── GOAWAY frame ────────────────────────────────────────────────────────────

/// A GOAWAY frame (RFC 7540 §6.8).
#[derive(Debug, Clone)]
pub struct GoAwayFrame {
    pub last_stream_id: u32,
    pub error_code: ErrorCode,
    pub debug_data: Bytes,
}

impl GoAwayFrame {
    pub fn new(last_stream_id: u32, error_code: ErrorCode) -> Self {
        Self { last_stream_id, error_code, debug_data: Bytes::new() }
    }

    /// Parse a GOAWAY frame (8+ bytes: 4-byte last stream + 4-byte error + debug).
    pub fn parse(_head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() < 8 { return Err("GOAWAY: payload < 8 bytes"); }
        Ok(Self {
            last_stream_id: u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) & 0x7FFF_FFFF,
            error_code: ErrorCode::from_u32(u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]])),
            debug_data: Bytes::copy_from_slice(&payload[8..]),
        })
    }

    /// Encode this GOAWAY frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let payload_len = 8 + self.debug_data.len();
        let head = Head { kind: Kind::GoAway, flag: 0, stream_id: 0 };
        head.encode(payload_len as u32, dst);
        dst.put_u32(self.last_stream_id);
        dst.put_u32(self.error_code as u32);
        dst.put_slice(&self.debug_data);
    }
}

// ── WINDOW_UPDATE frame ─────────────────────────────────────────────────────

/// A WINDOW_UPDATE frame (RFC 7540 §6.9).
#[derive(Debug, Clone, Copy)]
pub struct WindowUpdateFrame {
    pub stream_id: u32,
    pub size_increment: u32,
}

impl WindowUpdateFrame {
    /// Parse a WINDOW_UPDATE frame (always 4 bytes, but last bit reserved).
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        if payload.len() != 4 { return Err("WINDOW_UPDATE: payload != 4 bytes"); }
        let increment = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) & 0x7FFF_FFFF;
        if increment == 0 {
            return Err("WINDOW_UPDATE: increment must be > 0");
        }
        Ok(Self { stream_id: head.stream_id, size_increment: increment })
    }

    /// Encode this WINDOW_UPDATE frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let head = Head { kind: Kind::WindowUpdate, flag: 0, stream_id: self.stream_id };
        head.encode(4, dst);
        dst.put_u32(self.size_increment);
    }
}

// ── CONTINUATION frame ──────────────────────────────────────────────────────

/// CONTINUATION frame flags.
pub mod continuation_flags {
    pub const END_HEADERS: u8 = 0x04;
}

/// A CONTINUATION frame (RFC 7540 §6.10).
#[derive(Debug, Clone)]
pub struct ContinuationFrame {
    pub stream_id: u32,
    pub flags: u8,
    pub header_block: Bytes,
}

impl ContinuationFrame {
    pub fn new(stream_id: u32, header_block: impl Into<Bytes>) -> Self {
        Self { stream_id, flags: continuation_flags::END_HEADERS, header_block: header_block.into() }
    }

    /// Parse a CONTINUATION frame.
    pub fn parse(head: &Head, payload: &[u8]) -> Result<Self, &'static str> {
        Ok(Self {
            stream_id: head.stream_id,
            flags: head.flag,
            header_block: Bytes::copy_from_slice(payload),
        })
    }

    /// Encode this CONTINUATION frame into `dst`.
    pub fn encode(&self, dst: &mut BytesMut) {
        let head = Head { kind: Kind::Continuation, flag: self.flags, stream_id: self.stream_id };
        head.encode(self.header_block.len() as u32, dst);
        dst.put_slice(&self.header_block);
    }
}
