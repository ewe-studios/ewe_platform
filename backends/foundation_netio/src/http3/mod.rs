//! HTTP/3 (RFC 9114) — framing, QPACK, and the mapping onto our QUIC traits (F34).
//!
//! WHY: the third transport. Decision 01 commits to replicating `h3`'s *design*
//! tokio-free: same frame codec, same QPACK, same connection/stream mapping — but
//! driven by valtron over our own [`quic`](crate::quic) trait set rather than by a
//! tokio reactor's `Poll`/`Waker`.
//!
//! WHAT:
//! - [`varint`] — QUIC variable-length integers (RFC 9000 §16). Everything else
//!   is built on these.
//! - [`frame`] — HTTP/3 frames and a resumable [`FrameDecoder`], because a QUIC
//!   stream delivers a frame's bytes in whatever chunks it likes.
//! - [`qpack`] — RFC 9204 field compression, with the dynamic table disabled
//!   (`QPACK_MAX_TABLE_CAPACITY = 0`), which the RFC explicitly permits.
//!
//! HOW: the codec is an `IncrementalDecoder` (Decision 12 §11), so a partial frame
//! is `Pending` and never an error. The transport produces `SimpleIncomingRequest`
//! and consumes `SimpleOutgoingResponse` exactly as HTTP/1.1 and HTTP/2 do — no new
//! handler-facing types (Decision 01 §"Type sufficiency").

pub mod frame;
pub mod qpack;
pub mod types;
pub mod varint;

pub use frame::{Frame, FrameDecoder};
pub use qpack::{decode_field_section, encode_field_section, QpackError};
pub use types::{request_from_fields, response_to_fields, MalformedRequest};
pub use varint::VarInt;
