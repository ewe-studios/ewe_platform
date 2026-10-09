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

#[test]
fn remaining_source_conversions_into_llama_and_generation() {
    use infrastructure_llama_cpp::{
        ChatTemplateError, LlamaContextLoadError, LlamaModelLoadError, StringToTokenError,
        TokenToStringError,
    };

    // Unit-variant sources → LlamaError + Display (LlamaError isn't Clone, so
    // build each twice: once to render Display, once to convert).
    for e in [
        LlamaError::from(ChatTemplateError::MissingTemplate),
        LlamaError::from(LlamaContextLoadError::NullReturn),
        LlamaError::from(LlamaModelLoadError::NullResult),
        LlamaError::from(TokenToStringError::UnknownTokenType),
    ] {
        assert!(!format!("{e}").is_empty(), "Display: {e:?}");
        let g: GenerationError = e.into();
        assert!(matches!(g, GenerationError::Llama(_)));
    }

    // StringToTokenError via a real TryFromIntError.
    let int_err = u8::try_from(300_i32).unwrap_err();
    let ste: StringToTokenError = int_err.into();
    let le: LlamaError = ste.into();
    assert!(matches!(le, LlamaError::Tokenization(_)));
    assert!(format!("{le}").to_lowercase().contains("token"));

    // Direct From<StringToTokenError> for GenerationError.
    let int_err2 = u8::try_from(400_i32).unwrap_err();
    let g: GenerationError = StringToTokenError::from(int_err2).into();
    assert!(matches!(g, GenerationError::Llama(_)));
}

#[test]
fn apply_chat_template_error_display_and_conversions() {
    use infrastructure_llama_cpp::ApplyChatTemplateError;

    // The ApplyChatTemplate Display arm + LlamaError wrap.
    let le = LlamaError::from(ApplyChatTemplateError::TemplateNotApplicable(-1));
    let rendered = format!("{le}").to_lowercase();
    assert!(rendered.contains("apply chat template"), "Display: {le}");

    // Direct From<ApplyChatTemplateError> for GenerationError.
    let g: GenerationError = ApplyChatTemplateError::TemplateNotApplicable(-7).into();
    assert!(matches!(g, GenerationError::Llama(_)));
}

#[test]
fn every_source_error_converts_into_generation_error() {
    use infrastructure_llama_cpp::{
        ChatTemplateError, DecodeError, EmbeddingsError, EncodeError, LlamaContextLoadError,
        LlamaCppError, LlamaModelLoadError, TokenToStringError,
    };

    // Each direct From<SourceError> for GenerationError arm.
    let g: GenerationError = LlamaCppError::BackendAlreadyInitialized.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = TokenToStringError::UnknownTokenType.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = DecodeError::NoKvCacheSlot.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = EncodeError::NTokensZero.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = EmbeddingsError::NotEnabled.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = ChatTemplateError::MissingTemplate.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = LlamaModelLoadError::NullResult.into();
    assert!(matches!(g, GenerationError::Llama(_)));
    let g: GenerationError = LlamaContextLoadError::NullReturn.into();
    assert!(matches!(g, GenerationError::Llama(_)));
}

#[test]
fn source_errors_convert_into_model_errors() {
    use infrastructure_llama_cpp::{EmbeddingsError, LlamaContextLoadError, LlamaModelLoadError};

    // The direct From<SourceError> for ModelErrors arms.
    let m: ModelErrors = LlamaModelLoadError::NullResult.into();
    assert!(matches!(m, ModelErrors::Llama(_)));
    let m: ModelErrors = LlamaContextLoadError::NullReturn.into();
    assert!(matches!(m, ModelErrors::Llama(_)));
    let m: ModelErrors = EmbeddingsError::NotEnabled.into();
    assert!(matches!(m, ModelErrors::Llama(_)));
}
