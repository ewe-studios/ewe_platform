//! T4 — `InstanceRegistry`: named, lightweight instances over shared models.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_agent_rpc::{
    AgentRpcError, InstanceConfig, InstanceRegistry, ModelHandle, RegistryConfig,
};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
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
    InstanceRegistry::new(vec![mock_model("mock", "hello")], RegistryConfig::default())
        .expect("registry builds")
}

#[valtron_test]
fn create_list_destroy_round_trip() {
    let reg = registry();
    reg.create("alpha", &InstanceConfig::for_model("mock"))
        .expect("create alpha");
    reg.create("beta", &InstanceConfig::for_model("mock"))
        .expect("create beta");

    let names: Vec<String> = reg.list().into_iter().map(|m| m.name).collect();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    assert!(reg.get("alpha").is_some());

    reg.destroy("alpha").expect("destroy alpha");
    assert!(reg.get("alpha").is_none());
    let names: Vec<String> = reg.list().into_iter().map(|m| m.name).collect();
    assert_eq!(names, vec!["beta".to_string()]);
}

#[valtron_test]
fn an_instance_answers_through_its_model() {
    let reg = registry();
    let inst = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
        .expect("create");
    let answer = inst.session().ask("hi").expect("turn starts");
    assert_eq!(answer.text(), "hello");
}

#[valtron_test]
fn duplicate_unknown_and_missing_names_are_typed_errors() {
    let reg = registry();
    reg.create("alpha", &InstanceConfig::for_model("mock"))
        .expect("create");

    let dup = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
        .expect_err("duplicate");
    assert!(matches!(dup.current_context(), AgentRpcError::InstanceExists(n) if n == "alpha"));

    let unknown = reg
        .create("gamma", &InstanceConfig::for_model("nope"))
        .expect_err("unknown model");
    assert!(matches!(
        unknown.current_context(),
        AgentRpcError::ModelNotFound(_)
    ));

    let missing = reg.destroy("zeta").expect_err("missing");
    assert!(matches!(
        missing.current_context(),
        AgentRpcError::InstanceNotFound(_)
    ));
}

#[valtron_test]
fn instance_names_are_validated() {
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
            .expect_err(bad);
        assert!(
            matches!(err.current_context(), AgentRpcError::InvalidName(_)),
            "{bad:?} -> {err:?}"
        );
    }
    reg.create("ok-name_1.v2", &InstanceConfig::for_model("mock"))
        .expect("valid name");
}

#[valtron_test]
fn the_instance_limit_is_configuration() {
    let reg = InstanceRegistry::new(
        vec![mock_model("mock", "hi")],
        RegistryConfig::default().with_max_instances(2),
    )
    .expect("registry");
    reg.create("a", &InstanceConfig::for_model("mock"))
        .expect("a");
    reg.create("b", &InstanceConfig::for_model("mock"))
        .expect("b");
    let err = reg
        .create("c", &InstanceConfig::for_model("mock"))
        .expect_err("limit");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceLimit(2)
    ));
}

#[valtron_test]
fn creating_instances_never_reloads_the_model() {
    let loads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&loads);
    let model = ModelHandle::load_with("tiny", move || {
        counter.fetch_add(1, Ordering::SeqCst);
        let handle = foundation_agent_rpc::ModelSource::candle(
            "tiny",
            fixture_dir("tiny-random-LlamaForCausalLM"),
        );
        // Load the concrete candle model through the public loader path.
        load_candle(&handle)
    })
    .expect("loads");

    let reg =
        InstanceRegistry::new(vec![model.clone()], RegistryConfig::default()).expect("registry");
    for i in 0..10 {
        let inst = reg
            .create(&format!("inst-{i}"), &InstanceConfig::for_model("tiny"))
            .expect("create");
        assert!(
            inst.model().same_model(&model),
            "instance shares the one model"
        );
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1, "ten instances, one load");
}

#[valtron_test]
fn a_second_concurrent_turn_on_one_instance_is_rejected() {
    let reg = registry();
    let inst = reg
        .create("alpha", &InstanceConfig::for_model("mock"))
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
    source: &foundation_agent_rpc::ModelSource,
) -> foundation_agent_rpc::AgentRpcResult<foundation_ai::backends::candle::CandleModels> {
    use foundation_ai::backends::candle::CandleBackend;
    use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
    let foundation_agent_rpc::ModelSource::Candle { name, dir } = source;
    CandleBackend::cpu()
        .get_model_by_spec(ModelSpec {
            name: name.clone(),
            id: ModelId::Name(name.clone(), None),
            devices: None,
            model_location: Some(dir.clone()),
            lora_location: None,
        })
        .map_err(|e| AgentRpcError::ModelLoad(e.to_string()).into())
}
