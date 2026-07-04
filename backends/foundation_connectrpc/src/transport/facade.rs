//! Typed message facades over the byte seam (Decision 11 §Layering).
//!
//! WHY: `MessageSink<T>` / `MessageSource<T>` are the typed, per-RPC facade the
//! handler machinery sees. They own the `Arc<dyn CodecFor<T>>` (Decision 02) and
//! are the **only** place the concrete `Req`/`Res` is encoded/decoded — the seam
//! below carries only encoded frames + metadata (no `dyn Any`). Every
//! frame-moving method is async; flow control is awaited by the underlying pipe,
//! never surfaced as an error (Decision 03 norm).
//!
//! WHAT: the internal [`MessageSink`] (producer) and [`MessageSource`] (consumer)
//! adapters. Not user-facing — users write async fns / `Stream`s (Decision 04).

use std::marker::PhantomData;
use std::sync::Arc;

use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::SimpleHeaders;

use crate::codec::CodecFor;
use crate::error::{ConnectError, ConnectResult};

use super::conn::{ConnReceiver, ConnSender};

/// Producer facade over a response/request send half — the only place `T` is
/// encoded.
pub struct MessageSink<T> {
    sender: Box<dyn ConnSender>,
    codec: Arc<dyn CodecFor<T>>,
    _marker: PhantomData<fn(T)>,
}

impl<T: Send + 'static> MessageSink<T> {
    /// Build a sink over a send half with the resolved codec.
    #[must_use]
    pub fn new(sender: Box<dyn ConnSender>, codec: Arc<dyn CodecFor<T>>) -> Self {
        Self {
            sender,
            codec,
            _marker: PhantomData,
        }
    }

    /// Set outbound headers (idempotent until the first frame).
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace if the seam rejects the headers.
    pub async fn set_headers(&mut self, headers: SimpleHeaders) -> ConnectResult<()> {
        self.sender.send_headers(headers).await
    }

    /// Encode and send one message. Parks on a full pipe (never `Err(Full)`).
    ///
    /// # Errors
    /// Codec failure, cancellation, or a closed pipe (all as
    /// `ErrorTrace<ConnectError>`).
    pub async fn send(&mut self, msg: T) -> ConnectResult<()> {
        let frame = self.codec.marshal(&msg)?;
        self.sender.send(frame).await
    }

    /// Finish the stream successfully with trailers.
    ///
    /// # Errors
    /// Cancellation or a closed pipe.
    pub async fn close(self, trailers: SimpleHeaders) -> ConnectResult<()> {
        self.sender.close(None, trailers).await
    }

    /// Finish the stream with a terminal error and trailers.
    ///
    /// # Errors
    /// Cancellation or a closed pipe.
    pub async fn close_with_error(
        self,
        error: ErrorTrace<ConnectError>,
        trailers: SimpleHeaders,
    ) -> ConnectResult<()> {
        self.sender.close(Some(error), trailers).await
    }
}

/// Consumer facade over a request/response receive half — the only place `T` is
/// decoded.
pub struct MessageSource<T> {
    receiver: Box<dyn ConnReceiver>,
    codec: Arc<dyn CodecFor<T>>,
    headers: SimpleHeaders,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Send + 'static> MessageSource<T> {
    /// Build a source over a receive half with the resolved codec and the
    /// (already-read) request/response headers.
    #[must_use]
    pub fn new(
        receiver: Box<dyn ConnReceiver>,
        codec: Arc<dyn CodecFor<T>>,
        headers: SimpleHeaders,
    ) -> Self {
        Self {
            receiver,
            codec,
            headers,
            _marker: PhantomData,
        }
    }

    /// Next message, owned-decoded. `Ok(None)` at end of stream. Parks on an
    /// empty pipe; cancellation also wakes it.
    ///
    /// # Errors
    /// Codec failure, a terminal stream error, or cancellation.
    pub async fn receive(&mut self) -> ConnectResult<Option<T>> {
        match self.receiver.receive().await? {
            Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
            None => Ok(None),
        }
    }

    /// The request/response headers.
    #[must_use]
    pub fn headers(&self) -> &SimpleHeaders {
        &self.headers
    }

    /// Trailing metadata observed at end of stream.
    #[must_use]
    pub fn trailers(&self) -> &SimpleHeaders {
        self.receiver.trailers()
    }
}
