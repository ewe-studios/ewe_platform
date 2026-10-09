//! `errors/mod.rs` — Display + every From conversion for the base error enums.
//! Pure and offline; these modules were ~8% covered.

use foundation_ai::errors::{
    FoundationAIErrors, GenerationError, ModelErrors, ModelProviderErrors,
};

// ---------------------------------------------------------------------------
// GenerationError

#[test]
fn generation_error_display_and_from() {
    // From<String> and From<BoxedError>.
    let g: GenerationError = "boom".to_string().into();
    assert!(matches!(g, GenerationError::Generic(_)));
    assert!(format!("{g}").contains("boom"));

    let boxed: Box<dyn std::error::Error> = Box::<dyn std::error::Error>::from("inner");
    let g2: GenerationError = boxed.into();
    assert!(matches!(g2, GenerationError::Failed(_)));

    // Each Display arm renders non-empty and self-describing.
    for (e, needle) in [
        (GenerationError::Tokenizer("tk".into()), "Tokenizer"),
        (GenerationError::Backend("be".into()), "Backend"),
        (GenerationError::Generic("gx".into()), "Generation error"),
    ] {
        assert!(format!("{e}").contains(needle), "{e}");
    }
}

#[test]
fn generation_error_from_candle() {
    let c = candle_core::Error::Msg("tensor".into());
    let g: GenerationError = c.into();
    assert!(matches!(g, GenerationError::Candle(_)));
    assert!(format!("{g}").contains("Candle"));
}

// ---------------------------------------------------------------------------
// ModelErrors

#[test]
fn model_errors_display() {
    for (e, needle) in [
        (ModelErrors::NotFound("m".into()), "not found"),
        (ModelErrors::CandleModelLoad("bad".into()), "Candle model load"),
        (ModelErrors::UnsupportedArchitecture("qwen".into()), "Unsupported architecture"),
    ] {
        let s = format!("{e}");
        assert!(
            s.to_lowercase().contains(&needle.to_lowercase()),
            "{s} should contain {needle}"
        );
    }
    let boxed: Box<dyn std::error::Error> = Box::<dyn std::error::Error>::from("load");
    let e = ModelErrors::FailedLoading(boxed);
    assert!(format!("{e}").contains("failed to load"));
}

// ---------------------------------------------------------------------------
// ModelProviderErrors — Display + From<ModelErrors>

#[test]
fn provider_errors_display_and_from_model_errors() {
    let pe: ModelProviderErrors = ModelErrors::NotFound("x".into()).into();
    assert!(matches!(pe, ModelProviderErrors::ModelErrors(_)));
    assert!(format!("{pe}").to_lowercase().contains("not found"));

    let nf = ModelProviderErrors::NotFound("y".into());
    assert!(format!("{nf}").to_lowercase().contains("not found"));

    let boxed: Box<dyn std::error::Error> = Box::<dyn std::error::Error>::from("fetch");
    let ff = ModelProviderErrors::FailedFetching(boxed);
    assert!(format!("{ff}").to_lowercase().contains("fetch"));
}

// ---------------------------------------------------------------------------
// FoundationAIErrors — Display + every From

#[test]
fn foundation_errors_from_each_source() {
    let a: FoundationAIErrors = ModelErrors::NotFound("m".into()).into();
    assert!(matches!(a, FoundationAIErrors::ModelErrors(_)));
    assert!(format!("{a}").to_lowercase().contains("model error"));

    let b: FoundationAIErrors = GenerationError::Generic("g".into()).into();
    assert!(matches!(b, FoundationAIErrors::GenerationErrors(_)));
    assert!(format!("{b}").to_lowercase().contains("generation error"));

    let c: FoundationAIErrors = ModelProviderErrors::NotFound("p".into()).into();
    assert!(matches!(c, FoundationAIErrors::RegistryErrors(_)));
    assert!(format!("{c}").to_lowercase().contains("registry error"));
}

// ---------------------------------------------------------------------------
// model_descriptors() — the generated provider descriptor registry
// (models/providers/mod.rs was 0%).

#[test]
fn model_descriptors_registry_is_populated_and_wellformed() {
    let all = foundation_ai::models::model_descriptors();
    assert!(!all.is_empty(), "the descriptor registry must not be empty");

    for d in all {
        assert!(!d.id.is_empty(), "every descriptor has a non-empty id");
        assert!(!d.name.is_empty(), "every descriptor has a non-empty name: {}", d.id);
        assert!(d.context_window > 0, "context window must be positive for {}", d.id);
    }

    // Multiple providers are represented (the concatenation actually ran).
    let providers: std::collections::BTreeSet<_> =
        all.iter().map(|d| format!("{:?}", d.provider)).collect();
    assert!(
        providers.len() > 1,
        "the registry must span multiple providers, got {providers:?}"
    );
}
