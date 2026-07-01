//! MTP capability-gate integration test (spec-51 G2).
//!
//! WHY: enabling MTP on a model that has no MTP head must fail at model load —
//! not silently do nothing. This uses the small SmolLM2 test model (no MTP
//! head) and drives `LlamaBackends::load_model` directly with a local model
//! spec, so it tests exactly the capability gate without the download-by-id
//! machinery.
//!
//! Gated behind the `integration_tests` feature (like the other pull tests);
//! downloads the ~150 MB SmolLM2 GGUF via TestHarness.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{ModelId, ModelSpec};
use foundation_core::valtron::valtron_test;
use foundation_testing::huggingface::TestHarness;
use tracing_test::traced_test;

fn project_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories")
        .to_path_buf()
}

fn smollm_spec() -> ModelSpec {
    let harness = TestHarness::new(&project_root());
    let model_path = harness
        .get_smollm_model()
        .expect("Failed to download SmolLM2 test model");
    ModelSpec {
        name: "smollm2-360m".to_string(),
        id: ModelId::Name("smollm2".to_string(), None),
        devices: None,
        model_location: Some(model_path.to_string_lossy().to_string().into()),
        lora_location: None,
    }
}

/// Enabling MTP on a model without an MTP head must error at load time.
#[valtron_test]
#[traced_test]
fn test_mtp_on_unsupported_model_errors() {
    let backend = LlamaBackends::LLamaCPU;
    let mtp = LlamaBackendConfig::builder()
        .mtp(None, 4)
        .build()
        .speculative;

    let result = backend.load_model(smollm_spec(), mtp);

    assert!(
        result.is_err(),
        "MTP on a model without an MTP head must fail at load, got Ok"
    );
    let msg = format!("{:?}", result.err().unwrap());
    assert!(
        msg.to_uppercase().contains("MTP"),
        "error should mention MTP; got: {msg}"
    );
}

/// The same model loads fine with MTP disabled (control).
#[valtron_test]
#[traced_test]
fn test_no_mtp_loads_normally() {
    let backend = LlamaBackends::LLamaCPU;
    let result = backend.load_model(smollm_spec(), None);
    assert!(
        result.is_ok(),
        "SmolLM2 should load with standard decoding: {:?}",
        result.err()
    );
}
