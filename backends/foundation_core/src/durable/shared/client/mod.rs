//! The Durable Streams client core.
//!
//! The core defines the client and the transport trait it expects. A native
//! or web host implements [`ClientTransport`] above this crate.

mod core;
mod transport;

pub use core::{
    AppendResult, ClientConfig, ClientError, DurableClient, HeadResult, ReadBuilder, ReadChunk,
    ReadMode, ReadResult, SseRead, StreamHandle,
};
pub use transport::{
    ClientFuture, ClientRequest, ClientResponse, ClientSseStream, ClientTransport, HttpMethod,
    SseEvent, SseOpen, TransportError,
};
