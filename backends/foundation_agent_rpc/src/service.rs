//! The `AgentService` implementation and its connectrpc `Router`.
//!
//! WHY: clients reach instances through one generated RPC surface (Connect,
//! gRPC and gRPC-Web from the same router). This module is the only place that
//! turns RPC requests into registry/session operations.
//!
//! WHAT: [`AgentRpcService`] implements the generated
//! [`AgentService`](crate::proto::v1::agent_service::AgentService) trait over an
//! [`InstanceRegistry`]; [`agent_router`] registers it on a `Router`.
//!
//! HOW: handlers are async. A turn runs on valtron (`AgentSession::
//! run_turn_stream` schedules the agent loop) and is consumed here as an async
//! stream through valtron's parking bridge (`into_future_stream`), so a handler
//! never blocks waiting on inference. `ChatStream` forwards each turn event as
//! it arrives — nothing is buffered; `Chat` gathers one turn's records (bounded
//! by the instance's token limits) because its reply is the whole answer.
//! Every error leaves as a typed status via `AgentRpcError::code`.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_ai::agentic::{AgentProgress, Turn, TurnEvent, TurnStream};
use foundation_ai::types::{SessionRecord, UserModelContent};
use foundation_connectrpc::{ConnectResult, Ctx, Request, Response, Router};
use foundation_core::valtron::{
    Stream as ValtronStream, StreamIteratorExt, StreamParkingFutureStream,
};
use foundation_db::MemoryDocumentStore;
use futures::Stream;

use crate::error::AgentRpcError;
use crate::instance::{InstanceConfig, InstanceHandle, InstanceMeta, InstanceRegistry, TurnGuard};
use crate::proto::v1::agent_service::{register_agent_service, AgentService};
use crate::proto::v1::{
    self as pb, ChatRequest, ChatResponse, CreateInstanceRequest, DestroyInstanceRequest,
    DestroyInstanceResponse, HealthRequest, HealthResponse, InstanceInfo, ListInstancesRequest,
    ListInstancesResponse, ModelInfo,
};

type KvMemory = foundation_ai::agentic::KvMemoryStore<foundation_db::MemoryStorage>;
type SessionTurn = TurnStream<MemoryDocumentStore, KvMemory>;

/// The boxed event stream `ChatStream` returns (the generated trait's shape).
pub type TurnEventStream = Pin<Box<dyn Stream<Item = ConnectResult<pb::TurnEvent>> + Send>>;

/// `AgentService` over an instance registry.
pub struct AgentRpcService {
    registry: Arc<InstanceRegistry>,
}

impl std::fmt::Debug for AgentRpcService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRpcService")
            .field("registry", &self.registry)
            .finish()
    }
}

impl AgentRpcService {
    /// Serve `registry`'s models and instances.
    #[must_use]
    pub fn new(registry: Arc<InstanceRegistry>) -> Self {
        Self { registry }
    }

    /// The registry this service operates on.
    #[must_use]
    pub fn registry(&self) -> &Arc<InstanceRegistry> {
        &self.registry
    }
}

/// A `Router` with every `AgentService` procedure registered.
#[must_use]
pub fn agent_router(service: Arc<AgentRpcService>) -> Router {
    let mut router = Router::new();
    register_agent_service(&mut router, service);
    router
}

impl AgentService for AgentRpcService {
    fn health(
        &self,
        _ctx: Ctx,
        _request: Request<HealthRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<HealthResponse>>> + Send {
        let registry = Arc::clone(&self.registry);
        async move {
            let models = registry
                .models()
                .map(|m| ModelInfo {
                    name: m.name().to_string(),
                    ..Default::default()
                })
                .collect();
            let instances = u32::try_from(registry.list().len()).unwrap_or(u32::MAX);
            Ok(Response::new(HealthResponse {
                status: "serving".into(),
                models,
                instances,
                ..Default::default()
            }))
        }
    }

    fn create_instance(
        &self,
        _ctx: Ctx,
        request: Request<CreateInstanceRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<InstanceInfo>>> + Send {
        let registry = Arc::clone(&self.registry);
        async move {
            let req = request.msg;
            let cfg = InstanceConfig {
                model: req.model,
                system_prompt: req.system_prompt,
                max_tokens: req.max_tokens.map(|n| n as usize),
                temperature: req.temperature,
            };
            let handle = registry
                .create(&req.name, &cfg)
                .map_err(|e| rpc_error(e.current_context()))?;
            Ok(Response::new(instance_info(handle.meta())))
        }
    }

    fn list_instances(
        &self,
        _ctx: Ctx,
        _request: Request<ListInstancesRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<ListInstancesResponse>>> + Send
    {
        let registry = Arc::clone(&self.registry);
        async move {
            Ok(Response::new(ListInstancesResponse {
                instances: registry.list().into_iter().map(instance_info).collect(),
                ..Default::default()
            }))
        }
    }

    fn destroy_instance(
        &self,
        _ctx: Ctx,
        request: Request<DestroyInstanceRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<DestroyInstanceResponse>>> + Send
    {
        let registry = Arc::clone(&self.registry);
        async move {
            registry
                .destroy(&request.msg.name)
                .map_err(|e| rpc_error(e.current_context()))?;
            Ok(Response::new(DestroyInstanceResponse::default()))
        }
    }

    fn chat(
        &self,
        _ctx: Ctx,
        request: Request<ChatRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<ChatResponse>>> + Send {
        let registry = Arc::clone(&self.registry);
        async move {
            let RunningTurn { turn, guard, .. } = start_turn(&registry, request.msg)?;
            let mut stream = turn.into_future_stream();
            let mut records = Vec::new();
            while let Some(item) = futures::StreamExt::next(&mut stream).await {
                records.extend(records_of(item));
            }
            drop(guard);

            if let Some(error) = records.iter().find_map(|r| match r {
                SessionRecord::FailedAction { error, .. } => Some(error.to_string()),
                _ => None,
            }) {
                return Err(rpc_error(&AgentRpcError::Inference(error)));
            }
            let (message_count, usage) = records
                .iter()
                .find_map(|r| match r {
                    SessionRecord::Summary {
                        message_count,
                        usage,
                    } => Some((*message_count, usage_of(usage))),
                    _ => None,
                })
                .ok_or_else(|| {
                    rpc_error(&AgentRpcError::Inference(
                        "turn ended without a summary".into(),
                    ))
                })?;
            Ok(Response::new(ChatResponse {
                text: Turn::text_of_records(&records),
                usage: usage.into(),
                message_count,
                ..Default::default()
            }))
        }
    }

    fn chat_stream(
        &self,
        ctx: Ctx,
        request: Request<ChatRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<TurnEventStream>> + Send {
        let registry = Arc::clone(&self.registry);
        async move {
            let running = start_turn(&registry, request.msg)?;
            Ok(Box::pin(TurnEvents::new(running, ctx)) as TurnEventStream)
        }
    }
}

/// A started turn: its raw stream plus the instance claim it holds.
struct RunningTurn {
    instance: InstanceHandle,
    turn: SessionTurn,
    guard: TurnGuard,
}

fn start_turn(registry: &InstanceRegistry, req: ChatRequest) -> ConnectResult<RunningTurn> {
    let instance = registry
        .get(&req.instance)
        .ok_or_else(|| rpc_error(&AgentRpcError::InstanceNotFound(req.instance.clone())))?;
    let guard = instance
        .begin_turn()
        .map_err(|e| rpc_error(e.current_context()))?;
    let turn = instance
        .session()
        .run_turn_stream(req.prompt)
        .map_err(|e| rpc_error(&AgentRpcError::Inference(e.to_string())))?;
    Ok(RunningTurn {
        instance,
        turn,
        guard,
    })
}

/// `ChatStream`'s body: the turn's events, forwarded as they arrive.
///
/// Holds the instance's turn claim until the stream ends or is dropped; a
/// stream dropped before its terminal event (client gone) aborts the turn.
struct TurnEvents {
    stream: StreamParkingFutureStream<SessionTurn>,
    pending: VecDeque<pb::TurnEvent>,
    finished: bool,
    instance: InstanceHandle,
    ctx: Ctx,
    _guard: TurnGuard,
}

impl TurnEvents {
    fn new(running: RunningTurn, ctx: Ctx) -> Self {
        Self {
            stream: running.turn.into_future_stream(),
            pending: VecDeque::new(),
            finished: false,
            instance: running.instance,
            ctx,
            _guard: running.guard,
        }
    }
}

impl Stream for TurnEvents {
    type Item = ConnectResult<pb::TurnEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(event) = this.pending.pop_front() {
                return Poll::Ready(Some(Ok(event)));
            }
            if this.finished {
                return Poll::Ready(None);
            }
            if this.ctx.is_canceled() {
                this.instance.session().abort();
                this.finished = true;
                return Poll::Ready(Some(Err(foundation_connectrpc::ConnectError::new(
                    foundation_connectrpc::Code::Canceled,
                    "turn canceled",
                )
                .into())));
            }
            match Pin::new(&mut this.stream).poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    for event in TurnEvent::from_stream_item(item) {
                        let terminal = event.is_terminal();
                        if let Some(wire) = wire_event(event) {
                            this.pending.push_back(wire);
                        }
                        if terminal {
                            // Nothing follows a terminal event.
                            this.finished = true;
                            break;
                        }
                    }
                }
                Poll::Ready(None) => this.finished = true,
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Drop for TurnEvents {
    fn drop(&mut self) {
        if !self.finished {
            tracing::debug!(instance = %self.instance.name(), "chat stream dropped mid-turn; aborting");
            self.instance.session().abort();
        }
    }
}

/// The session records one raw stream item carries.
fn records_of(item: ValtronStream<SessionRecord, AgentProgress>) -> Vec<SessionRecord> {
    use foundation_core::valtron::StreamSpread;
    match item {
        ValtronStream::Next(record) => vec![record],
        ValtronStream::Spread(items) => items
            .into_iter()
            .filter_map(|s| match s {
                StreamSpread::Done(record) => Some(record),
                StreamSpread::Pending(_) => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A turn event on the wire; progress hints are not part of the contract.
fn wire_event(event: TurnEvent) -> Option<pb::TurnEvent> {
    use pb::__buffa::oneof::turn_event::Event;
    let event = match event {
        TurnEvent::Text(text) => Event::Text(text),
        TurnEvent::Thinking(text) => Event::Thinking(text),
        TurnEvent::ToolCall {
            id,
            name,
            arguments,
        } => Event::from(pb::ToolCall {
            id,
            name,
            arguments_json: arguments
                .map(|a| serde_json::Value::Object(a).to_string())
                .unwrap_or_default(),
            ..Default::default()
        }),
        TurnEvent::ToolResult {
            tool_call_id,
            name,
            content,
        } => Event::from(pb::ToolResult {
            tool_call_id,
            name,
            content: match content {
                UserModelContent::Text(t) => t.content,
                UserModelContent::Image(_) => "[image]".to_string(),
            },
            ..Default::default()
        }),
        TurnEvent::Retract { reason } => Event::from(pb::Retract {
            reason,
            ..Default::default()
        }),
        TurnEvent::Failed(error) => Event::from(pb::TurnFailed {
            message: error.to_string(),
            ..Default::default()
        }),
        TurnEvent::Done(summary) => Event::from(pb::TurnDone {
            message_count: summary.message_count,
            usage: usage_of(&summary.usage).into(),
            ..Default::default()
        }),
        // `TurnEvent` is non-exhaustive; progress (and anything newer) stays
        // server-side.
        _ => return None,
    };
    Some(pb::TurnEvent {
        event: Some(event),
        ..Default::default()
    })
}

fn usage_of(snapshot: &foundation_ai::agentic::TokenSnapshot) -> pb::Usage {
    pb::Usage {
        input_tokens: snapshot.input,
        output_tokens: snapshot.output,
        total_tokens: snapshot.total,
        ..Default::default()
    }
}

fn instance_info(meta: InstanceMeta) -> InstanceInfo {
    InstanceInfo {
        name: meta.name,
        model: meta.model,
        created_unix_ms: meta.created_unix_ms,
        turns: meta.turns,
        ..Default::default()
    }
}

/// The wire error for `err`, logged at the boundary.
fn rpc_error(
    err: &AgentRpcError,
) -> foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError> {
    tracing::debug!(code = err.code().as_str(), error = %err, "rpc error");
    err.clone().into()
}
