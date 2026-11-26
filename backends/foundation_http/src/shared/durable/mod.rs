//! The HTTP adapter helpers for Durable Streams.
//!
//! The adapter owns the wire rules. The core stays transport-neutral.

pub mod parse;
pub mod status;

pub use parse::{
    decode_offset, encode_offset, parse_expires_at, parse_offset, parse_producer,
    parse_stream_closed, parse_stream_seq, parse_sub_offset, parse_ttl,
};
pub use status::{header, status_for};
