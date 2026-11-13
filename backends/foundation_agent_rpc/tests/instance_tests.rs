//! T4 — `InstanceRegistry`: named, lightweight instances over shared models.

// The `#[valtron_test]` `Send` check descends the deeply nested async chain of
// the candle model load and the agent session build, which overflows the
// default recursion limit (128). Raise it for this test crate (the depth is in
// the external candle dependency).
#![recursion_limit = "256"]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_agent_rpc::{
    AgentRpcError, InstanceConfig, InstanceRegistry, ModelHandle, RegistryConfig,
};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

fn mock_model(name: &str, reply: &str) -> ModelHandle {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text(reply)]);
    ModelHandle::from_router(name, name, mock.into_router())
}

fn registry() -> InstanceRegistry {
    InstanceRegistry::new(
        vec![mock_model("mock", "hello")],
        RegistryConfig::default(),
        Ctx::host(),
    )
    .expect("registry builds")
}

#[valtron_test]
async fn create_list_destroy_round_trip() {
    let reg = registry();
    reg.create("alpha", &InstanceConfig::for_model("mock"))
        .await
        .expect("create alpha");
    reg.create("beta", &InstanceConfig::for_model("mock"))
        .await
        .expect("create beta");

    let names: Vec<String> = reg.list().into_iter().map(|m| m.name).collect();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    assert!(reg.get("alpha").is_some());

    reg.destroy("alpha").await.expect("destroy alpha");
    assert!(reg.get("alpha").is_none());
    let names: Vec<String> = reg.list().into_iter().map(|m| m.name).collect();
    assert_eq!(names, vec!["beta".to_string()]);
}

#[valtron_test]
async fn an_instance_answers_through_its_model() {
    let reg = registry();
    let inst = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
        .await
        .expect("create");
    let answer = inst.session().ask("hi").await.expect("turn starts");
    assert_eq!(answer.text(), "hello");
}

#[valtron_test]
async fn duplicate_unknown_and_missing_names_are_typed_errors() {
    let reg = registry();
    reg.create("alpha", &InstanceConfig::for_model("mock"))
        .await
        .expect("create");

    let dup = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
        .await
        .expect_err("duplicate");
    assert!(matches!(dup.current_context(), AgentRpcError::InstanceExists(n) if n == "alpha"));

    let unknown = reg
        .create("gamma", &InstanceConfig::for_model("nope"))
        .await
        .expect_err("unknown model");
    assert!(matches!(
        unknown.current_context(),
        AgentRpcError::ModelNotFound(_)
    ));

    let missing = reg.destroy("zeta").await.expect_err("missing");
    assert!(matches!(
        missing.current_context(),
        AgentRpcError::InstanceNotFound(_)
    ));
}

#[valtron_test]
async fn instance_names_are_validated() {
    let reg = registry();
    for bad in [
        "",
        "../etc",
        "a/b",
        "has space",
        "ctl\u{7}",
        "-leading",
        &"x".repeat(64),
    ] {
        let err = reg
            .create(bad, &InstanceConfig::for_model("mock"))
            .await
            .expect_err(bad);
        assert!(
            matches!(err.current_context(), AgentRpcError::InvalidName(_)),
            "{bad:?} -> {err:?}"
        );
    }
    reg.create("ok-name_1.v2", &InstanceConfig::for_model("mock"))
        .await
        .expect("valid name");
}

#[valtron_test]
async fn the_instance_limit_is_configuration() {
    let reg = InstanceRegistry::new(
        vec![mock_model("mock", "hi")],
        RegistryConfig::default().with_max_instances(2),
        Ctx::host(),
    )
    .expect("registry");
    reg.create("a", &InstanceConfig::for_model("mock"))
        .await
        .expect("a");
    reg.create("b", &InstanceConfig::for_model("mock"))
        .await
        .expect("b");
    let err = reg
        .create("c", &InstanceConfig::for_model("mock"))
        .await
        .expect_err("limit");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceLimit(2)
    ));
}

#[valtron_test]
async fn creating_instances_never_reloads_the_model() {
    let loads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&loads);
    let model = ModelHandle::load_with("tiny", move || {
        counter.fetch_add(1, Ordering::SeqCst);
        // Load the concrete candle model through the public loader path.
        load_candle("tiny", &fixture_dir("tiny-random-LlamaForCausalLM"))
    })
    .expect("loads");

    let reg = InstanceRegistry::new(vec![model.clone()], RegistryConfig::default(), Ctx::host())
        .expect("registry");
    for i in 0..10 {
        let inst = reg
            .create(&format!("inst-{i}"), &InstanceConfig::for_model("tiny"))
            .await
            .expect("create");
        assert!(
            inst.model().same_model(&model),
            "instance shares the one model"
        );
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1, "ten instances, one load");
}

#[valtron_test]
async fn a_second_concurrent_turn_on_one_instance_is_rejected() {
    let reg = registry();
    let inst = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
        .await
        .expect("create");
    let turn = inst.begin_turn().expect("first turn");
    let err = inst.begin_turn().expect_err("second turn while busy");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceBusy(_)
    ));
    drop(turn);
    inst.begin_turn()
        .expect("free again after the first turn ends");
}

fn load_candle(
    name: &str,
    dir: &std::path::Path,
) -> foundation_agent_rpc::AgentRpcResult<foundation_ai::backends::candle::CandleModels> {
    use foundation_ai::backends::candle::CandleBackend;
    use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
    CandleBackend::cpu(Ctx::host())
        .get_model_by_spec(ModelSpec {
            name: name.to_string(),
            id: ModelId::Name(name.to_string(), None),
            devices: None,
            model_location: Some(dir.to_path_buf()),
            lora_location: None,
        })
        .map_err(|e| AgentRpcError::ModelLoad(e.to_string()).into())
}
