//! The Durable Streams client core.
//!
//! The core defines the client and the transport trait it expects. A native
//! or web host implements [`ClientTransport`] above this crate.
//!
//! The client repeats transient failures through [`RetryPolicy`], evaluates
//! dynamic values through [`DynamicHeaders`] once per attempt, and writes
//! idempotently through a [`Producer`].

mod core;
mod dynamic;
mod producer;
mod retry;
mod transport;

pub use core::{
    AppendResult, ClientConfig, ClientError, DurableClient, HeadResult, ReadBuilder, ReadChunk,
    ReadMode, ReadResult, SseRead, StreamHandle,
};
pub use dynamic::DynamicHeaders;
pub use producer::{
    Producer, ProducerBuilder, ProducerConfig, DEFAULT_LINGER, DEFAULT_MAX_BATCH_BYTES,
    DEFAULT_MAX_BATCH_ITEMS, DEFAULT_MAX_EPOCH_CLAIMS, DEFAULT_MAX_IN_FLIGHT,
    DEFAULT_MAX_QUEUE_BYTES,
};
pub use retry::{
    RetryPolicy, DEFAULT_INITIAL_BACKOFF, DEFAULT_JITTER, DEFAULT_MAX_ATTEMPTS,
    DEFAULT_MAX_BACKOFF, DEFAULT_MULTIPLIER, DEFAULT_RETRY_AFTER_CAP, DEFAULT_RETRY_STATUSES,
};
pub use transport::{
    ClientFuture, ClientRequest, ClientResponse, ClientSseStream, ClientTransport, HttpMethod,
    SseEvent, SseOpen, TransportError,
};
