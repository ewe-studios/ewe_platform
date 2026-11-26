//! An async [`RequestBody`] from a JavaScript `ReadableStream`.
//!
//! WHY: a Worker receives the call body as a `web_sys::ReadableStream`. The
//! router reads a [`RequestBody`] (`futures::Stream<Item = io::Result<Bytes>>`).
//! The in-memory adapter turns a read that is not ready yet into a
//! `WouldBlock` error, because it has no connection to wait on. A wasm body
//! does have one: the reader task in `WasmByteStream` fires a wake when the next
//! chunk arrives.
//!
//! WHAT: [`request_body_from_stream`].
//!
//! HOW: the adapter pulls one bounded chunk at a time from the
//! [`WasmByteStream`] queue. On a retry it registers on the stream's wake
//! registry and returns `Poll::Pending`, so the task parks instead of spinning.
//! The byte limit and the read-ahead bound are the caller's configuration.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use foundation_core::io::readers::Data;
use foundation_netio::wasm::client::stream::WasmByteStream;
use futures::Stream;

use crate::shared::router::RequestBody;

/// A request body from a JavaScript `ReadableStream`.
///
/// `chunks` bounds the queue ahead of the reader. `max` bounds the total bytes
/// (`None` means no limit). A body over `max` ends with an error that names the
/// limit.
///
/// The stream parses the Connect or gRPC-Web envelopes, so the body is the
/// request as it arrives. A `Retry` read parks the caller; it is never an
/// error.
#[must_use]
pub fn request_body_from_stream(
    stream: web_sys::ReadableStream,
    chunks: usize,
    max: Option<usize>,
) -> RequestBody<'static> {
    Box::pin(StreamBody {
        bytes: WasmByteStream::new(stream, chunks, max),
    })
}

/// The stream of one request body.
struct StreamBody {
    bytes: WasmByteStream,
}

impl Stream for StreamBody {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.bytes.next() {
                Some(Ok(Data::Bytes(chunk))) => return Poll::Ready(Some(Ok(Bytes::from(chunk)))),
                Some(Ok(Data::Retry)) => {
                    let waker = cx.waker().clone();
                    self.bytes
                        .register_wake(Box::new(move || waker.wake_by_ref()));
                    // A chunk may have arrived while the callback registered.
                    // Poll once more; a second retry parks the task.
                    return match self.bytes.next() {
                        Some(Ok(Data::Bytes(chunk))) => {
                            Poll::Ready(Some(Ok(Bytes::from(chunk))))
                        }
                        Some(Ok(Data::Retry)) => Poll::Pending,
                        Some(Err(error)) => {
                            Poll::Ready(Some(Err(io::Error::other(error.to_string()))))
                        }
                        None => Poll::Ready(None),
                    };
                }
                Some(Err(error)) => {
                    return Poll::Ready(Some(Err(io::Error::other(error.to_string()))));
                }
                None => return Poll::Ready(None),
            }
        }
    }
}
