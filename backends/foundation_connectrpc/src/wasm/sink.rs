//! A streaming [`ResponseSink`] for wasm hosts.
//!
//! WHY: a Connect or gRPC-Web call on a Worker or a page must send each frame
//! as the handler produces it. A sink that collects the whole reply in memory
//! defeats server streaming (root rule 4). The production `foundation_http`
//! builders (`CfConn::into_response`, `WasmResponse`) build a `Vec<u8>`; this
//! sink is the streaming counterpart.
//!
//! WHAT: [`ConnectSink`] implements [`ResponseSink`] over a JS
//! `web_sys::Response` whose body is a pull-based `ReadableStream` built by
//! `foundation_netio::wasm::response::streaming_response_with`. [`whole`] sends
//! one response; [`open`] sends the head, each [`chunk`] sends one frame, and
//! [`finish`] closes the body.
//!
//! HOW: a bounded `futures` multi-producer single-consumer (mpsc) channel
//! carries the frames. `streaming_response_with` pulls one frame per request,
//! so a slow client slows the handler and memory stays flat. A dropped
//! `Response` drops the source, which ends the call (rule 7 and rule 10).
//!
//! [`whole`]: ResponseSink::whole
//! [`open`]: ResponseSink::open
//! [`chunk`]: ResponseSink::chunk
//! [`finish`]: ResponseSink::finish

use std::io;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_compact::SendWrapper;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeaders, SimpleOutgoingResponse};

use crate::shared::router::ResponseSink;
use crate::shared::transport::BoxFuture;

/// The frame channel capacity: the number of frames held ahead of the client.
///
/// It is the client's `body_queue_chunks` default, so the server and client
/// read-ahead bounds are the same number. The frames wait in the channel until
/// the runtime pulls them.
fn frame_capacity() -> usize {
    foundation_netio::shared::client::ClientConfig::default().body_queue_chunks
}

/// A shared slot for the built response and the frame sender.
///
/// The `web_sys::Response` is a JavaScript value; [`SendWrapper`] lets it cross
/// the `Send` bound of [`ResponseSink`]. It is sound because wasm is
/// single-threaded.
struct SinkState {
    response: Option<SendWrapper<web_sys::Response>>,
    sender: Option<futures::channel::mpsc::Sender<Vec<u8>>>,
}

/// A [`ResponseSink`] that streams its reply to a JS `Response`.
///
/// [`ConnectSink::new`] returns the sink and a [`ConnectSinkHandle`]. Move the
/// sink into `ConnectRpcHandler::dispatch_to`; after it resolves, take the
/// response from the handle.
pub struct ConnectSink {
    inner: Arc<Mutex<SinkState>>,
}

/// The caller's handle to the response that a [`ConnectSink`] builds.
pub struct ConnectSinkHandle {
    inner: Arc<Mutex<SinkState>>,
}

impl ConnectSink {
    /// Create a sink. Its response is not reachable; use
    /// [`ConnectSink::new_pair`] when the caller must take the response.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SinkState {
                response: None,
                sender: None,
            })),
        }
    }

    /// Create a sink and a handle to the response it builds.
    #[must_use]
    pub fn new_pair() -> (Self, ConnectSinkHandle) {
        let sink = Self::new();
        let handle = ConnectSinkHandle {
            inner: Arc::clone(&sink.inner),
        };
        (sink, handle)
    }

    /// Build the streaming response for `head` and store the sender.
    fn start(&mut self, status: usize, headers: &SimpleHeaders) -> io::Result<()> {
        let status = u16::try_from(status).unwrap_or(500);

        let (sender, receiver) = futures::channel::mpsc::channel::<Vec<u8>>(frame_capacity());
        let response =
            foundation_netio::wasm::response::streaming_response_with(status, headers, receiver)
                .map_err(|e| {
                    io::Error::other(format!("failed to build streaming response: {e:?}"))
                })?;

        let mut state = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("response sink lock poisoned"))?;
        state.response = Some(SendWrapper::new(response));
        state.sender = Some(sender);
        Ok(())
    }

    /// The frame sender, cloned out of the lock so it can be used across an
    /// await.
    fn sender(&self) -> io::Result<futures::channel::mpsc::Sender<Vec<u8>>> {
        self.inner
            .lock()
            .map_err(|_| io::Error::other("response sink lock poisoned"))?
            .sender
            .clone()
            .ok_or_else(|| io::Error::other("response sink is not open"))
    }

    /// Send one frame and wait for room when the client is slow.
    async fn send_frame(&self, bytes: Vec<u8>) -> io::Result<()> {
        use futures::SinkExt;
        let mut sender = self.sender()?;
        sender
            .send(bytes)
            .await
            .map_err(|_| io::Error::other("response stream closed by the client"))
    }

    /// Send a body that is already in memory as frames.
    async fn send_body(&self, body: SendSafeBody) -> io::Result<()> {
        let mut iter = SendSafeBodyBytesIterator::new(body);
        loop {
            match iter.next() {
                Some(foundation_core::valtron::Stream::Next(
                    SendSafeBodyBytesItem::Chunk(bytes),
                )) => self.send_frame(bytes.to_vec()).await?,
                Some(foundation_core::valtron::Stream::Next(
                    SendSafeBodyBytesItem::StreamError(error),
                )) => {
                    return Err(io::Error::other(format!(
                        "response body read failed: {error}"
                    )))
                }
                Some(foundation_core::valtron::Stream::Wait) => {
                    return Err(io::Error::other(
                        "a whole response body must be ready; it streamed instead",
                    ))
                }
                Some(foundation_core::valtron::Stream::Ignore) => {}
                Some(_) => {}
                None => return Ok(()),
            }
        }
    }

    /// Close the frame channel, so the `ReadableStream` ends after the queued
    /// frames.
    fn close(&mut self) {
        if let Ok(mut state) = self.inner.lock() {
            if let Some(mut sender) = state.sender.take() {
                sender.close_channel();
            }
        }
    }
}

impl Default for ConnectSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseSink for ConnectSink {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let status: usize = response.status.clone().into();
            let body = response.body;
            self.start(status, &response.headers)?;
            if let Some(body) = body {
                self.send_body(body).await?;
            }
            self.close();
            Ok(())
        })
    }

    fn open(&mut self, head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let status: usize = head.status.clone().into();
            let body = head.body;
            self.start(status, &head.headers)?;
            if let Some(body) = body {
                self.send_body(body).await?;
            }
            Ok(())
        })
    }

    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move { self.send_frame(bytes.to_vec()).await })
    }

    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            self.close();
            Ok(())
        })
    }
}

impl ConnectSinkHandle {
    /// The response the sink built, or `None` when dispatch wrote nothing.
    #[must_use]
    pub fn into_response(self) -> Option<web_sys::Response> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut state| state.response.take())
            .map(SendWrapper::into_inner)
    }
}

impl std::fmt::Debug for ConnectSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectSink").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ConnectSinkHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectSinkHandle").finish_non_exhaustive()
    }
}
