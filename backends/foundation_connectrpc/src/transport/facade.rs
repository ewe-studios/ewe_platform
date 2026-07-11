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

use bytes::Bytes;
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeaders;

use crate::codec::CodecFor;
use crate::error::{ConnectError, ConnectResult};

use super::conn::{ConnReceiver, ConnSender};

/// Per-procedure message-middleware over the **codec-encoded frame bytes +
/// metadata** (Decision 11 §Facade message-middleware). Sees the uncompressed
/// frame (compression stays the outermost byte step, Decision 06).
pub trait FrameMiddleware: Send + Sync + 'static {
    /// Transform an outbound frame before it reaches the seam.
    ///
    /// # Errors
    /// Any middleware-specific failure as `ErrorTrace<ConnectError>`.
    fn on_send(&self, frame: Bytes, headers: &SimpleHeaders) -> ConnectResult<Bytes> {
        let _ = headers;
        Ok(frame)
    }
    /// Transform an inbound frame after the seam, before decode.
    ///
    /// # Errors
    /// Any middleware-specific failure.
    fn on_receive(&self, frame: Bytes, headers: &SimpleHeaders) -> ConnectResult<Bytes> {
        let _ = headers;
        Ok(frame)
    }
}

/// Per-procedure typed message-middleware (validation, redaction, defaulting) —
/// the facade decodes once and lends the typed message (Decision 11).
pub trait TypedMiddleware<T>: Send + Sync + 'static {
    /// Inspect an outbound message before encoding.
    ///
    /// # Errors
    /// Any middleware-specific failure (e.g. validation).
    fn on_send(&self, msg: &T) -> ConnectResult<()> {
        let _ = msg;
        Ok(())
    }
    /// Inspect/mutate an inbound message after decoding, before the handler.
    ///
    /// # Errors
    /// Any middleware-specific failure.
    fn on_receive(&self, msg: &mut T) -> ConnectResult<()> {
        let _ = msg;
        Ok(())
    }
}

/// Producer facade over a response/request send half — the only place `T` is
/// encoded.
pub struct MessageSink<T> {
    sender: Box<dyn ConnSender>,
    codec: Arc<dyn CodecFor<T>>,
    typed_mw: Vec<Arc<dyn TypedMiddleware<T>>>,
    frame_mw: Vec<Arc<dyn FrameMiddleware>>,
    _marker: PhantomData<fn(T)>,
}

impl<T: Send + 'static> MessageSink<T> {
    /// Build a sink over a send half with the resolved codec.
    #[must_use]
    pub fn new(sender: Box<dyn ConnSender>, codec: Arc<dyn CodecFor<T>>) -> Self {
        Self {
            sender,
            codec,
            typed_mw: Vec::new(),
            frame_mw: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Install a typed message-middleware (runs before encode, in order).
    #[must_use]
    pub fn with_typed_middleware(mut self, mw: Arc<dyn TypedMiddleware<T>>) -> Self {
        self.typed_mw.push(mw);
        self
    }

    /// Install a frame middleware (runs after encode, in order).
    #[must_use]
    pub fn with_frame_middleware(mut self, mw: Arc<dyn FrameMiddleware>) -> Self {
        self.frame_mw.push(mw);
        self
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
        // Ordering (Decision 11): typed middleware → marshal → frame middleware → seam.
        for mw in &self.typed_mw {
            mw.on_send(&msg)?;
        }
        let mut frame = self.codec.marshal(&msg)?;
        // Headers are already on the wire by send-time; frame middleware inspect
        // the encoded bytes.
        let empty = SimpleHeaders::new();
        for mw in &self.frame_mw {
            frame = mw.on_send(frame, &empty)?;
        }
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
    typed_mw: Vec<Arc<dyn TypedMiddleware<T>>>,
    frame_mw: Vec<Arc<dyn FrameMiddleware>>,
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
            typed_mw: Vec::new(),
            frame_mw: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Install a typed message-middleware (runs after decode, in order).
    #[must_use]
    pub fn with_typed_middleware(mut self, mw: Arc<dyn TypedMiddleware<T>>) -> Self {
        self.typed_mw.push(mw);
        self
    }

    /// Install a frame middleware (runs before decode, in order).
    #[must_use]
    pub fn with_frame_middleware(mut self, mw: Arc<dyn FrameMiddleware>) -> Self {
        self.frame_mw.push(mw);
        self
    }

    /// Next message, owned-decoded. `Ok(None)` at end of stream. Parks on an
    /// empty pipe; cancellation also wakes it.
    ///
    /// # Errors
    /// Codec failure, a terminal stream error, or cancellation.
    pub async fn receive(&mut self) -> ConnectResult<Option<T>> {
        // Ordering (Decision 11): seam → frame middleware → unmarshal → typed middleware.
        match self.receiver.receive().await? {
            Some(mut frame) => {
                for mw in &self.frame_mw {
                    frame = mw.on_receive(frame, &self.headers)?;
                }
                let mut msg = self.codec.unmarshal(frame)?;
                for mw in &self.typed_mw {
                    mw.on_receive(&mut msg)?;
                }
                Ok(Some(msg))
            }
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
