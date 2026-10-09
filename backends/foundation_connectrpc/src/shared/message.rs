//! Typed request / response wrappers handed to handlers (Decision 04 §Request &
//! Response Wrappers).
//!
//! WHY: A handler's `async fn` receives the decoded message **plus** its call
//! metadata (spec, peer, headers) and returns the decoded response **plus** the
//! headers/trailers it wants on the wire. These wrappers carry that envelope so
//! the codec/seam stay internal — the handler never touches `Bytes`, codecs, or
//! the frame pipes.
//!
//! WHAT: [`Request<T>`] (inbound message + read-only call metadata + mutable
//! request headers) and [`Response<T>`] (outbound message + mutable
//! headers/trailers). The generic registration wrappers (Decision 08) build a
//! `Request<Req>` from the decoded frame and turn the returned `Response<Res>`
//! back into `(bytes, headers, trailers)`.
//!
//! HOW: plain owned structs — no `Arc`, no interior mutability. Built once per
//! call at the codec boundary and moved into / out of the handler future.

use foundation_netio::shared::http::SimpleHeaders;

use crate::shared::context::{Peer, Spec};

/// The decoded request handed to a handler: the message plus its call metadata.
pub struct Request<T> {
    /// The decoded request message.
    pub msg: T,
    spec: Spec,
    peer: Peer,
    headers: SimpleHeaders,
}

impl<T> Request<T> {
    /// Build a bare request around a message (empty metadata) — used by clients
    /// and tests; the server dispatch path uses [`with_parts`](Self::with_parts).
    #[must_use]
    pub fn new(msg: T) -> Self {
        Self {
            msg,
            spec: Spec::empty(false),
            peer: Peer::empty(),
            headers: SimpleHeaders::new(),
        }
    }

    /// Build a request with the call metadata the dispatcher resolved.
    #[must_use]
    pub fn with_parts(msg: T, spec: Spec, peer: Peer, headers: SimpleHeaders) -> Self {
        Self {
            msg,
            spec,
            peer,
            headers,
        }
    }

    /// The procedure spec.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The remote peer.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    /// The request headers.
    #[must_use]
    pub fn headers(&self) -> &SimpleHeaders {
        &self.headers
    }

    /// Mutable request headers.
    pub fn headers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.headers
    }

    /// Consume the wrapper, yielding the message.
    #[must_use]
    pub fn into_message(self) -> T {
        self.msg
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Request")
            .field("msg", &self.msg)
            .field("spec", &self.spec)
            .field("peer", &self.peer)
            .finish_non_exhaustive()
    }
}

/// The response a handler returns: the message plus the headers and trailers it
/// wants on the wire.
pub struct Response<T> {
    /// The response message to encode.
    pub msg: T,
    headers: SimpleHeaders,
    trailers: SimpleHeaders,
}

impl<T> Response<T> {
    /// Build a response around a message with empty headers/trailers.
    #[must_use]
    pub fn new(msg: T) -> Self {
        Self {
            msg,
            headers: SimpleHeaders::new(),
            trailers: SimpleHeaders::new(),
        }
    }

    /// The response headers.
    #[must_use]
    pub fn headers(&self) -> &SimpleHeaders {
        &self.headers
    }

    /// Mutable response headers.
    pub fn headers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.headers
    }

    /// The response trailers.
    #[must_use]
    pub fn trailers(&self) -> &SimpleHeaders {
        &self.trailers
    }

    /// Mutable response trailers.
    pub fn trailers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.trailers
    }

    /// Decompose into `(msg, headers, trailers)` — used by the dispatcher after
    /// the handler returns.
    #[must_use]
    pub fn into_parts(self) -> (T, SimpleHeaders, SimpleHeaders) {
        (self.msg, self.headers, self.trailers)
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Response<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Response")
            .field("msg", &self.msg)
            .field("headers", &self.headers.len())
            .field("trailers", &self.trailers.len())
            .finish()
    }
}
