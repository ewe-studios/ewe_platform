//! Smoke test: the committed tiny-random fixtures load through CandleBackend.
//!
//! WHY: `artefacts/test-models/` costs ~54 MB of permanent repo history. That is
//! only justified if the fixtures actually load offline through our own backend
//! — this test is the evidence, and fails loudly if a fixture rots.

use foundation_ai::backends::candle::CandleBackend;
use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
use foundation_core::valtron::valtron_test;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

#[valtron_test]
fn tiny_random_llama_fixture_loads_offline() {
    let dir = fixture_dir("tiny-random-LlamaForCausalLM");
    assert!(dir.join("model.safetensors").exists(), "fixture missing: {dir:?}");

    let backend = CandleBackend::cpu();
    let spec = ModelSpec {
        name: "tiny-random-llama".to_string(),
        id: ModelId::Name("tiny-random-llama".to_string(), None),
        devices: None,
        model_location: Some(dir.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let result = backend.get_model_by_spec(spec);
    assert!(result.is_ok(), "fixture should load: {:?}", result.err());
}
