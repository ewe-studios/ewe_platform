//! The daemon's streamed answers, as async streams of typed items.
//!
//! WHY: Several endpoints answer with bodies of unbounded size — image and
//! container tarballs, followed logs, the `/events` feed — or with a JSON-lines
//! progress stream the daemon writes *while* it pulls, pushes, builds or loads.
//! Collecting such a body means holding a whole image in memory, or never
//! returning for a feed that does not end. A pull or push also fails *inside*
//! its `200` stream, as an `{"errorDetail": …}` line, so the status alone says
//! nothing about the outcome.
//!
//! WHAT: Thin wrappers over the generated functions' bodies
//! ([`JsonLinesStream`] / [`ByteStream`] from `foundation_deployment`), each an
//! async stream yielding `Result<_, DockerError>`:
//!
//! - [`ProgressStream`] — pull ([`CreateImageInfo`]), push ([`PushImageInfo`]),
//!   build and load ([`BuildInfo`]) progress. An in-band error ends it with
//!   that error, mapped by the endpoint ([`DockerError::PullFailed`],
//!   [`DockerError::PushFailed`], [`DockerError::BuildFailed`]).
//! - [`EventStream`] — `/events`, one [`EventMessage`] per event.
//! - [`ByteBody`] — raw chunks (tar archives).
//! - [`LogStream`] — demultiplexed stdout/stderr frames (logs, attach, exec).
//!
//! HOW: The generated function checks the status before the body is touched
//! (reading at most a bounded amount of an error body) and hands the body back
//! unread. Each wrapper reads it as its items are asked for — `next().await`,
//! or through [`futures_lite::Stream`] — and holds only the item it is
//! assembling. Dropping one closes the connection.

use core::pin::Pin;
use core::task::{Context, Poll};
use std::future::poll_fn;

use bytes::Bytes;
use foundation_deployment::providers::common::{ByteStream, JsonLinesStream};
use futures_lite::Stream as FuturesStream;
use serde::de::DeserializeOwned;

use crate::error::DockerError;
use crate::generated::create::CreateImageInfo;
use crate::generated::events::EventMessage;
use crate::generated::push::PushImageInfo;
use crate::generated::shared::{BuildInfo, ErrorDetail};
use crate::streaming::decoder::{LogFrameDecoder, LogOutput};

/// The largest multiplexed frame a [`LogStream`] buffers. dockerd writes
/// frames of at most a few tens of kilobytes; a header announcing more than
/// this is treated as a corrupt stream rather than buffered.
pub const MAX_LOG_FRAME: usize = 16 * 1024 * 1024;

// ── progress ────────────────────────────────────────────────────────────

/// A progress item that can report a failure in-band.
pub trait ProgressItem: DeserializeOwned + Unpin {
    /// The failure this item reports, if it is the daemon's `errorDetail`.
    fn error_message(&self) -> Option<String>;
}

/// `errorDetail.message`, or — for a detail with no message — its code.
fn detail_message(detail: Option<&ErrorDetail>) -> Option<String> {
    let detail = detail?;
    Some(match (&detail.message, detail.code) {
        (Some(message), _) => message.clone(),
        (None, Some(code)) => format!("error code {code}"),
        (None, None) => "unspecified error".to_string(),
    })
}

impl ProgressItem for CreateImageInfo {
    fn error_message(&self) -> Option<String> {
        detail_message(self.error_detail.as_ref())
    }
}

impl ProgressItem for PushImageInfo {
    fn error_message(&self) -> Option<String> {
        detail_message(self.error_detail.as_ref())
    }
}

impl ProgressItem for BuildInfo {
    fn error_message(&self) -> Option<String> {
        detail_message(self.error_detail.as_ref())
    }
}

/// A JSON-lines progress stream (pull, push, build, load).
///
/// The daemon does the work while it writes this stream: read it to its end
/// ([`Self::finish`]) before relying on the result. Ends after the first error.
pub struct ProgressStream<T> {
    lines: JsonLinesStream<T>,
    on_error: fn(String) -> DockerError,
    done: bool,
}

impl<T: ProgressItem> ProgressStream<T> {
    /// Wrap a progress body; an in-band error becomes `on_error(message)`.
    #[must_use]
    pub fn new(lines: JsonLinesStream<T>, on_error: fn(String) -> DockerError) -> Self {
        Self {
            lines,
            on_error,
            done: false,
        }
    }

    /// The next progress item, `None` once the daemon has finished.
    ///
    /// # Errors
    ///
    /// The in-band failure (e.g. [`DockerError::PullFailed`]), a malformed line
    /// ([`DockerError::JsonParse`]) or a failed read
    /// ([`DockerError::Transport`]); the stream ends after it.
    pub async fn next(&mut self) -> Option<Result<T, DockerError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }

    /// Read the stream to its end, discarding the items: the operation has
    /// finished when this returns `Ok`.
    ///
    /// # Errors
    ///
    /// As [`Self::next`].
    pub async fn finish(mut self) -> Result<(), DockerError> {
        while let Some(item) = self.next().await {
            item?;
        }
        Ok(())
    }
}

impl ProgressStream<BuildInfo> {
    /// Read a build's (or load's) output to its end and return the id of the
    /// image it reports, from the final `aux` line. Each output line is logged
    /// at `debug` and dropped, not kept.
    ///
    /// # Errors
    ///
    /// As [`Self::next`] — a failed build is [`DockerError::BuildFailed`].
    pub async fn image_id(mut self) -> Result<Option<String>, DockerError> {
        let mut image_id = None;
        while let Some(item) = self.next().await {
            let item = item?;
            if let Some(text) = item.stream.as_deref() {
                tracing::debug!(target: "docker::build", "{}", text.trim_end());
            }
            if let Some(id) = item.aux.and_then(|aux| aux.id) {
                image_id = Some(id);
            }
        }
        Ok(image_id)
    }
}

impl<T> core::fmt::Debug for ProgressStream<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProgressStream")
            .field("lines", &self.lines)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl<T: ProgressItem> FuturesStream for ProgressStream<T> {
    type Item = Result<T, DockerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        let item = match Pin::new(&mut this.lines).poll_next(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => {
                this.done = true;
                return Poll::Ready(None);
            }
            Poll::Ready(Some(item)) => item,
        };
        Poll::Ready(Some(match item {
            Ok(item) => match item.error_message() {
                Some(message) => {
                    this.done = true;
                    Err((this.on_error)(message))
                }
                None => Ok(item),
            },
            Err(e) => {
                this.done = true;
                Err(e.into())
            }
        }))
    }
}

// ── events ──────────────────────────────────────────────────────────────

/// The `/events` feed, one [`EventMessage`] per event. Without an `until`,
/// the daemon keeps it open: it ends only when the caller drops it.
pub struct EventStream {
    lines: JsonLinesStream<EventMessage>,
}

impl EventStream {
    /// Wrap an `/events` body.
    #[must_use]
    pub fn new(lines: JsonLinesStream<EventMessage>) -> Self {
        Self { lines }
    }

    /// The next event, `None` once the daemon closes the feed.
    ///
    /// # Errors
    ///
    /// A malformed event ([`DockerError::JsonParse`]) or a failed read
    /// ([`DockerError::Transport`]); the stream ends after it.
    pub async fn next(&mut self) -> Option<Result<EventMessage, DockerError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

impl core::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventStream")
            .field("lines", &self.lines)
            .finish()
    }
}

impl FuturesStream for EventStream {
    type Item = Result<EventMessage, DockerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().lines)
            .poll_next(cx)
            .map(|item| item.map(|r| r.map_err(DockerError::from)))
    }
}

// ── raw bytes ───────────────────────────────────────────────────────────

/// A raw response body (an image or container tarball), chunk by chunk.
pub struct ByteBody {
    bytes: ByteStream,
}

impl ByteBody {
    /// Wrap a raw body.
    #[must_use]
    pub fn new(bytes: ByteStream) -> Self {
        Self { bytes }
    }

    /// The next chunk, `None` once the body has ended.
    ///
    /// # Errors
    ///
    /// [`DockerError::Transport`] if reading fails; the stream ends after it.
    pub async fn next(&mut self) -> Option<Result<Bytes, DockerError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }

    /// Read the rest of the body, discarding it; returns its length.
    ///
    /// # Errors
    ///
    /// [`DockerError::Transport`] if reading fails.
    pub async fn drain(self) -> Result<u64, DockerError> {
        Ok(self.bytes.drain().await?)
    }
}

impl core::fmt::Debug for ByteBody {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ByteBody")
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl FuturesStream for ByteBody {
    type Item = Result<Bytes, DockerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().bytes)
            .poll_next(cx)
            .map(|item| item.map(|r| r.map_err(DockerError::from)))
    }
}

// ── multiplexed logs ────────────────────────────────────────────────────

/// A multiplexed stdout/stderr body (logs, attach, exec), one frame at a time.
///
/// Ends after the body ends or after the first error: a body that stops
/// inside a frame, or a frame larger than [`MAX_LOG_FRAME`], is
/// [`DockerError::JsonParse`] (a malformed answer); a failed read is
/// [`DockerError::Transport`].
pub struct LogStream {
    bytes: ByteStream,
    decoder: LogFrameDecoder,
    done: bool,
}

impl LogStream {
    /// Wrap a multiplexed body.
    #[must_use]
    pub fn new(bytes: ByteStream) -> Self {
        Self {
            bytes,
            decoder: LogFrameDecoder::new(),
            done: false,
        }
    }

    /// The next frame, `None` once the body has ended.
    ///
    /// # Errors
    ///
    /// See [`LogStream`]; the stream ends after an error.
    pub async fn next(&mut self) -> Option<Result<LogOutput, DockerError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }

    /// Read the stream to its end, appending stdout and stderr (lossy UTF-8)
    /// to the two strings as the frames arrive.
    ///
    /// For output the caller is going to keep — a one-shot command's result,
    /// a readiness check's logs. A followed log never ends: read it with
    /// [`Self::next`] instead.
    ///
    /// # Errors
    ///
    /// See [`LogStream`].
    pub async fn read_into(
        mut self,
        stdout: &mut String,
        stderr: &mut String,
    ) -> Result<(), DockerError> {
        while let Some(frame) = self.next().await {
            match frame? {
                LogOutput::StdOut { message } => {
                    stdout.push_str(&String::from_utf8_lossy(&message))
                }
                LogOutput::StdErr { message } => {
                    stderr.push_str(&String::from_utf8_lossy(&message))
                }
                LogOutput::StdIn { .. } | LogOutput::Console { .. } => {}
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for LogStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LogStream")
            .field("buffered", &self.decoder.buffered_len())
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl FuturesStream for LogStream {
    type Item = Result<LogOutput, DockerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.done {
                return Poll::Ready(None);
            }
            let before = this.decoder.buffered_len();
            if let Some(frame) = this.decoder.decode() {
                this.decoder.compact();
                return Poll::Ready(Some(Ok(frame)));
            }
            // A frame of an unknown stream type is skipped (decode moved past
            // it): look for the next frame before reading more.
            if this.decoder.buffered_len() < before {
                this.decoder.compact();
                continue;
            }
            if this.decoder.buffered_len() > MAX_LOG_FRAME + 8 {
                this.done = true;
                return Poll::Ready(Some(Err(DockerError::JsonParse(format!(
                    "multiplexed stream frame larger than {MAX_LOG_FRAME} bytes"
                )))));
            }
            match Pin::new(&mut this.bytes).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => this.decoder.feed(&chunk),
                Poll::Ready(Some(Err(e))) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e.into())));
                }
                Poll::Ready(None) => {
                    this.done = true;
                    let left = this.decoder.buffered_len();
                    if left > 0 {
                        return Poll::Ready(Some(Err(DockerError::JsonParse(format!(
                            "multiplexed stream ended inside a frame ({left} bytes left over)"
                        )))));
                    }
                    return Poll::Ready(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_netio::shared::http::SendSafeBody;

    fn block_on<F: core::future::Future>(fut: F) -> F::Output {
        let mut fut = core::pin::pin!(fut);
        let mut cx = Context::from_waker(core::task::Waker::noop());
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    fn pull(text: &str) -> ProgressStream<CreateImageInfo> {
        ProgressStream::new(
            JsonLinesStream::new(SendSafeBody::Bytes(text.as_bytes().to_vec())),
            DockerError::PullFailed,
        )
    }

    #[test]
    fn progress_items_are_typed() {
        let mut s = pull("{\"status\":\"Pulling\",\"id\":\"latest\",\"progressDetail\":{\"current\":1,\"total\":4}}\n");
        let item = block_on(s.next()).expect("item").expect("ok");
        assert_eq!(item.status.as_deref(), Some("Pulling"));
        assert_eq!(item.progress_detail.and_then(|p| p.total), Some(4));
        assert!(block_on(s.next()).is_none());
    }

    #[test]
    fn an_in_band_error_ends_the_stream_with_the_endpoints_error() {
        let s = pull(
            "{\"status\":\"a\"}\n{\"errorDetail\":{\"message\":\"denied\"},\"error\":\"denied\"}\n{\"status\":\"never\"}\n",
        );
        let err = block_on(s.finish()).expect_err("in-band error");
        assert!(
            matches!(&err, DockerError::PullFailed(m) if m == "denied"),
            "{err:?}"
        );
    }

    #[test]
    fn a_build_reports_the_image_it_built() {
        let s = ProgressStream::<BuildInfo>::new(
            JsonLinesStream::new(SendSafeBody::Text(
                "{\"stream\":\"Step 1/1 : FROM scratch\\n\"}\n{\"aux\":{\"ID\":\"sha256:abc\"}}\n"
                    .into(),
            )),
            DockerError::BuildFailed,
        );
        assert_eq!(
            block_on(s.image_id()).expect("built").as_deref(),
            Some("sha256:abc")
        );
    }

    #[test]
    fn log_frames_are_demultiplexed_and_a_truncated_frame_is_an_error() {
        let mut raw = vec![1, 0, 0, 0, 0, 0, 0, 3];
        raw.extend_from_slice(b"out");
        raw.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 3]);
        raw.extend_from_slice(b"err");
        let (mut out, mut err) = (String::new(), String::new());
        block_on(
            LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw.clone())))
                .read_into(&mut out, &mut err),
        )
        .expect("frames");
        assert_eq!((out.as_str(), err.as_str()), ("out", "err"));

        raw.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 9, b'x']);
        let result = block_on(
            LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw))).read_into(&mut out, &mut err),
        );
        assert!(
            matches!(result, Err(DockerError::JsonParse(_))),
            "{result:?}"
        );
    }

    #[test]
    fn an_oversized_frame_header_is_an_error_not_a_huge_buffer() {
        let mut raw = vec![1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
        raw.extend(std::iter::repeat_n(b'x', MAX_LOG_FRAME + 16));
        let mut s = LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw)));
        let first = block_on(s.next()).expect("an error");
        assert!(
            matches!(first, Err(DockerError::JsonParse(ref m)) if m.contains("larger than")),
            "{first:?}"
        );
    }
}
