//! A model or LoRA path that does not exist is a typed error, not a panic.
//!
//! WHY: `load_from_file` / `lora_adapter_init` used `debug_assert!(path.exists())`,
//! so in debug and `uat` builds a missing file panicked (taking the caller's
//! thread or test with it), and in release llama.cpp only reported an opaque
//! null result. Found loading a missing GGUF through agent-rpc's `ModelHandle`.

use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::LlamaModel;
use infrastructure_llama_cpp::LlamaModelLoadError;

#[test]
fn loading_a_missing_model_file_is_file_not_found() {
    let backend = LlamaBackend::init_or_get().expect("backend initialises");
    let path = std::path::Path::new("/no/such/model.gguf");
    let err = LlamaModel::load_from_file(&backend, path, &LlamaModelParams::default())
        .err()
        .expect("missing file must not load");
    assert_eq!(err, LlamaModelLoadError::FileNotFound(path.to_path_buf()));
}
