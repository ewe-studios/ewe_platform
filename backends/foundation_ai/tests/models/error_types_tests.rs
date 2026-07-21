//! Coverage for the root error taxonomy in `foundation_ai::errors`.
//!
//! WHY: `GenerationError`, `ModelErrors`, `ModelProviderErrors` and
//! `FoundationAIErrors` are the types every public API returns, yet their
//! `Display` output and `From` conversions had no tests — the existing
//! `errors_tests.rs` covers the *agentic* `AgenticError`, which is a different
//! type. The `Display` strings are user-facing (they surface in CLI output and
//! logs) and the `From` chain is what lets `?` bubble a low-level failure to the
//! root type, so both deserve locking down.
//!
//! WHAT: one test per `Display` arm and per `From` impl, plus the nesting
//! behaviour when errors are wrapped through several layers.
//!
//! HOW: pure construction + formatting. No providers, no models, no network —
//! these run everywhere and in milliseconds.

use foundation_ai::errors::{
    FoundationAIErrors, GenerationError, ModelErrors, ModelProviderErrors,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A `BoxedError` carrying a known message, for the variants that wrap one.
fn boxed(msg: &'static str) -> foundation_core::extensions::result_ext::BoxedError {
    Box::new(std::io::Error::other(msg))
}

// ---------------------------------------------------------------------------
// GenerationError — Display
// ---------------------------------------------------------------------------

#[test]
fn generation_error_display_covers_every_portable_arm() {
    // `Failed` wraps a BoxedError and prefixes it.
    let failed = GenerationError::Failed(boxed("io blew up"));
    let rendered = failed.to_string();
    assert!(
        rendered.starts_with("Generation failed: "),
        "unexpected prefix: {rendered}"
    );
    assert!(rendered.contains("io blew up"), "inner error must be shown: {rendered}");

    assert_eq!(
        GenerationError::Tokenizer("bad vocab".into()).to_string(),
        "Tokenizer error: bad vocab"
    );
    assert_eq!(
        GenerationError::Backend("pool gone".into()).to_string(),
        "Backend error: pool gone"
    );
    assert_eq!(
        GenerationError::Generic("something".into()).to_string(),
        "Generation error: something"
    );
}

#[test]
fn generation_error_from_string_is_generic() {
    let err: GenerationError = "plain message".to_string().into();
    assert!(
        matches!(err, GenerationError::Generic(ref m) if m == "plain message"),
        "String must convert to the Generic arm, got: {err:?}"
    );
    assert_eq!(err.to_string(), "Generation error: plain message");
}

#[test]
fn generation_error_from_boxed_is_failed() {
    let err: GenerationError = boxed("wrapped").into();
    assert!(
        matches!(err, GenerationError::Failed(_)),
        "BoxedError must convert to the Failed arm, got: {err:?}"
    );
    assert!(err.to_string().contains("wrapped"));
}

#[test]
fn generation_error_is_a_std_error() {
    // The `impl std::error::Error` is what lets these cross `Box<dyn Error>`
    // boundaries and participate in `?`.
    fn as_std(e: &dyn std::error::Error) -> String {
        e.to_string()
    }
    let err = GenerationError::Generic("boxed up".into());
    assert_eq!(as_std(&err), "Generation error: boxed up");
}

// ---------------------------------------------------------------------------
// ModelErrors — Display
// ---------------------------------------------------------------------------

#[test]
fn model_errors_display_covers_every_portable_arm() {
    assert_eq!(
        ModelErrors::NotFound("llama-3".into()).to_string(),
        "Model not found: llama-3"
    );
    assert_eq!(
        ModelErrors::CandleModelLoad("no safetensors".into()).to_string(),
        "Candle model load error: no safetensors"
    );
    assert_eq!(
        ModelErrors::UnsupportedArchitecture("mamba".into()).to_string(),
        "Unsupported architecture: mamba"
    );

    let loading = ModelErrors::FailedLoading(boxed("disk error")).to_string();
    assert!(loading.starts_with("Model failed to load: "), "got: {loading}");
    assert!(loading.contains("disk error"), "got: {loading}");
}

// ---------------------------------------------------------------------------
// ModelProviderErrors — Display + From
// ---------------------------------------------------------------------------

#[test]
fn model_provider_errors_display_covers_every_arm() {
    assert_eq!(
        ModelProviderErrors::NotFound("anthropic".into()).to_string(),
        "Not found: anthropic"
    );

    let fetching = ModelProviderErrors::FailedFetching(boxed("429")).to_string();
    assert!(fetching.starts_with("Fetch failed: "), "got: {fetching}");
    assert!(fetching.contains("429"), "got: {fetching}");

    let nested = ModelProviderErrors::ModelErrors(ModelErrors::NotFound("gpt-4o".into()));
    assert_eq!(nested.to_string(), "Model error: Model not found: gpt-4o");
}

#[test]
fn model_errors_converts_into_provider_errors() {
    let err: ModelProviderErrors = ModelErrors::NotFound("x".into()).into();
    assert!(
        matches!(err, ModelProviderErrors::ModelErrors(_)),
        "ModelErrors must nest under ModelErrors arm, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// FoundationAIErrors — Display + the three From impls
// ---------------------------------------------------------------------------

#[test]
fn foundation_errors_display_covers_every_arm() {
    assert_eq!(
        FoundationAIErrors::ModelErrors(ModelErrors::NotFound("m".into())).to_string(),
        "Model error: Model not found: m"
    );
    assert_eq!(
        FoundationAIErrors::GenerationErrors(GenerationError::Generic("g".into())).to_string(),
        "Generation error: Generation error: g"
    );
    assert_eq!(
        FoundationAIErrors::RegistryErrors(ModelProviderErrors::NotFound("p".into())).to_string(),
        "Registry error: Not found: p"
    );
}

#[test]
fn foundation_errors_from_model_errors() {
    let err: FoundationAIErrors = ModelErrors::UnsupportedArchitecture("rwkv".into()).into();
    assert!(matches!(err, FoundationAIErrors::ModelErrors(_)), "got: {err:?}");
    assert!(err.to_string().contains("rwkv"));
}

#[test]
fn foundation_errors_from_generation_error() {
    let err: FoundationAIErrors = GenerationError::Backend("no workers".into()).into();
    assert!(
        matches!(err, FoundationAIErrors::GenerationErrors(_)),
        "got: {err:?}"
    );
    assert!(err.to_string().contains("no workers"));
}

#[test]
fn foundation_errors_from_provider_errors() {
    let err: FoundationAIErrors = ModelProviderErrors::NotFound("openai".into()).into();
    assert!(
        matches!(err, FoundationAIErrors::RegistryErrors(_)),
        "got: {err:?}"
    );
    assert!(err.to_string().contains("openai"));
}

#[test]
fn error_chain_nests_through_all_three_layers() {
    // ModelErrors → ModelProviderErrors → FoundationAIErrors, the exact path a
    // `?` takes from a provider lookup up to a public API boundary. Each layer
    // must keep the innermost message visible.
    let root: ModelErrors = ModelErrors::NotFound("deep-model".into());
    let provider: ModelProviderErrors = root.into();
    let top: FoundationAIErrors = provider.into();

    let rendered = top.to_string();
    assert!(
        rendered.contains("deep-model"),
        "the innermost message must survive both conversions: {rendered}"
    );
    assert_eq!(rendered, "Registry error: Model error: Model not found: deep-model");
}
