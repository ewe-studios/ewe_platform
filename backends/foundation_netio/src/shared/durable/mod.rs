//! The Durable Streams client core.
//!
//! The core speaks the protocol and injects its transport. It has no socket,
//! runtime or valtron type. A native host uses the `foundation_netio` client as
//! its transport; a wasm host uses `fetch`; a test uses a mock.

mod client;
mod transport;

pub use client::{
    AppendResult, ClientConfig, ClientError, DurableClient, HeadResult, ReadBuilder, ReadChunk,
    ReadMode, ReadResult, StreamHandle,
};
pub use transport::{
    ClientFuture, ClientRequest, ClientResponse, ClientSseStream, ClientTransport, HttpMethod,
    SseEvent, TransportError,
};
