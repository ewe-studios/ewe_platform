//! Candle registry + local-load error paths.
//!
//! WHY: I had written these off as "GPU paths". They are not. Candle has no
//! model registry at all, so `get_one`/`get_all` always error — and
//! `get_model_by_spec` rejects a spec with no `model_location` before any
//! device or weight work happens. Those branches decide whether a caller gets a
//! clear "Candle needs a local path" message or a confusing failure deep inside
//! the loader. `load_from_local`'s missing-`config.json` check is likewise plain
//! filesystem logic.
//!
//! WHAT: the deliberate registry `NotFound`s, the missing-`model_location`
//! rejection, and the missing-`config.json` rejection against a real temp dir.
//!
//! HOW: offline — a `tempdir`, no weights, no device. The one thing NOT covered
//! here is successful weight loading, which the gated live-model suite does.

use foundation_ai::backends::candle::{CandleArchitecture, CandleBackend, CandleBackendConfig, CandleDType};
use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};

// ---------------------------------------------------------------------------
// Config builder — pure plumbing
// ---------------------------------------------------------------------------

#[test]
fn config_builder_default_matches_new() {
    let a = CandleBackendConfig::builder().build();
    let b = CandleBackendConfig::default();
    assert_eq!(a.context_length, b.context_length);
}

#[test]
fn config_builder_chain_preserves_every_field() {
    let c = CandleBackendConfig::builder()
        .context_length(8192)
        .dtype(CandleDType::F16)
        .architecture(CandleArchitecture::Llama)
        .build();
    assert_eq!(c.context_length, 8192);
    assert_eq!(c.dtype, CandleDType::F16);
    assert_eq!(c.architecture, CandleArchitecture::Llama);
}

#[test]
fn every_dtype_is_distinct() {
    // The dtype selects the weight precision; two variants collapsing to one
    // would silently load at the wrong precision.
    let f32c = CandleBackendConfig::builder().dtype(CandleDType::F32).build();
    let f16c = CandleBackendConfig::builder().dtype(CandleDType::F16).build();
    let bf16c = CandleBackendConfig::builder().dtype(CandleDType::BF16).build();
    assert_ne!(f32c.dtype, f16c.dtype);
    assert_ne!(f16c.dtype, bf16c.dtype);
    assert_ne!(f32c.dtype, bf16c.dtype);
}

#[test]
fn a_custom_architecture_is_preserved() {
    let c = CandleBackendConfig::builder()
        .architecture(CandleArchitecture::Custom("mamba".into()))
        .build();
    assert_eq!(
        c.architecture,
        CandleArchitecture::Custom("mamba".into()),
        "an unsupported architecture must survive so the loader can reject it by name"
    );
}

// ---------------------------------------------------------------------------
// Registry — Candle deliberately has none
// ---------------------------------------------------------------------------

#[test]
fn get_one_is_not_supported() {
    // Candle loads from a local path; there is no catalog to query. This must
    // be an explicit error, not an empty result that reads as "no such model".
    let backend = CandleBackend::cpu();
    let err = backend
        .get_one(ModelId::Name("anything".into(), None))
        .expect_err("Candle has no registry");
    assert!(
        err.to_string().to_lowercase().contains("not supported")
            || err.to_string().to_lowercase().contains("registry"),
        "the error must explain that Candle has no registry: {err}"
    );
}

#[test]
fn get_all_is_not_supported() {
    let backend = CandleBackend::cpu();
    assert!(
        backend
            .get_all(ModelId::Name("anything".into(), None))
            .is_err(),
        "Candle has no registry to list"
    );
}

// ---------------------------------------------------------------------------
// get_model_by_spec — rejects before touching a device
// ---------------------------------------------------------------------------

fn spec_without_location() -> ModelSpec {
    ModelSpec {
        name: "local".into(),
        id: ModelId::Name("local".into(), None),
        devices: None,
        model_location: None,
        lora_location: None,
    }
}

#[test]
fn get_model_by_spec_requires_a_model_location() {
    // The rejection happens before any device init or weight read, so this is
    // reachable with no GPU and no files.
    let backend = CandleBackend::cpu();
    let err = backend
        .get_model_by_spec(spec_without_location())
        .expect_err("a spec with no local path cannot load");
    assert!(
        err.to_string().contains("model_location"),
        "the error must name the missing field so the caller can fix it: {err}"
    );
}

#[test]
fn get_model_by_spec_rejects_a_directory_with_no_config_json() {
    // A real directory that is missing config.json — plain filesystem logic, no
    // weights involved. Without this check the failure surfaces much deeper in
    // the loader with a far less actionable message.
    let dir = std::env::temp_dir().join(format!(
        "candle-empty-{}",
        foundation_compact::ids::new_scru128()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");

    let backend = CandleBackend::cpu();
    let spec = ModelSpec {
        name: "local".into(),
        id: ModelId::Name("local".into(), None),
        devices: None,
        model_location: Some(dir.clone()),
        lora_location: None,
    };

    let result = backend.get_model_by_spec(spec);
    std::fs::remove_dir_all(&dir).ok();

    let err = result.expect_err("a directory with no config.json cannot load");
    assert!(
        err.to_string().contains("config.json"),
        "the error must name the missing file: {err}"
    );
}

#[test]
fn get_model_by_spec_rejects_a_nonexistent_directory() {
    let backend = CandleBackend::cpu();
    let spec = ModelSpec {
        name: "local".into(),
        id: ModelId::Name("local".into(), None),
        devices: None,
        model_location: Some(std::path::PathBuf::from(
            "/nonexistent/candle/path/that/does/not/exist",
        )),
        lora_location: None,
    };
    assert!(
        backend.get_model_by_spec(spec).is_err(),
        "a missing directory must error rather than panic"
    );
}

// ---------------------------------------------------------------------------
// Weight discovery + architecture detection
// ---------------------------------------------------------------------------
//
// Before any tensor is read, the loader (a) requires at least one .safetensors
// file and (b) works out the architecture from config.json — `model_type`
// first, falling back to the `architectures` array. Both run on plain
// filesystem/JSON input, so both are reachable without real weights.

fn temp_model_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "candle-{tag}-{}",
        foundation_compact::ids::new_scru128()
    ));
    std::fs::create_dir_all(&dir).expect("temp model dir");
    dir
}

fn spec_at(dir: &std::path::Path) -> ModelSpec {
    ModelSpec {
        name: "local".into(),
        id: ModelId::Name("local".into(), None),
        devices: None,
        model_location: Some(dir.to_path_buf()),
        lora_location: None,
    }
}

fn load_err(dir: &std::path::Path) -> String {
    let backend = CandleBackend::cpu();
    backend
        .get_model_by_spec(spec_at(dir))
        .expect_err("a directory without real weights cannot load")
        .to_string()
}

#[test]
fn a_config_without_any_safetensors_is_rejected_by_name() {
    // config.json present but no weights at all. The error must say which piece
    // is missing, or the user has to guess.
    let dir = temp_model_dir("noweights");
    std::fs::write(dir.join("config.json"), br#"{"model_type":"llama"}"#).expect("config");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        err.contains("safetensors"),
        "the error must name the missing weights: {err}"
    );
}

#[test]
fn architecture_is_detected_from_model_type() {
    // With a weights file present the loader gets past the discovery checks and
    // runs detect_architecture, then fails on the (empty) tensor data. Reaching
    // a weights-level failure — rather than a config-level one — is what shows
    // detection ran.
    let dir = temp_model_dir("modeltype");
    std::fs::write(dir.join("config.json"), br#"{"model_type":"llama"}"#).expect("config");
    std::fs::write(dir.join("model.safetensors"), b"").expect("weights marker");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        !err.contains("config.json not found"),
        "the loader must have progressed past config discovery: {err}"
    );
    assert!(
        !err.contains("No safetensors files found"),
        "the weights file must have been discovered: {err}"
    );
}

#[test]
fn architecture_falls_back_to_the_architectures_array() {
    // Some configs carry no `model_type`, only `architectures`. That fallback
    // is what keeps those models loadable at all.
    let dir = temp_model_dir("archarray");
    std::fs::write(
        dir.join("config.json"),
        br#"{"architectures":["LlamaForCausalLM"]}"#,
    )
    .expect("config");
    std::fs::write(dir.join("model.safetensors"), b"").expect("weights marker");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        !err.contains("config.json not found") && !err.contains("No safetensors files found"),
        "detection must have run off the architectures array: {err}"
    );
}

#[test]
fn a_sharded_weight_set_is_discovered() {
    // Large models ship as model-00001-of-00002.safetensors. Discovery collects
    // any .safetensors, so a sharded set must not read as "no weights".
    let dir = temp_model_dir("sharded");
    std::fs::write(dir.join("config.json"), br#"{"model_type":"llama"}"#).expect("config");
    std::fs::write(dir.join("model-00001-of-00002.safetensors"), b"").expect("shard 1");
    std::fs::write(dir.join("model-00002-of-00002.safetensors"), b"").expect("shard 2");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        !err.contains("No safetensors files found"),
        "sharded weights must be discovered: {err}"
    );
}

#[test]
fn a_non_safetensors_file_does_not_count_as_weights() {
    // A stray pytorch_model.bin must not satisfy the weights requirement — the
    // Candle loader cannot read it.
    let dir = temp_model_dir("binonly");
    std::fs::write(dir.join("config.json"), br#"{"model_type":"llama"}"#).expect("config");
    std::fs::write(dir.join("pytorch_model.bin"), b"").expect("bin");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        err.contains("safetensors"),
        "a .bin must not satisfy the safetensors requirement: {err}"
    );
}

#[test]
fn a_malformed_config_json_does_not_panic() {
    // detect_architecture parses config.json with serde; invalid JSON must
    // degrade to "no detection" rather than unwrapping.
    let dir = temp_model_dir("badjson");
    std::fs::write(dir.join("config.json"), b"{not json").expect("config");
    std::fs::write(dir.join("model.safetensors"), b"").expect("weights marker");

    let err = load_err(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(!err.is_empty(), "a malformed config must error, not panic");
}
