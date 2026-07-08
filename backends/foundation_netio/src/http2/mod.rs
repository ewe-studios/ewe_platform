//! HTTP/2 binary-frame layer — tokio-free, owned implementation with `h2` as design reference.
//!
//! WHY: HTTP/2 (and gRPC over it) requires a frame codec, HPACK, flow control,
//! and a stream state machine — all operating on non-blocking fds. The existing
//! `simple_http/` module handles text-based HTTP/1.1; this is its binary sibling
//! (Decision 12 §5).
//!
//! WHAT: Direction-neutral building blocks:
//!   - [`flow_control`] — per-stream + per-connection window arithmetic
//!   - `frame` — 9-byte header codec, all 10 frame types (on [`IncrementalDecoder`])
//!   - `hpack` — header compression/decompression (RFC 7541)
//!   - `settings` — SETTINGS frame encode/decode
//!   - `stream` — stream state machine (idle → open → half-closed → closed)
//!
//! HOW: Each module is a standalone pure-Rust state machine with no tokio or I/O
//! dependency. Multiplexers and connection-level orchestration are in Feature 30;
//! this crate (Feature 29) builds the substrate they run on.
//!
//! [`IncrementalDecoder`]: foundation_core::io::IncrementalDecoder

pub mod flow_control;
pub mod frame;
pub mod hpack;
pub mod settings;
pub mod stream;
pub mod connection;
pub mod channel;
pub mod conn;
pub mod types;
pub mod client;
pub mod server;
pub mod detect;
