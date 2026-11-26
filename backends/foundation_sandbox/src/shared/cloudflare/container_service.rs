//! The server of the internal `ContainerService`.
//!
//! It answers one [`ContainerControl`]. The Durable Object hosts it beside
//! `SandboxService`, behind the same Worker token (invariant I14).

use std::pin::Pin;
use std::sync::Arc;

use foundation_connectrpc::{ConnectError, ConnectResult, Request, Response, RpcCtx};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::{Stream, StreamExt};

use crate::shared::container::{ContainerControl, ContainerId};
use crate::shared::control::exec_session_with_id;
use crate::shared::error::SandboxError;
use crate::shared::handles::ExecFileCalls;
use crate::shared::ids::ExecId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    ContainerDestroyRequest, ContainerExecInputRequest, ContainerExecRequest,
    ContainerExecSignalRequest, ContainerFetchEvent, ContainerFetchRequest, ContainerGetRequest,
    ContainerGetResponse, ContainerInfo, ContainerListResponse, ContainerLogTailRequest,
    ContainerLogTailResponse, ContainerSignalRequest, ContainerStartRequest, ContainerStopRequest,
    ContainerWaitRequest, Empty, ExecEventKind, ExecEventMsg, ExecInputResponse, SubstrateExit,
};
use crate::shared::rpc::ContainerService;

/// The internal `ContainerService` on one container control.
pub struct ContainerControlService {
    ctx: Ctx,
    control: Arc<dyn ContainerControl>,
    limits: SandboxLimits,
    calls: Arc<dyn ExecFileCalls>,
}

impl ContainerControlService {
    /// A service over one control.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        control: Arc<dyn ContainerControl>,
        limits: SandboxLimits,
        calls: Arc<dyn ExecFileCalls>,
    ) -> Self {
        Self {
            ctx,
            control,
            limits,
            calls,
        }
    }

    fn connect_error(&self, error: &ErrorTrace<SandboxError>) -> ErrorTrace<ConnectError> {
        ErrorTrace::new(
            error
                .current_context()
                .to_connect(self.limits.max_error_message),
        )
    }

    fn not_found() -> ErrorTrace<ConnectError> {
        ErrorTrace::new(
            SandboxError::NotFound {
                what: "container".to_string(),
            }
            .to_connect(64 * 1024),
        )
    }

    async fn handle(
        &self,
        container_id: &str,
    ) -> ConnectResult<crate::shared::container::ContainerHandle> {
        let id = container_id
            .parse::<ContainerId>()
            .map_err(|_| Self::not_found())?;
        self.control
            .get(self.ctx.clone(), &id)
            .await
            .map_err(|error| self.connect_error(&error))?
            .ok_or_else(Self::not_found)
    }
}

impl ContainerService for ContainerControlService {
    async fn start(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerStartRequest>,
    ) -> ConnectResult<Response<ContainerInfo>> {
        let handle = self
            .control
            .start(self.ctx.clone(), req.msg.spec)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(handle.info()))
    }

    async fn get(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerGetRequest>,
    ) -> ConnectResult<Response<ContainerGetResponse>> {
        let id = req
            .msg
            .container_id
            .parse::<ContainerId>()
            .map_err(|_| Self::not_found())?;
        let container = self
            .control
            .get(self.ctx.clone(), &id)
            .await
            .map_err(|error| self.connect_error(&error))?
            .map(|handle| handle.info());
        Ok(Response::new(ContainerGetResponse { container }))
    }

    async fn list(
        &self,
        _ctx: RpcCtx,
        _req: Request<Empty>,
    ) -> ConnectResult<Response<ContainerListResponse>> {
        let containers = self
            .control
            .list(self.ctx.clone())
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(ContainerListResponse { containers }))
    }

    async fn fetch(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerFetchRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<ContainerFetchEvent>> + Send>>> {
        let handle = self.handle(&req.msg.container_id).await?;
        let response = handle
            .fetch(req.msg.port, req.msg.request)
            .await
            .map_err(|error| self.connect_error(&error))?;
        let head = ContainerFetchEvent::Head {
            status: response.status,
            headers: response.headers,
        };
        let max = self.limits.max_error_message;
        let body = response.body.map(move |item| {
            item.map(|data| ContainerFetchEvent::Chunk { data })
                .map_err(|error| ErrorTrace::new(error.current_context().to_connect(max)))
        });
        let stream = futures::stream::once(async move {
            Ok::<ContainerFetchEvent, ErrorTrace<ConnectError>>(head)
        })
        .chain(body);
        Ok(Box::pin(stream))
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerExecRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<ExecEventMsg>> + Send>>> {
        self.handle(&req.msg.container_id).await?;
        let exec_request = req.msg.request;
        let exec_id = exec_request
            .exec_id
            .parse::<ExecId>()
            .map_err(|error| self.connect_error(&ErrorTrace::new(error)))?;
        let command = crate::shared::command::Command {
            argv: exec_request.argv,
            env: exec_request.env,
            cwd: exec_request.cwd,
            stdin: if exec_request.stdin {
                crate::shared::command::StdinMode::Piped
            } else {
                crate::shared::command::StdinMode::Null
            },
            timeout: exec_request
                .timeout_ms
                .map(std::time::Duration::from_millis),
            user: exec_request.user,
        };
        let mut exec = exec_session_with_id(
            self.ctx.clone(),
            self.limits.clone(),
            Arc::clone(&self.calls),
            command,
            exec_id,
        )
        .await
        .map_err(|error| self.connect_error(&error))?;
        let stream = exec.events();
        let max = self.limits.max_error_message;
        let mut seq = 0_u64;
        let mapped = stream.map(move |item| {
            seq = seq.saturating_add(1);
            match item {
                Ok(crate::shared::command::ExecEvent::Stdout(data)) => Ok(ExecEventMsg {
                    seq,
                    event: ExecEventKind::Stdout { data },
                }),
                Ok(crate::shared::command::ExecEvent::Stderr(data)) => Ok(ExecEventMsg {
                    seq,
                    event: ExecEventKind::Stderr { data },
                }),
                Ok(crate::shared::command::ExecEvent::Exit(status)) => Ok(ExecEventMsg {
                    seq,
                    event: ExecEventKind::Exit {
                        code: status.exit_code(),
                        signal: status.terminating_signal(),
                    },
                }),
                Err(error) => Err(ErrorTrace::new(error.current_context().to_connect(max))),
            }
        });
        Ok(Box::pin(mapped))
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        self.handle(&req.msg.container_id).await?;
        let answer = self
            .calls
            .exec_input(req.msg.request)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        self.handle(&req.msg.container_id).await?;
        self.calls
            .exec_signal(req.msg.request)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }

    async fn signal(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let handle = self.handle(&req.msg.container_id).await?;
        handle
            .signal(req.msg.signo)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }

    async fn wait(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerWaitRequest>,
    ) -> ConnectResult<Response<SubstrateExit>> {
        let handle = self.handle(&req.msg.container_id).await?;
        let exit = handle
            .wait()
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(exit))
    }

    async fn stop(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerStopRequest>,
    ) -> ConnectResult<Response<SubstrateExit>> {
        let handle = self.handle(&req.msg.container_id).await?;
        let grace = std::time::Duration::from_millis(req.msg.grace_ms);
        let exit = handle
            .stop(grace)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(exit))
    }

    async fn destroy(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerDestroyRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let handle = self.handle(&req.msg.container_id).await?;
        handle
            .destroy()
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }

    async fn log_tail(
        &self,
        _ctx: RpcCtx,
        req: Request<ContainerLogTailRequest>,
    ) -> ConnectResult<Response<ContainerLogTailResponse>> {
        let handle = self.handle(&req.msg.container_id).await?;
        let data = handle
            .log_tail(req.msg.max)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(ContainerLogTailResponse { data }))
    }
}
