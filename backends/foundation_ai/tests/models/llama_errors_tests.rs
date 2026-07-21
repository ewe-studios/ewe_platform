//! `errors/llama.rs` — `LlamaError` Display + the From conversions from each
//! source llama.cpp error into `LlamaError` / `GenerationError` / `ModelErrors`.
//! Pure and offline; this module was 0% covered.

use foundation_ai::errors::llama::LlamaError;
use foundation_ai::errors::{GenerationError, ModelErrors};
use infrastructure_llama_cpp::{
    DecodeError, EmbeddingsError, EncodeError, LlamaCppError,
};

#[test]
fn llama_error_display_covers_each_variant() {
    // Construct the easily-constructible unit-variant sources and wrap them.
    let cases: Vec<LlamaError> = vec![
        LlamaCppError::BackendAlreadyInitialized.into(),
        DecodeError::NoKvCacheSlot.into(),
        EncodeError::NTokensZero.into(),
        EmbeddingsError::NotEnabled.into(),
    ];
    for e in &cases {
        let s = format!("{e}");
        assert!(!s.is_empty(), "LlamaError Display must render: {e:?}");
    }
}

#[test]
fn from_llama_error_into_generation_error() {
    let le: LlamaError = DecodeError::NoKvCacheSlot.into();
    let g: GenerationError = le.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    assert!(format!("{g}").to_lowercase().contains("llama"));
}

#[test]
fn from_llama_error_into_model_errors() {
    let le: LlamaError = EncodeError::NoKvCacheSlot.into();
    let m: ModelErrors = le.into();
    assert!(matches!(m, ModelErrors::Llama(_)));
    assert!(format!("{m}").to_lowercase().contains("llama"));
}

#[test]
fn from_source_errors_directly_into_generation_error() {
    // errors/llama.rs also provides From<SourceError> for GenerationError.
    let g1: GenerationError = LlamaCppError::BackendAlreadyInitialized.into();
    assert!(matches!(g1, GenerationError::Llama(_)));

    let g2: GenerationError = DecodeError::NTokensZero.into();
    assert!(matches!(g2, GenerationError::Llama(_)));

    let g3: GenerationError = EncodeError::NoKvCacheSlot.into();
    assert!(matches!(g3, GenerationError::Llama(_)));

    let g4: GenerationError = EmbeddingsError::LogitsNotEnabled.into();
    assert!(matches!(g4, GenerationError::Llama(_)));
}

#[test]
fn llama_cpp_error_wraps_sub_errors() {
    // LlamaCppError::DecodeError(#[from] DecodeError) → LlamaError::Cpp.
    let cpp: LlamaCppError = DecodeError::NoKvCacheSlot.into();
    let le: LlamaError = cpp.into();
    assert!(matches!(le, LlamaError::Cpp(_)));
    assert!(format!("{le}").to_lowercase().contains("llama.cpp"));
}
