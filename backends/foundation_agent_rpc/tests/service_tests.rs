//! T5 — the generated `AgentService`, implemented over the registry and wired
//! into a connectrpc `Router`.

use std::sync::Arc;

use foundation_agent_rpc::proto::v1::agent_service::AgentService;
use foundation_agent_rpc::proto::v1::{
    ChatRequest, CreateInstanceRequest, DestroyInstanceRequest, HealthRequest, ListInstancesRequest,
};
use foundation_agent_rpc::{
    agent_router, AgentRpcService, InstanceRegistry, ModelHandle, RegistryConfig,
};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{Code, Ctx, Request};
use foundation_core::valtron::valtron_test;

fn service(reply: &str) -> Arc<AgentRpcService> {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text(reply)]);
    let model = ModelHandle::from_router("mock", "mock", mock.into_router());
    let registry = InstanceRegistry::new(vec![model], RegistryConfig::default()).expect("registry");
    Arc::new(AgentRpcService::new(Arc::new(registry)))
}

fn create(name: &str) -> CreateInstanceRequest {
    CreateInstanceRequest {
        name: name.into(),
        model: "mock".into(),
        ..Default::default()
    }
}

#[test]
fn every_agent_service_procedure_is_registered() {
    let router = agent_router(service("hi"));
    assert_eq!(
        router.len(),
        6,
        "Health, Create/List/DestroyInstance, Chat, ChatStream"
    );
}

#[valtron_test]
async fn health_reports_models_and_instances() {
    let svc = service("hi");
    svc.create_instance(Ctx::background(), Request::new(create("a")))
        .await
        .expect("create");
    let health = svc
        .health(Ctx::background(), Request::new(HealthRequest::default()))
        .await
        .expect("health")
        .msg;
    assert_eq!(health.status, "serving");
    assert_eq!(health.instances, 1);
    let models: Vec<String> = health.models.iter().map(|m| m.name.clone()).collect();
    assert_eq!(models, vec!["mock".to_string()]);
}

#[valtron_test]
async fn instance_lifecycle_through_the_service() {
    let svc = service("hi");
    let info = svc
        .create_instance(Ctx::background(), Request::new(create("alpha")))
        .await
        .expect("create")
        .msg;
    assert_eq!((info.name.as_str(), info.model.as_str()), ("alpha", "mock"));

    let listed = svc
        .list_instances(
            Ctx::background(),
            Request::new(ListInstancesRequest::default()),
        )
        .await
        .expect("list")
        .msg;
    assert_eq!(listed.instances.len(), 1);

    svc.destroy_instance(
        Ctx::background(),
        Request::new(DestroyInstanceRequest {
            name: "alpha".into(),
            ..Default::default()
        }),
    )
    .await
    .expect("destroy");

    let err = svc
        .destroy_instance(
            Ctx::background(),
            Request::new(DestroyInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect_err("already gone");
    assert_eq!(err.current_context().code(), Code::NotFound);
}

#[valtron_test]
async fn chat_runs_a_turn_on_the_instance() {
    let svc = service("hello from the model");
    svc.create_instance(Ctx::background(), Request::new(create("alpha")))
        .await
        .expect("create");
    let reply = svc
        .chat(
            Ctx::background(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("chat")
        .msg;
    assert_eq!(reply.text, "hello from the model");
    assert!(reply.message_count > 0);
}
