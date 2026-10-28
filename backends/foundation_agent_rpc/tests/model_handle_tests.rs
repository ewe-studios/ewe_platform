//! T3 — `ModelHandle`: an `Arc`-backed, load-once handle to one model.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_agent_rpc::{AgentRpcError, ModelHandle, ModelSource};
use foundation_core::valtron::valtron_test;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

#[test]
fn model_handle_is_clone_send_sync_static() {
    fn assert_bounds<T: Clone + Send + Sync + 'static>() {}
    assert_bounds::<ModelHandle>();
}

#[valtron_test]
fn loads_a_candle_model_once_and_clones_share_it() {
    let handle = ModelHandle::load(&ModelSource::candle(
        "tiny-llama",
        fixture_dir("tiny-random-LlamaForCausalLM"),
    ))
    .expect("fixture loads");
    assert_eq!(handle.name(), "tiny-llama");

    let clone = handle.clone();
    assert!(
        clone.same_model(&handle),
        "a clone is the same loaded model"
    );

    // The router a session would be built on serves the handle's model.
    let model = handle
        .router()
        .get_model(handle.model_id())
        .expect("router serves the loaded model");
    assert_eq!(model.spec().name, "tiny-llama");
}

#[valtron_test]
fn the_loader_runs_exactly_once() {
    let loads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&loads);
    let handle = ModelHandle::load_with("counted", move || {
        use foundation_ai::backends::candle::CandleBackend;
        use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
        counter.fetch_add(1, Ordering::SeqCst);
        let dir = fixture_dir("tiny-random-LlamaForCausalLM");
        CandleBackend::cpu()
            .get_model_by_spec(ModelSpec {
                name: "counted".into(),
                id: ModelId::Name("counted".into(), None),
                devices: None,
                model_location: Some(dir.to_string_lossy().to_string().into()),
                lora_location: None,
            })
            .map_err(|e| AgentRpcError::ModelLoad(e.to_string()).into())
    })
    .expect("loads");

    for _ in 0..8 {
        let routed = handle.clone().router();
        routed
            .get_model(handle.model_id())
            .expect("each router hands out the shared model");
    }
    assert_eq!(
        loads.load(Ordering::SeqCst),
        1,
        "no reload after the first load"
    );
}

#[valtron_test]
fn a_missing_model_directory_is_a_model_load_error() {
    let err = ModelHandle::load(&ModelSource::candle("ghost", "/nonexistent/model/dir"))
        .expect_err("must fail");
    assert!(
        matches!(err.current_context(), AgentRpcError::ModelLoad(_)),
        "got {err:?}"
    );
}

#[test]
fn two_separate_loads_are_different_models() {
    use foundation_ai::agentic::testing::MockModelProvider;
    let a = ModelHandle::from_router("a", "mock", MockModelProvider::new().into_router());
    let b = ModelHandle::from_router("b", "mock", MockModelProvider::new().into_router());
    assert!(!a.same_model(&b));
    assert!(a.clone().same_model(&a));
}
