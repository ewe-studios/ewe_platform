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
use futures::Stream;

use crate::shared::command::{ExecEvent, ExitStatus};
use crate::shared::control::SandboxHandle;
use crate::shared::driver::CreateProgress;
use crate::shared::error::SandboxResult;
use crate::shared::ids::ExecId;
use crate::shared::rpc::messages::{
    ExecInputRequest, ExecInputResponse, ExecRequest, ExecSignalRequest, FileReadRequest,
    FileWriteAbortRequest, FileWriteBeginRequest, FileWriteBeginResponse, FileWriteChunkRequest,
    FileWriteChunkResponse, FileWriteCommitRequest, FileWriteCommitResponse,
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

/// The progress iterator of one create.
///
/// `StreamIterator` has a blanket implementation for every `Iterator` whose
/// item is a valtron `Stream`, so this wrapper is also a `StreamIterator`.
pub struct CreateStream(
    Box<
        dyn Iterator<Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>>
            + Send
            + 'static,
    >,
);

impl CreateStream {
    /// Wraps the progress iterator of a create.
    #[must_use]
    pub fn new(
        inner: Box<
            dyn Iterator<Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>>
                + Send
                + 'static,
        >,
    ) -> Self {
        Self(inner)
    }

    /// The wrapped iterator.
    pub fn into_inner(
        self,
    ) -> Box<
        dyn Iterator<Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>>
            + Send
            + 'static,
    > {
        self.0
    }
}

impl Iterator for CreateStream {
    type Item = ValtronStream<SandboxResult<SandboxHandle>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
