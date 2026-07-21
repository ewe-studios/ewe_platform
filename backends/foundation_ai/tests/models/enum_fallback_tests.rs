//! The `Custom`/`Message` fallback arms of the `From<&str>` conversions.
//!
//! WHY: six enums in `base_types` parse vendor strings — `CacheRetention`,
//! `ThinkingLevels`, `ModelProviders`, `ModelAPI`, `MimeType`, `StopReason`.
//! Each ends in a catch-all that preserves the unrecognised string. Those
//! catch-alls had no coverage, and they are precisely what runs when a vendor
//! ships a value we have not seen: if a fallback silently mapped to a *known*
//! variant instead of preserving the text, a new provider/model/stop-reason
//! would be misclassified with no error anywhere.
//!
//! WHAT: for each enum, a known value maps to its variant AND an unknown value
//! round-trips through the catch-all with the original string intact. Plus
//! `ModelId::name()` across all four variants and `Args`'s hand-written `Debug`.
//!
//! HOW: pure conversions. Note these are `From<&'static str>`, so the inputs
//! must be literals.

use foundation_ai::types::{
    Args, CacheRetention, MimeType, ModelAPI, ModelId, ModelProviders, StopReason, ThinkingLevels,
};

// ---------------------------------------------------------------------------
// CacheRetention
// ---------------------------------------------------------------------------

#[test]
fn cache_retention_known_values_map_to_variants() {
    assert_eq!(CacheRetention::from("short"), CacheRetention::Short);
    assert_eq!(CacheRetention::from("long"), CacheRetention::Long);
}

#[test]
fn cache_retention_unknown_value_is_preserved_as_custom() {
    assert_eq!(
        CacheRetention::from("forever"),
        CacheRetention::Custom("forever".to_string()),
        "an unrecognised retention must keep its text, not collapse to a known variant"
    );
}

// ---------------------------------------------------------------------------
// ThinkingLevels
// ---------------------------------------------------------------------------

#[test]
fn thinking_levels_known_values_map_to_variants() {
    assert_eq!(ThinkingLevels::from("medium"), ThinkingLevels::Medium);
    assert_eq!(ThinkingLevels::from("minimal"), ThinkingLevels::Minimal);
}

#[test]
fn thinking_levels_unknown_value_is_preserved_as_custom() {
    assert_eq!(
        ThinkingLevels::from("ultra"),
        ThinkingLevels::Custom("ultra".to_string()),
        "a new reasoning level must survive as Custom"
    );
}

// ---------------------------------------------------------------------------
// ModelProviders
// ---------------------------------------------------------------------------

#[test]
fn model_providers_known_values_map_to_variants() {
    assert_eq!(ModelProviders::from("huggingface"), ModelProviders::HUGGINGFACE);
    assert_eq!(ModelProviders::from("opencode"), ModelProviders::OPENCODE);
    assert_eq!(ModelProviders::from("kimi-coding"), ModelProviders::KIMICODING);
    assert_eq!(ModelProviders::from("minimax-cn"), ModelProviders::MINIMAXCN);
}

#[test]
fn model_providers_unknown_value_is_preserved_as_custom() {
    // A provider we have not enumerated must still be routable by name.
    assert_eq!(
        ModelProviders::from("brand-new-inc"),
        ModelProviders::Custom("brand-new-inc".to_string())
    );
}

#[test]
fn model_providers_lookup_is_case_sensitive() {
    // The catalog uses lowercase ids; treating "HuggingFace" as HUGGINGFACE
    // would hide a malformed catalog entry.
    assert_eq!(
        ModelProviders::from("HuggingFace"),
        ModelProviders::Custom("HuggingFace".to_string())
    );
}

// ---------------------------------------------------------------------------
// ModelAPI
// ---------------------------------------------------------------------------

#[test]
fn model_api_known_values_map_to_variants() {
    assert_eq!(ModelAPI::from("anthropic-messages"), ModelAPI::AnthropicMessages);
    assert_eq!(ModelAPI::from("google-vertex"), ModelAPI::GoogleVertex);
    assert_eq!(
        ModelAPI::from("google-generative-ai"),
        ModelAPI::GoogleGenerativeAi
    );
    assert_eq!(ModelAPI::from("google-gemini-cli"), ModelAPI::GoogleGeminiCli);
    assert_eq!(
        ModelAPI::from("bedrock-converse-stream"),
        ModelAPI::BedrockConverseStream
    );
}

#[test]
fn model_api_unknown_value_is_preserved_as_custom() {
    // generator.rs feeds vendor `api` strings straight in — an unknown wire
    // protocol must be visible, not silently aliased onto a known one.
    assert_eq!(
        ModelAPI::from("some-new-protocol"),
        ModelAPI::Custom("some-new-protocol".to_string())
    );
}

// ---------------------------------------------------------------------------
// MimeType
// ---------------------------------------------------------------------------

#[test]
fn mime_type_known_values_map_to_variants() {
    assert_eq!(MimeType::from("audio/mpeg"), MimeType::AudioMpeg);
    assert_eq!(MimeType::from("video/mp4"), MimeType::VideoMp4);
    assert_eq!(MimeType::from("video/webm"), MimeType::VideoWebm);
    assert_eq!(MimeType::from("video/ogg"), MimeType::VideoOgg);
}

#[test]
fn mime_type_unknown_value_is_preserved_as_custom() {
    assert_eq!(
        MimeType::from("application/x-thing"),
        MimeType::Custom("application/x-thing".to_string()),
        "an unknown mime type must round-trip so multimodal payloads are not mislabelled"
    );
}

// ---------------------------------------------------------------------------
// StopReason
// ---------------------------------------------------------------------------

#[test]
fn stop_reason_known_values_map_to_variants() {
    assert_eq!(StopReason::from("error"), StopReason::Error);
    assert_eq!(StopReason::from("aborted"), StopReason::Aborted);
}

#[test]
fn stop_reason_unknown_value_is_preserved_as_message() {
    // StopReason's catch-all is `Message`, not `Custom` — a vendor-specific
    // stop reason is kept verbatim so it can be surfaced to the user.
    assert_eq!(
        StopReason::from("content_filter"),
        StopReason::Message("content_filter".to_string())
    );
}

// ---------------------------------------------------------------------------
// ModelId::name — every variant
// ---------------------------------------------------------------------------

#[test]
fn model_id_name_reads_the_string_from_every_variant() {
    // `name()` is how routing resolves a model; a variant returning the wrong
    // field would make that model unroutable.
    assert_eq!(ModelId::Name("a".into(), None).name(), "a");
    assert_eq!(ModelId::Alias("b".into(), None).name(), "b");
    assert_eq!(ModelId::Group("c".into(), None).name(), "c");
    assert_eq!(ModelId::Architecture("d".into(), None).name(), "d");
}

#[test]
fn model_id_display_matches_name() {
    // Display delegates to name(); routing rules and log lines must agree.
    let id = ModelId::Alias("gpt-4o".into(), None);
    assert_eq!(id.to_string(), id.name());
}

#[test]
fn model_id_name_ignores_quantization() {
    use foundation_ai::types::Quantization;
    // The quantization rides along but must not leak into the routing name.
    let plain = ModelId::Name("m".into(), None);
    let quant = ModelId::Name("m".into(), Some(Quantization::Q4_KM));
    assert_eq!(plain.name(), quant.name());
}

// ---------------------------------------------------------------------------
// Args — hand-written Debug
// ---------------------------------------------------------------------------

#[test]
fn args_debug_shows_the_schema() {
    // Args holds a compiled validator that has no useful Debug, so the impl is
    // hand-written to print the schema. If it regressed to the derived form the
    // output would be unreadable in every tool-error message.
    let args = Args::from_value(serde_json::json!({"type": "object"}));
    let rendered = format!("{args:?}");
    assert!(rendered.contains("Args"), "got: {rendered}");
    assert!(
        rendered.contains("schema"),
        "the schema field must be shown: {rendered}"
    );
    assert!(
        rendered.contains("object"),
        "the schema contents must be shown: {rendered}"
    );
}
