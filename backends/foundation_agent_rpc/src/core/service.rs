//! The `AgentService` handlers over the core traits.
//!
//! WHY: one handler set serves every host. The handlers see a model catalog,
//! an instance registry and the checked caller, and nothing else.
//!
//! WHAT: [`AgentRpcService`] implements the generated `AgentService` trait,
//! [`agent_router`] registers it on a connectrpc `Router`, and
//! `GetCapabilities` reports the model kinds and limits of the host.
//!
//! HOW: the auth interceptor has already checked the call and put the
//! [`Principal`] into the call context. A handler turns it into a [`Caller`]
//! and calls the registry. A streamed turn goes out event by event.

use std::sync::Arc;

use buffa::Message;
use foundation_auth::client_auth::Principal;
use foundation_connectrpc::{
    get_auth_artifact, get_auth_info, ConnectError, ConnectResult, ErrorDetail, Request, Response,
    Router, RpcCtx,
};
use foundation_errstacks::ErrorTrace;
use futures::StreamExt;

use crate::core::config::{RegistryConfig, TurnLimits};
use crate::core::model::{Device, ModelCapabilities, ModelCatalog};
use crate::core::registry::{Caller, InstanceInfo, InstanceRegistry, ScopePolicy};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::agent_service::{register_agent_service, AgentService};
use crate::proto::v1::{
    self as pb, ChatRequest, ChatResponse, CreateInstanceRequest, DestroyInstanceRequest,
    DestroyInstanceResponse, GetCapabilitiesRequest, GetCapabilitiesResponse, HealthRequest,
    HealthResponse, InstanceInfo as PbInstanceInfo, InstanceLimits, ListInstancesRequest,
    ListInstancesResponse, ModelDescriptor, ModelInfo, ModelKindSupport, ModelSelectionError,
};

/// The wire event stream of one turn (the generated trait's shape).
type WireTurnEventStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = ConnectResult<pb::TurnEvent>> + Send>>;

/// `AgentService` over the core traits.
pub struct AgentRpcService {
    registry: Arc<dyn InstanceRegistry>,
    catalog: Arc<dyn ModelCatalog>,
    config: RegistryConfig,
    turns: TurnLimits,
    protocols: Vec<String>,
    auth_modes: Vec<String>,
    host_kind: String,
}

impl std::fmt::Debug for AgentRpcService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRpcService")
            .field("host_kind", &self.host_kind)
            .field("config", &self.config)
            .field("turns", &self.turns)
            .finish_non_exhaustive()
    }
}

impl AgentRpcService {
    /// Serve `registry`'s instances and `catalog`'s models.
    #[must_use]
    pub fn new(
        registry: Arc<dyn InstanceRegistry>,
        catalog: Arc<dyn ModelCatalog>,
        config: RegistryConfig,
        turns: TurnLimits,
        protocols: Vec<String>,
        auth_modes: Vec<String>,
        host_kind: impl Into<String>,
    ) -> Self {
        Self {
            registry,
            catalog,
            config,
            turns,
            protocols,
            auth_modes,
            host_kind: host_kind.into(),
        }
    }

    /// The registry this service operates on.
    #[must_use]
    pub fn registry(&self) -> &Arc<dyn InstanceRegistry> {
        &self.registry
    }

    /// The catalog that this service reports and serves.
    #[must_use]
    pub fn catalog(&self) -> &Arc<dyn ModelCatalog> {
        &self.catalog
    }
}

/// The caller of one checked call.
fn caller(ctx: &RpcCtx, policy: ScopePolicy) -> AgentRpcResult<Caller> {
    let principal = get_auth_artifact::<Principal>(ctx)
        .cloned()
        .ok_or_else(|| AgentRpcError::Auth("the auth provider set no principal".to_string()))?;
    Ok(policy.caller(principal))
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
        ctx: RpcCtx,
        _request: Request<HealthRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<HealthResponse>>> + Send {
        let registry = Arc::clone(&self.registry);
        let capabilities = self.catalog.capabilities();
        let policy = self.config.scope_policy;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            let instances = registry
                .count(&caller)
                .await
                .map_err(|error| trace_rpc_error(&error))?;
            let models = capabilities
                .models
                .iter()
                .map(|model| ModelInfo {
                    name: model.name.clone(),
                    kind: model.kind.to_enum_value(),
                    device: device_field(model.device),
                    ..Default::default()
                })
                .collect();
            Ok(Response::new(HealthResponse {
                status: "serving".into(),
                models,
                instances: u32::try_from(instances).unwrap_or(u32::MAX),
                ..Default::default()
            }))
        }
    }

    fn get_capabilities(
        &self,
        ctx: RpcCtx,
        _request: Request<GetCapabilitiesRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<GetCapabilitiesResponse>>> + Send
    {
        let capabilities = self.catalog.capabilities();
        let protocols = self.protocols.clone();
        let auth_modes = self.auth_modes.clone();
        let host_kind = self.host_kind.clone();
        let config = self.config.clone();
        let turns = self.turns.clone();
        async move {
            // The auth interceptor already checked the call. This call also
            // proves that the caller is a known principal, as the other calls
            // require.
            if get_auth_info(&ctx).is_none() {
                return Err(rpc_error(&AgentRpcError::Auth(
                    "no authenticated caller".to_string(),
                )));
            }
            Ok(Response::new(GetCapabilitiesResponse {
                model_kinds: capabilities
                    .kinds
                    .iter()
                    .map(|support| ModelKindSupport {
                        kind: support.kind.to_enum_value(),
                        devices: support
                            .devices
                            .iter()
                            .map(|device| device.to_wire())
                            .collect(),
                        ..Default::default()
                    })
                    .collect(),
                models: capabilities
                    .models
                    .iter()
                    .map(|model| ModelDescriptor {
                        name: model.name.clone(),
                        kind: model.kind.to_enum_value(),
                        device: device_field(model.device),
                        state: buffa::EnumValue::Known(model.state.to_wire()),
                        ..Default::default()
                    })
                    .collect(),
                protocols,
                auth_modes,
                limits: Some(InstanceLimits {
                    max_instances: u32::try_from(config.max_instances).unwrap_or(u32::MAX),
                    max_name_len: u32::try_from(config.max_name_len).unwrap_or(u32::MAX),
                    max_prompt_bytes: u32::try_from(turns.max_prompt_bytes).unwrap_or(u32::MAX),
                    ..Default::default()
                })
                .into(),
                host_kind,
                ..Default::default()
            }))
        }
    }

    fn create_instance(
        &self,
        ctx: RpcCtx,
        request: Request<CreateInstanceRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<PbInstanceInfo>>> + Send {
        let registry = Arc::clone(&self.registry);
        let capabilities = self.catalog.capabilities();
        let policy = self.config.scope_policy;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            let request = request.msg;
            let spec = crate::core::registry::InstanceSpec::from_wire(&request)
                .map_err(|error| create_error(error.current_context(), &request, &capabilities))?;
            let info = registry
                .create(&caller, spec)
                .await
                .map_err(|error| create_error(error.current_context(), &request, &capabilities))?;
            Ok(Response::new(instance_info(info)))
        }
    }

    fn list_instances(
        &self,
        ctx: RpcCtx,
        request: Request<ListInstancesRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<ListInstancesResponse>>> + Send
    {
        let registry = Arc::clone(&self.registry);
        let policy = self.config.scope_policy;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            let request = request.msg;
            let page = registry
                .list(
                    &caller,
                    request.page_size,
                    Some(request.page_token.as_str()),
                )
                .await
                .map_err(|error| trace_rpc_error(&error))?;
            Ok(Response::new(ListInstancesResponse {
                instances: page.items.into_iter().map(instance_info).collect(),
                next_page_token: page.next_token.unwrap_or_default(),
                ..Default::default()
            }))
        }
    }

    fn destroy_instance(
        &self,
        ctx: RpcCtx,
        request: Request<DestroyInstanceRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<DestroyInstanceResponse>>> + Send
    {
        let registry = Arc::clone(&self.registry);
        let policy = self.config.scope_policy;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            registry
                .destroy(&caller, &request.msg.name)
                .await
                .map_err(|error| trace_rpc_error(&error))?;
            Ok(Response::new(DestroyInstanceResponse::default()))
        }
    }

    fn chat(
        &self,
        ctx: RpcCtx,
        request: Request<ChatRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<ChatResponse>>> + Send {
        let registry = Arc::clone(&self.registry);
        let policy = self.config.scope_policy;
        let limit = self.turns.max_unary_reply_bytes;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            let request = request.msg;
            let mut stream = registry
                .start_turn(&caller, &request.instance, request.prompt)
                .await
                .map_err(|error| trace_rpc_error(&error))?;
            let mut text = String::new();
            let mut usage = pb::Usage::default();
            let mut message_count = 0_u64;
            let mut finished = false;
            while let Some(event) = stream.next().await {
                let event = event.map_err(|error| trace_rpc_error(&error))?;
                match event.event {
                    Some(pb::__buffa::oneof::turn_event::Event::Text(chunk)) => {
                        text.push_str(&chunk);
                        if text.len() > limit {
                            return Err(rpc_error(&AgentRpcError::ReplyTooLarge(limit)));
                        }
                    }
                    Some(pb::__buffa::oneof::turn_event::Event::Failed(failed)) => {
                        return Err(rpc_error(&AgentRpcError::Inference(failed.message)));
                    }
                    Some(pb::__buffa::oneof::turn_event::Event::Done(done)) => {
                        usage = done.usage.into_option().unwrap_or_default();
                        message_count = done.message_count;
                        finished = true;
                        break;
                    }
                    _ => {}
                }
            }
            if !finished {
                return Err(rpc_error(&AgentRpcError::Inference(
                    "the turn ended without a result".to_string(),
                )));
            }
            Ok(Response::new(ChatResponse {
                text,
                usage: usage.into(),
                message_count,
                ..Default::default()
            }))
        }
    }

    fn chat_stream(
        &self,
        ctx: RpcCtx,
        request: Request<ChatRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<WireTurnEventStream>> + Send {
        let registry = Arc::clone(&self.registry);
        let policy = self.config.scope_policy;
        async move {
            let caller = caller(&ctx, policy).map_err(|error| trace_rpc_error(&error))?;
            let request = request.msg;
            let stream = registry
                .start_turn(&caller, &request.instance, request.prompt)
                .await
                .map_err(|error| trace_rpc_error(&error))?;
            Ok(Box::pin(ConnectTurnEvents { inner: stream }) as WireTurnEventStream)
        }
    }
}

/// The wire stream of one turn.
struct ConnectTurnEvents {
    inner: crate::core::registry::TurnEventStream,
}

impl futures::Stream for ConnectTurnEvents {
    type Item = ConnectResult<pb::TurnEvent>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let stream = self.get_mut().inner.as_mut();
        match futures::Stream::poll_next(stream, cx) {
            std::task::Poll::Ready(Some(Ok(event))) => std::task::Poll::Ready(Some(Ok(event))),
            std::task::Poll::Ready(Some(Err(error))) => {
                std::task::Poll::Ready(Some(Err(trace_rpc_error(&error))))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

fn device_field(device: Device) -> buffa::MessageField<pb::Device, buffa::Inline<pb::Device>> {
    device.to_wire().into()
}

fn instance_info(info: InstanceInfo) -> PbInstanceInfo {
    PbInstanceInfo {
        name: info.name,
        model: info.model,
        created_unix_ms: info.created_unix_ms,
        turns: info.turns,
        kind: info.kind.to_enum_value(),
        device: device_field(info.device),
        ..Default::default()
    }
}

/// The wire error for `err`, logged at the boundary.
fn rpc_error(err: &AgentRpcError) -> ErrorTrace<ConnectError> {
    tracing::debug!(code = err.code().as_str(), error = %err, "rpc error");
    err.clone().into()
}

/// The wire error of a core result.
fn trace_rpc_error(error: &ErrorTrace<AgentRpcError>) -> ErrorTrace<ConnectError> {
    rpc_error(error.current_context())
}

/// The wire error of a failed model selection, with the detail message
/// (proto.md section 5).
fn selection_error(
    error: &AgentRpcError,
    request: &CreateInstanceRequest,
    capabilities: &ModelCapabilities,
) -> ErrorTrace<ConnectError> {
    tracing::debug!(code = error.code().as_str(), error = %error, "model selection failed");
    let requested_device: buffa::MessageField<pb::Device, buffa::Inline<pb::Device>> =
        match request.device.as_option() {
            Some(device) => Some(device.clone()).into(),
            None => buffa::MessageField::default(),
        };
    let detail = ModelSelectionError {
        requested_model: request.model.clone(),
        requested_kind: request.kind,
        requested_device,
        supported: capabilities
            .kinds
            .iter()
            .map(|support| ModelKindSupport {
                kind: support.kind.to_enum_value(),
                devices: support
                    .devices
                    .iter()
                    .map(|device| device.to_wire())
                    .collect(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut wire = ConnectError::from(error.clone());
    wire.add_detail(ErrorDetail::from_raw(
        ModelSelectionError::TYPE_URL,
        detail.encode_to_vec(),
    ));
    ErrorTrace::new(wire)
}

/// The wire error of a failed `CreateInstance`.
///
/// A selection failure carries the `ModelSelectionError` detail; every other
/// failure keeps its own code and message.
fn create_error(
    error: &AgentRpcError,
    request: &CreateInstanceRequest,
    capabilities: &ModelCapabilities,
) -> ErrorTrace<ConnectError> {
    if matches!(error, AgentRpcError::ModelSelection { .. }) {
        selection_error(error, request, capabilities)
    } else {
        rpc_error(error)
    }
}
