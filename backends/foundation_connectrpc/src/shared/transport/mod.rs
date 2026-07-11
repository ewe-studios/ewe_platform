//! Transport seam & streaming model (Decision 11) — the byte/frame seam that
//! decouples the three RPC protocols from every transport.
//!
//! Layering, end to end:
//! ```text
//! transport ⇄ BYTES (ByteSink/ByteSource) ⇄ protocol reader/writer tasks
//!           ⇄ FramePipes (Frame) ⇄ conn halves ⇄ typed facade ⇄ handler
//! ```
//!
//! See the sub-modules: [`frame`] (the `Frame`/`FramePipe` + async plumbing),
//! [`conn`] (the `HandlerConn`/`ClientConn` seam + pipe-backed impls),
//! [`facade`] (the typed `MessageSink`/`MessageSource`), [`capabilities`]
//! (capability matching), and [`transport`] (the client `Transport` trait).

pub mod base;
pub mod capabilities;
pub mod conn;
pub mod facade;
pub mod frame;
// H1 + WS ride `foundation_netio`'s platform-selecting client/websocket wrappers
// (F51/F52), so they are cross-platform. The native-only H2/H3 transports live in
// [`crate::native::transport`].
pub mod h1;
pub mod wasm;
pub mod ws;

pub use base::{
    body_stream_from_pipe, head_stream_from_pipe, BodyStream, ByteSink, ByteSource, HeadSource,
    HeadStream, SendBody, Transport, TransportError, TransportStream,
};
pub use capabilities::{
    check_compatible, requirements, CallRequirements, ProtocolKind, TransportCapabilities,
};
pub use conn::{
    ClientConn, ClientConnEnds, ClientReceiver, ClientSender, ConnReceiver, ConnSender,
    HandlerConn, HandlerConnEnds, PipeClientConn, PipeHandlerConn,
};
pub use facade::{FrameMiddleware, MessageSink, MessageSource, TypedMiddleware};
pub use frame::{race, BoxFuture, Either, Frame, FramePipe, DEFAULT_PIPE_DEPTH};
