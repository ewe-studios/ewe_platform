//! The `ContainerService` definition: the internal container control of one
//! Durable Object.
//!
//! It is not part of `SandboxService`. It has its own package, so a client that
//! only controls a container does not need the sandbox messages. The Worker
//! entry of feature 08 routes both services to the Durable Object.

#![allow(
    unused_variables,
    reason = "the service macro generates default bodies that ignore their parameters"
)]

use foundation_connectrpc::{service, ConnectResult, Request, Response, RpcCtx};

use super::messages::{
    ContainerDestroyRequest, ContainerExecRequest, ContainerFetchEvent, ContainerFetchRequest,
    ContainerGetRequest, ContainerGetResponse, ContainerInfo, ContainerListResponse,
    ContainerLogTailRequest, ContainerLogTailResponse, ContainerSignalRequest,
    ContainerStartRequest, ContainerStopRequest, ContainerWaitRequest, Empty, ExecEventMsg,
    SubstrateExit,
};

/// The container control of one Durable Object.
#[service(package = "ewe.sandbox.container.v1", codecs(json))]
pub trait ContainerService {
    /// Starts one container and returns its information.
    async fn start(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerStartRequest>,
    ) -> ConnectResult<Response<ContainerInfo>>;

    /// Finds the container of this Durable Object.
    async fn get(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerGetRequest>,
    ) -> ConnectResult<Response<ContainerGetResponse>>;

    /// Lists the containers of this control.
    async fn list(
        &self,
        ctx: RpcCtx,
        req: Request<Empty>,
    ) -> ConnectResult<Response<ContainerListResponse>>;

    /// Sends one request to a port and streams the response.
    async fn fetch(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerFetchRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<ContainerFetchEvent>>>;

    /// Runs one command and streams its events.
    async fn exec(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerExecRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<ExecEventMsg>>>;

    /// Sends one signal to the container.
    async fn signal(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerSignalRequest>,
    ) -> ConnectResult<Response<Empty>>;

    /// Waits for the container to exit.
    async fn wait(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerWaitRequest>,
    ) -> ConnectResult<Response<SubstrateExit>>;

    /// Stops the container with a grace period.
    async fn stop(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerStopRequest>,
    ) -> ConnectResult<Response<SubstrateExit>>;

    /// Removes the container and every resource it holds.
    async fn destroy(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerDestroyRequest>,
    ) -> ConnectResult<Response<Empty>>;

    /// The last bytes of the container log.
    async fn log_tail(
        &self,
        ctx: RpcCtx,
        req: Request<ContainerLogTailRequest>,
    ) -> ConnectResult<Response<ContainerLogTailResponse>>;
}
