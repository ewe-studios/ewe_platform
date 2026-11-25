//! The `SandboxService` definition. Feature 02 and feature 06 implement it.

#![allow(
    unused_variables,
    reason = "the service macro generates default bodies that ignore their parameters"
)]

use foundation_connectrpc::{service, ConnectResult, Request, Response, RpcCtx};

use super::messages::{
    CreateEvent, CreateRequest, Empty, ExecEventMsg, ExecInputRequest, ExecInputResponse,
    ExecRequest, ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest,
    FileWriteBeginRequest, FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse,
    FileWriteCommitRequest, FileWriteCommitResponse, ListResponse, ProbeResponse, SandboxInfo,
    SandboxRef, Scoped,
};
use crate::shared::control::ExitReport;

/// The service that runs in the control plane.
#[service(package = "ewe.sandbox.control.v1", codecs(json))]
pub trait SandboxService {
    /// Asks every registered driver what this host can do.
    async fn probe(
        &self,
        ctx: RpcCtx,
        req: Request<Empty>,
    ) -> ConnectResult<Response<ProbeResponse>>;

    /// Creates one sandbox and streams its progress.
    async fn create(
        &self,
        ctx: RpcCtx,
        req: Request<CreateRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<CreateEvent>>>;

    /// Finds one sandbox.
    async fn get(
        &self,
        ctx: RpcCtx,
        req: Request<SandboxRef>,
    ) -> ConnectResult<Response<SandboxInfo>>;

    /// Lists the live sandboxes.
    async fn list(&self, ctx: RpcCtx, req: Request<Empty>)
        -> ConnectResult<Response<ListResponse>>;

    /// Stops one sandbox.
    async fn stop(
        &self,
        ctx: RpcCtx,
        req: Request<SandboxRef>,
    ) -> ConnectResult<Response<ExitReport>>;

    /// Kills one sandbox.
    async fn kill(
        &self,
        ctx: RpcCtx,
        req: Request<SandboxRef>,
    ) -> ConnectResult<Response<ExitReport>>;

    /// Runs one program in a sandbox and streams its events.
    async fn exec(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<ExecRequest>>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<ExecEventMsg>>>;

    /// Writes one chunk of input to a running program.
    async fn exec_input(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<ExecInputRequest>>,
    ) -> ConnectResult<Response<ExecInputResponse>>;

    /// Sends one signal to a running program.
    async fn exec_signal(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<ExecSignalRequest>>,
    ) -> ConnectResult<Response<Empty>>;

    /// Starts a file write.
    async fn file_write_begin(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<FileWriteBeginRequest>>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>>;

    /// Writes one file chunk.
    async fn file_write_chunk(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<FileWriteChunkRequest>>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>>;

    /// Commits a file write.
    async fn file_write_commit(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<FileWriteCommitRequest>>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>>;

    /// Aborts a file write.
    async fn file_write_abort(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<FileWriteAbortRequest>>,
    ) -> ConnectResult<Response<Empty>>;

    /// Reads one file and streams its chunks.
    async fn file_read(
        &self,
        ctx: RpcCtx,
        req: Request<Scoped<FileReadRequest>>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<FileChunk>>>;
}
