//! The `SlaveService` definition. Feature 01 implements it.

#![allow(
    unused_variables,
    reason = "the service macro generates default bodies that ignore their parameters"
)]

use foundation_connectrpc::{service, ConnectResult, Request, Response, RpcCtx};

use super::messages::{
    Empty, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest, ExecSignalRequest,
    FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo,
};

/// The service that runs in the sandbox.
#[service(package = "ewe.sandbox.slave.v1", codecs(json))]
pub trait SlaveService {
    /// Reports what the slave is.
    async fn info(
        &self,
        ctx: RpcCtx,
        req: Request<InfoRequest>,
    ) -> ConnectResult<Response<SlaveInfo>>;

    /// Prepares the root, the environment and the limits of the guest.
    async fn prepare(
        &self,
        ctx: RpcCtx,
        req: Request<PrepareRequest>,
    ) -> ConnectResult<Response<PrepareResponse>>;

    /// Runs one program and streams its events.
    async fn exec(
        &self,
        ctx: RpcCtx,
        req: Request<ExecRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<ExecEventMsg>>>;

    /// Writes one chunk of input to a running program.
    async fn exec_input(
        &self,
        ctx: RpcCtx,
        req: Request<ExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>>;

    /// Sends one signal to a running program.
    async fn exec_signal(
        &self,
        ctx: RpcCtx,
        req: Request<ExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>>;

    /// Starts a file write.
    async fn file_write_begin(
        &self,
        ctx: RpcCtx,
        req: Request<FileWriteBeginRequest>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>>;

    /// Writes one file chunk.
    async fn file_write_chunk(
        &self,
        ctx: RpcCtx,
        req: Request<FileWriteChunkRequest>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>>;

    /// Commits a file write.
    async fn file_write_commit(
        &self,
        ctx: RpcCtx,
        req: Request<FileWriteCommitRequest>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>>;

    /// Aborts a file write.
    async fn file_write_abort(
        &self,
        ctx: RpcCtx,
        req: Request<FileWriteAbortRequest>,
    ) -> ConnectResult<Response<Empty>>;

    /// Reads one file and streams its chunks.
    async fn file_read(
        &self,
        ctx: RpcCtx,
        req: Request<FileReadRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<FileChunk>>>;

    /// Stops the sandbox.
    async fn shutdown(
        &self,
        ctx: RpcCtx,
        req: Request<ShutdownRequest>,
    ) -> ConnectResult<Response<Empty>>;
}
