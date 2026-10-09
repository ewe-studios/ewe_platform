//! MTP / speculative-decoding config tests (spec-51, Phase 1).
//!
//! WHY: MTP is opt-in and capability-gated. These offline tests lock in the
//! config surface — the `with_mtp` helper wires a `SpeculativeConfig::Mtp` onto
//! the GGUF config, presets declare `SUPPORTS_MTP` correctly, and the default
//! config carries no speculative setting (zero behavior change unless opted in).
//! The runtime capability gate (error on a model without an MTP head) is
//! exercised by the integration test, since it needs a real model load.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, SpeculativeKind};
use foundation_ai::harness::{self, Gemma4_26b, Glm52, Ornith10, Qwen36};

#[test]
fn default_config_has_no_speculative() {
    // The default must be standard decoding — opt-in only.
    assert!(LlamaBackendConfig::default().speculative.is_none());
}

#[test]
fn with_mtp_sets_speculative_on_gguf_config() {
    let config = harness::with_mtp(None, 4, None);
    let spec = config
        .llama_config
        .speculative
        .expect("with_mtp must set a speculative config");
    assert_eq!(spec.kind, SpeculativeKind::Mtp);
    assert_eq!(spec.n_max, 4);
    assert!(spec.mtp_model.is_none());
}

#[test]
fn with_mtp_preserves_base_config() {
    // Start from a base config and confirm with_mtp only adds speculative.
    let base = foundation_ai::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig::builder()
        .default_quantization("Q5_K_M")
        .build();
    let config = harness::with_mtp(Some(base), 8, None);
    assert_eq!(config.default_quantization.as_deref(), Some("Q5_K_M"));
    assert_eq!(config.llama_config.speculative.unwrap().n_max, 8);
}

#[test]
fn builder_mtp_helper_matches_explicit() {
    let via_builder = LlamaBackendConfig::builder().mtp(None, 2).build();
    let spec = via_builder.speculative.expect("builder .mtp sets speculative");
    assert_eq!(spec.kind, SpeculativeKind::Mtp);
    assert_eq!(spec.n_max, 2);
}

#[test]
fn presets_declare_mtp_support() {
    // GLM 5.2, Qwen 3.6, Gemma 4 ship MTP heads; Ornith does not.
    assert!(Glm52::SUPPORTS_MTP);
    assert!(Qwen36::SUPPORTS_MTP);
    assert!(Gemma4_26b::SUPPORTS_MTP);
    assert!(!Ornith10::SUPPORTS_MTP);
}
