//! T16 — the `AgentService` handlers over fakes.
//!
//! The registry and the catalog are fakes: the handlers must depend only on
//! their traits.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use foundation_agent_rpc::core::config::{RegistryConfig, TurnLimits};
use foundation_agent_rpc::core::model::ModelRequest;
use foundation_agent_rpc::core::model::{
    AgentRpcFuture, Device, KindSupport, LoadState, ModelCapabilities, ModelCatalog,
    ModelDescriptor, ModelHandle, ModelKind,
};
use foundation_agent_rpc::core::registry::{
    decode_page_token, encode_page_token, Caller, InstanceInfo, InstanceRegistry, InstanceSpec,
    Page, TurnEventStream,
};
use foundation_agent_rpc::core::service::AgentRpcService;
use foundation_agent_rpc::proto::v1::agent_service::AgentService;
use foundation_agent_rpc::proto::v1::{
    self as pb, ChatRequest, CreateInstanceRequest, DestroyInstanceRequest, GetCapabilitiesRequest,
    HealthRequest, ListInstancesRequest, ModelKind as WireModelKind,
};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_auth::client_auth::{AuthInfo, AuthMethod, Principal};
use foundation_auth::AuthContext;
use foundation_connectrpc::{Code, Request, RpcCtx};
use foundation_core::valtron::valtron_test;
use futures::StreamExt;

fn context() -> RpcCtx {
    let info = AuthInfo::new(AuthContext::new("/".into(), None, None)).with_artifact(Principal {
        subject: "alice".into(),
        method: AuthMethod::Insecure,
        roles: vec!["agent-rpc:client".into()],
    });
    RpcCtx::background().with_extension(info)
}

fn info(name: &str) -> InstanceInfo {
    InstanceInfo {
        name: name.into(),
        model: "mock".into(),
        kind: ModelKind::Api,
        device: Device::Remote,
        created_unix_ms: 1,
        turns: 0,
    }
}

#[derive(Default)]
struct FakeRegistry {
    instances: Mutex<BTreeMap<String, InstanceInfo>>,
    created: Mutex<Vec<InstanceSpec>>,
    page_size: Mutex<u32>,
}

impl InstanceRegistry for FakeRegistry {
    fn create<'a>(
        &'a self,
        _caller: &'a Caller,
        spec: InstanceSpec,
    ) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            self.created.lock().unwrap().push(spec.clone());
            let mut instances = self.instances.lock().unwrap();
            if instances.contains_key(&spec.name) {
                return Err(AgentRpcError::InstanceExists(spec.name).into());
            }
            let value = info(&spec.name);
            instances.insert(spec.name, value.clone());
            Ok(value)
        })
    }

    fn get<'a>(&'a self, _caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            self.instances
                .lock()
                .unwrap()
                .get(name)
                .cloned()
                .ok_or_else(|| AgentRpcError::InstanceNotFound(name.to_string()).into())
        })
    }

    fn list<'a>(
        &'a self,
        _caller: &'a Caller,
        page_size: u32,
        page_token: Option<&'a str>,
    ) -> AgentRpcFuture<'a, Page<InstanceInfo>> {
        Box::pin(async move {
            *self.page_size.lock().unwrap() = page_size;
            let after = decode_page_token(page_token.unwrap_or(""))?;
            let instances = self.instances.lock().unwrap();
            let names: Vec<&String> = instances
                .keys()
                .filter(|name| after.as_ref().is_none_or(|after| *name > after))
                .collect();
            let size = if page_size == 0 {
                30
            } else {
                usize::try_from(page_size).unwrap_or(usize::MAX)
            };
            let take = names.len().min(size);
            let items = names[..take]
                .iter()
                .map(|name| instances[*name].clone())
                .collect();
            let next_token = (names.len() > take).then(|| encode_page_token(names[take - 1]));
            Ok(Page { items, next_token })
        })
    }

    fn destroy<'a>(&'a self, _caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, ()> {
        Box::pin(async move {
            self.instances
                .lock()
                .unwrap()
                .remove(name)
                .map(|_| ())
                .ok_or_else(|| AgentRpcError::InstanceNotFound(name.to_string()).into())
        })
    }

    fn start_turn<'a>(
        &'a self,
        _caller: &'a Caller,
        name: &'a str,
        _prompt: String,
    ) -> AgentRpcFuture<'a, TurnEventStream> {
        Box::pin(async move {
            if !self.instances.lock().unwrap().contains_key(name) {
                return Err(AgentRpcError::InstanceNotFound(name.to_string()).into());
            }
            let text = pb::TurnEvent {
                event: Some(pb::__buffa::oneof::turn_event::Event::Text(
                    "hello".to_string(),
                )),
                ..Default::default()
            };
            let done = pb::TurnEvent {
                event: Some(pb::__buffa::oneof::turn_event::Event::from(pb::TurnDone {
                    message_count: 2,
                    usage: Some(pb::Usage {
                        input_tokens: 1,
                        output_tokens: 2,
                        total_tokens: 3,
                        ..Default::default()
                    })
                    .into(),
                    ..Default::default()
                })),
                ..Default::default()
            };
            let stream: TurnEventStream = Box::pin(futures::stream::iter(vec![Ok(text), Ok(done)]));
            Ok(stream)
        })
    }

    fn count<'a>(&'a self, _caller: &'a Caller) -> AgentRpcFuture<'a, u64> {
        Box::pin(async move {
            Ok(u64::try_from(self.instances.lock().unwrap().len()).unwrap_or(u64::MAX))
        })
    }
}

struct FakeCatalog;

impl ModelCatalog for FakeCatalog {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: vec![
                KindSupport {
                    kind: ModelKind::Api,
                    devices: vec![Device::Remote],
                },
                KindSupport {
                    kind: ModelKind::Candle,
                    devices: vec![Device::Cpu],
                },
            ],
            models: vec![
                ModelDescriptor {
                    name: "gpt".into(),
                    kind: ModelKind::Api,
                    device: Device::Remote,
                    state: LoadState::Remote,
                },
                ModelDescriptor {
                    name: "tiny".into(),
                    kind: ModelKind::Candle,
                    device: Device::Cpu,
                    state: LoadState::Loaded,
                },
            ],
        }
    }

    fn resolve<'a>(&'a self, _request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            let mut mock = MockModelProvider::new();
            mock.on_any(vec![mock_text("hello")]);
            Ok(ModelHandle::from_router(
                "gpt",
                ModelKind::Api,
                Device::Remote,
                "gpt",
                mock.into_router(),
            ))
        })
    }
}

fn service(turns: TurnLimits) -> Arc<AgentRpcService> {
    Arc::new(AgentRpcService::new(
        Arc::new(FakeRegistry::default()),
        Arc::new(FakeCatalog),
        RegistryConfig::default(),
        turns,
        vec!["connect".into(), "grpc".into()],
        vec!["insecure".into()],
        "native",
    ))
}

fn create(name: &str) -> CreateInstanceRequest {
    CreateInstanceRequest {
        name: name.into(),
        model: "gpt".into(),
        ..Default::default()
    }
}

#[valtron_test]
async fn get_capabilities_reports_the_catalog_and_limits() {
    let service = service(TurnLimits::default());
    let response = service
        .get_capabilities(context(), Request::new(GetCapabilitiesRequest::default()))
        .await
        .expect("capabilities")
        .msg;
    assert_eq!(response.model_kinds.len(), 2);
    assert_eq!(
        response.model_kinds[0].kind,
        buffa::EnumValue::Known(WireModelKind::MODEL_KIND_API)
    );
    let models: Vec<&str> = response.models.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(models, vec!["gpt", "tiny"]);
    assert_eq!(response.protocols, vec!["connect", "grpc"]);
    assert_eq!(response.auth_modes, vec!["insecure"]);
    assert_eq!(response.host_kind, "native");
    let limits = response.limits.as_option().expect("limits");
    assert_eq!(limits.max_instances, 1024);
    assert_eq!(limits.max_name_len, 63);
    assert_eq!(limits.max_prompt_bytes, 1024 * 1024);
}

#[valtron_test]
async fn create_instance_reads_the_wire_request() {
    let service = service(TurnLimits::default());
    let response = service
        .create_instance(context(), Request::new(create("alpha")))
        .await
        .expect("create")
        .msg;
    assert_eq!(response.name, "alpha");
}

#[valtron_test]
async fn an_unknown_enum_is_invalid_argument_with_the_detail() {
    let service = service(TurnLimits::default());
    let error = service
        .create_instance(
            context(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                kind: buffa::EnumValue::Unknown(42),
                ..Default::default()
            }),
        )
        .await
        .expect_err("unknown kind");
    assert_eq!(error.current_context().code(), Code::InvalidArgument);
    assert!(
        !error.current_context().details().is_empty(),
        "the ModelSelectionError detail"
    );
}

#[valtron_test]
async fn list_instances_pages() {
    let service = service(TurnLimits::default());
    for name in ["alpha", "bravo", "charlie"] {
        service
            .create_instance(context(), Request::new(create(name)))
            .await
            .expect("create");
    }
    let first = service
        .list_instances(
            context(),
            Request::new(ListInstancesRequest {
                page_size: 2,
                ..Default::default()
            }),
        )
        .await
        .expect("first")
        .msg;
    assert_eq!(first.instances.len(), 2);
    assert_eq!(first.next_page_token, encode_page_token("bravo"));
    let second = service
        .list_instances(
            context(),
            Request::new(ListInstancesRequest {
                page_size: 2,
                page_token: first.next_page_token,
                ..Default::default()
            }),
        )
        .await
        .expect("second")
        .msg;
    assert_eq!(second.instances.len(), 1);
    assert_eq!(second.next_page_token, "");
}

#[valtron_test]
async fn chat_collects_the_text_and_the_summary() {
    let service = service(TurnLimits::default());
    service
        .create_instance(context(), Request::new(create("alpha")))
        .await
        .expect("create");
    let response = service
        .chat(
            context(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("chat")
        .msg;
    assert_eq!(response.text, "hello");
    assert_eq!(response.message_count, 2);
    let usage = response.usage.as_option().expect("usage");
    assert_eq!(usage.total_tokens, 3);
}

#[valtron_test]
async fn a_long_unary_reply_is_resource_exhausted() {
    let service = service(TurnLimits::default().with_max_unary_reply_bytes(2));
    service
        .create_instance(context(), Request::new(create("alpha")))
        .await
        .expect("create");
    let error = service
        .chat(
            context(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect_err("long reply");
    assert_eq!(error.current_context().code(), Code::ResourceExhausted);
}

#[valtron_test]
async fn chat_stream_forwards_each_event() {
    let service = service(TurnLimits::default());
    service
        .create_instance(context(), Request::new(create("alpha")))
        .await
        .expect("create");
    let mut stream = service
        .chat_stream(
            context(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("stream");
    let mut events = 0;
    while let Some(event) = stream.next().await {
        event.expect("event");
        events += 1;
    }
    assert_eq!(events, 2);
}

#[valtron_test]
async fn health_reports_the_instance_count() {
    let service = service(TurnLimits::default());
    service
        .create_instance(context(), Request::new(create("alpha")))
        .await
        .expect("create");
    let health = service
        .health(context(), Request::new(HealthRequest::default()))
        .await
        .expect("health")
        .msg;
    assert_eq!(health.status, "serving");
    assert_eq!(health.instances, 1);
}

#[valtron_test]
async fn destroy_an_unknown_instance_is_not_found() {
    let service = service(TurnLimits::default());
    let error = service
        .destroy_instance(
            context(),
            Request::new(DestroyInstanceRequest {
                name: "missing".into(),
                ..Default::default()
            }),
        )
        .await
        .expect_err("missing");
    assert_eq!(error.current_context().code(), Code::NotFound);
}

#[valtron_test]
async fn a_call_without_a_principal_is_unauthenticated() {
    let service = service(TurnLimits::default());
    let error = service
        .create_instance(RpcCtx::background(), Request::new(create("alpha")))
        .await
        .expect_err("no principal");
    assert_eq!(error.current_context().code(), Code::Unauthenticated);
}

#[valtron_test]
async fn the_router_registers_every_procedure() {
    let service = service(TurnLimits::default());
    let router = foundation_agent_rpc::core::service::agent_router(service);
    assert_eq!(router.len(), 7);
}
