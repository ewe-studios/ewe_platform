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
