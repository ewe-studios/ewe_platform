//! The handle layer over the low-level execution and file calls.
//!
//! Feature 02 adds the behaviour: it implements [`ExecHandleOps`],
//! [`ExecStdinOps`] and [`ExecFileCalls`]. This module declares the types and
//! the delegating methods, so the control plane, the drivers and the remote
//! client share one shape.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_core::valtron::Stream as ValtronStream;
use foundation_core::valtron::{
    SendWakeCallback, StreamIterator, WakeCallback, WakeResult, WakeableStream,
};
use futures::Stream;

use crate::shared::command::{ExecEvent, ExitStatus};
use crate::shared::control::SandboxHandle;
use crate::shared::driver::CreateProgress;
use crate::shared::error::SandboxResult;
use crate::shared::ids::ExecId;
use crate::shared::rpc::messages::{
    ExecInputRequest, ExecInputResponse, ExecRequest, ExecSignalRequest, FileReadRequest,
    FileWriteAbortRequest, FileWriteBeginRequest, FileWriteBeginResponse, FileWriteChunkRequest,
    FileWriteChunkResponse, FileWriteCommitRequest, FileWriteCommitResponse, UnpackAbortRequest,
    UnpackBeginRequest, UnpackBeginResponse, UnpackChunkRequest, UnpackChunkResponse,
    UnpackCommitRequest, UnpackCommitResponse,
};
use crate::shared::{BoxFuture, ExecEventStream, FileChunkStream};

/// The low-level execution and file calls of one sandbox.
///
/// [`SlaveClient`](crate::shared::control) and the scoped `SandboxService`
/// client of feature 06 implement this trait.
pub trait ExecFileCalls: Send + Sync + 'static {
    /// Opens one execution stream.
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<ExecEventStream>>;

    /// Writes one input chunk.
    fn exec_input(
        &self,
        request: ExecInputRequest,
    ) -> BoxFuture<'_, SandboxResult<ExecInputResponse>>;

    /// Sends one signal.
    fn exec_signal(&self, request: ExecSignalRequest) -> BoxFuture<'_, SandboxResult<()>>;

    /// Starts a file write.
    fn file_write_begin(
        &self,
        request: FileWriteBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteBeginResponse>>;

    /// Writes one file chunk.
    fn file_write_chunk(
        &self,
        request: FileWriteChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteChunkResponse>>;

    /// Commits a file write.
    fn file_write_commit(
        &self,
        request: FileWriteCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteCommitResponse>>;

    /// Aborts a file write.
    fn file_write_abort(&self, request: FileWriteAbortRequest) -> BoxFuture<'_, SandboxResult<()>>;

    /// Reads one file.
    fn file_read(&self, request: FileReadRequest) -> BoxFuture<'_, SandboxResult<FileChunkStream>>;

    /// Starts an image unpack (feature 12).
    fn unpack_begin(
        &self,
        request: UnpackBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackBeginResponse>>;

    /// Writes one chunk of the tar stream.
    fn unpack_chunk(
        &self,
        request: UnpackChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackChunkResponse>>;

    /// Ends the tar stream and applies the archive.
    fn unpack_commit(
        &self,
        request: UnpackCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackCommitResponse>>;

    /// Ends an unpack without applying it.
    fn unpack_abort(&self, request: UnpackAbortRequest) -> BoxFuture<'_, SandboxResult<()>>;
}

/// The operations behind one [`ExecHandle`].
///
/// Feature 02 implements this trait for a live slave session.
pub trait ExecHandleOps: Send + Sync + 'static {
    /// The process identifier in the guest.
    fn pid(&self) -> u32;

    /// The input writer, when the command uses piped input. It is present one
    /// time.
    fn stdin(&self) -> Option<ExecStdin>;

    /// The event stream of the program.
    fn events(&self) -> ExecEvents;

    /// Sends one signal to the program.
    fn signal(&self, signal: i32) -> BoxFuture<'_, SandboxResult<()>>;

    /// Waits for the program and reads and discards its output.
    fn wait(&self) -> BoxFuture<'_, SandboxResult<ExitStatus>>;

    /// Whether the command passed its time limit.
    ///
    /// The default is `false` for a handle that has no timer.
    fn timed_out(&self) -> bool {
        false
    }
}

/// A handle to one running program.
#[derive(Clone)]
pub struct ExecHandle {
    exec_id: ExecId,
    ops: Arc<dyn ExecHandleOps>,
}

impl ExecHandle {
    /// A handle over an execution and its operations.
    #[must_use]
    pub fn new(exec_id: ExecId, ops: Arc<dyn ExecHandleOps>) -> Self {
        Self { exec_id, ops }
    }

    /// The execution identifier.
    #[must_use]
    pub fn exec_id(&self) -> &ExecId {
        &self.exec_id
    }

    /// The process identifier in the guest.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.ops.pid()
    }

    /// The input writer, when the command uses piped input. It is present one
    /// time.
    pub fn stdin(&mut self) -> Option<ExecStdin> {
        self.ops.stdin()
    }

    /// The event stream of the program.
    pub fn events(&mut self) -> ExecEvents {
        self.ops.events()
    }

    /// Sends one signal to the program.
    ///
    /// # Errors
    /// The error of the slave call.
    pub async fn signal(&self, signal: i32) -> SandboxResult<()> {
        self.ops.signal(signal).await
    }

    /// Waits for the program and reads and discards its output.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::ExecTimeout`] when the command
    /// passed its time limit, or the error of the slave call.
    pub async fn wait(self) -> SandboxResult<ExitStatus> {
        self.ops.wait().await
    }

    /// Whether the command passed its time limit.
    #[must_use]
    pub fn timed_out(&self) -> bool {
        self.ops.timed_out()
    }
}

/// The operations behind one [`ExecStdin`].
pub trait ExecStdinOps: Send + Sync + 'static {
    /// Writes one piece of input.
    fn write(&self, bytes: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>>;

    /// Closes the input.
    fn close(&self) -> BoxFuture<'_, SandboxResult<()>>;
}

/// The input writer of one running program.
#[derive(Clone)]
pub struct ExecStdin {
    ops: Arc<dyn ExecStdinOps>,
}

impl ExecStdin {
    /// An input writer over its operations.
    #[must_use]
    pub fn new(ops: Arc<dyn ExecStdinOps>) -> Self {
        Self { ops }
    }

    /// Writes bytes to the program.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::InputQueueFull`] when the program
    /// does not read for the retry time, or the error of the slave call.
    pub async fn write(&mut self, bytes: &[u8]) -> SandboxResult<()> {
        self.ops.write(bytes.to_vec()).await
    }

    /// Closes the input.
    ///
    /// # Errors
    /// The error of the slave call.
    pub async fn close(self) -> SandboxResult<()> {
        self.ops.close().await
    }
}

/// The event stream of one running program.
pub struct ExecEvents(Pin<Box<dyn Stream<Item = SandboxResult<ExecEvent>> + Send>>);

impl ExecEvents {
    /// Wraps a stream of execution events.
    #[must_use]
    pub fn new(inner: Pin<Box<dyn Stream<Item = SandboxResult<ExecEvent>> + Send>>) -> Self {
        Self(inner)
    }

    /// The wrapped stream.
    pub fn into_inner(self) -> Pin<Box<dyn Stream<Item = SandboxResult<ExecEvent>> + Send>> {
        self.0
    }
}

impl Stream for ExecEvents {
    type Item = SandboxResult<ExecEvent>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().0.as_mut().poll_next(context)
    }
}

/// The chunk stream of one file read.
pub struct FileStream(Pin<Box<dyn Stream<Item = SandboxResult<Vec<u8>>> + Send>>);

impl FileStream {
    /// Wraps a stream of byte chunks.
    #[must_use]
    pub fn new(inner: Pin<Box<dyn Stream<Item = SandboxResult<Vec<u8>>> + Send>>) -> Self {
        Self(inner)
    }

    /// The wrapped stream.
    pub fn into_inner(self) -> Pin<Box<dyn Stream<Item = SandboxResult<Vec<u8>>> + Send>> {
        self.0
    }
}

impl Stream for FileStream {
    type Item = SandboxResult<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().0.as_mut().poll_next(context)
    }
}

/// The wake-carrying source of a [`CreateStream`].
///
/// The create progress iterator is a driving stream with a wake. A trait
/// object over [`WakeableStream`] cannot keep that wake: `StreamIterator` is a
/// supertrait with a blanket implementation, so a projection through a wrapper
/// does not resolve. This trait names the concrete item type once, and both the
/// item and the wake cross the box.
pub trait CreateSource: Send {
    /// The next item of the create stream.
    fn next_item(&mut self) -> Option<ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>>;
    /// Register a wake for the next item.
    fn wake(&mut self, cb: WakeCallback) -> WakeResult;
    /// Register a wake that may fire on another thread.
    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult;
    /// Announce an async consumer before the first pull.
    fn announce(&mut self) {}
    /// Whether `next_item` runs the producer.
    fn drives_its_producer(&self) -> bool {
        false
    }
    /// Whether the driving stream can make progress now.
    fn can_make_progress(&self) -> bool {
        true
    }
}

#[cfg(target_family = "wasm")]
impl<T> CreateSource for foundation_compact::SendWrapper<T>
where
    T: WakeableStream + StreamIterator<D = SandboxResult<SandboxHandle>, P = CreateProgress>,
{
    fn next_item(&mut self) -> Option<ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>> {
        Iterator::next(&mut **self)
    }

    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        (**self).wake(cb)
    }

    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        (**self).wake_send(cb)
    }

    fn announce(&mut self) {
        (**self).announce();
    }

    fn drives_its_producer(&self) -> bool {
        (**self).drives_its_producer()
    }

    fn can_make_progress(&self) -> bool {
        (**self).can_make_progress()
    }
}

#[cfg(not(target_family = "wasm"))]
impl<T> CreateSource for foundation_compact::SendWrapper<T>
where
    T: WakeableStream + StreamIterator<D = SandboxResult<SandboxHandle>, P = CreateProgress> + Send,
{
    fn next_item(&mut self) -> Option<ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>> {
        Iterator::next(&mut **self)
    }

    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        (**self).wake(cb)
    }

    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        (**self).wake_send(cb)
    }

    fn announce(&mut self) {
        (**self).announce();
    }

    fn drives_its_producer(&self) -> bool {
        (**self).drives_its_producer()
    }

    fn can_make_progress(&self) -> bool {
        (**self).can_make_progress()
    }
}

/// The progress iterator of one create.
pub struct CreateStream {
    inner: Box<dyn CreateSource>,
    /// Runs when the stream is dropped before it finishes: it cancels the
    /// create and removes any substrate it started.
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}

impl CreateStream {
    /// Wraps the progress iterator of a create.
    #[must_use]
    pub fn new(inner: Box<dyn CreateSource>, on_drop: Option<Box<dyn FnOnce() + Send>>) -> Self {
        Self { inner, on_drop }
    }
}

impl Drop for CreateStream {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            on_drop();
        }
    }
}

impl Iterator for CreateStream {
    type Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_item()
    }
}

impl WakeableStream for CreateStream {
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        self.inner.wake(cb)
    }

    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        self.inner.wake_send(cb)
    }

    fn announce(&mut self) {
        self.inner.announce();
    }

    fn drives_its_producer(&self) -> bool {
        self.inner.drives_its_producer()
    }

    fn can_make_progress(&self) -> bool {
        self.inner.can_make_progress()
    }
}

/// A stream that cannot deliver a wake.
///
/// Used for the immediate error of a failed create: it yields one item and
/// then ends, so it needs no readiness signal.
pub struct NoWake<I>(I);

impl<I> NoWake<I> {
    /// Wraps an iterator that cannot signal readiness.
    #[must_use]
    pub fn new(inner: I) -> Self {
        Self(inner)
    }
}

impl<I> Iterator for NoWake<I>
where
    I: Iterator<Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>>,
{
    type Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<I> CreateSource for NoWake<I>
where
    I: Iterator<Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>> + Send,
{
    fn next_item(&mut self) -> Option<ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>> {
        self.0.next()
    }

    fn wake(&mut self, _cb: WakeCallback) -> WakeResult {
        WakeResult::NotSupported
    }

    fn wake_send(&mut self, _cb: SendWakeCallback) -> WakeResult {
        WakeResult::NotSupported
    }
}
