//! Tests of the model-descriptor generator's offline helpers: the catalog
//! parsers, the overrides, the static catalogs and the Rust source it writes.
//! The network fetch (`generate_model_descriptors`) is not run here.

use std::collections::BTreeMap;

use foundation_ai::models::generator::{
    api_to_variant, apply_overrides, ctx, deduplicate, dev_cost, entry, format_f64,
    format_int_with_separators, format_u32, from_dev, generate_provider_file,
    generate_providers_mod, has, has_image, max_tok, parse_ai_gateway_response,
    parse_models_dev_response, parse_openrouter_response, pricing_val, provider_to_filename,
    provider_to_variant, static_antigravity, static_cloud_code_assist, static_codex_models,
    static_kimi_fallbacks, static_vertex, verify_struct_shape, ModelEntry, ModelsDevCost,
    ModelsDevLimit, ModelsDevModalities, ModelsDevModel,
};

#[test]
fn format_helpers_render_expected() {
    assert_eq!(format_f64(0.0), "0.0");
    assert_eq!(format_f64(1.5), "1.5");
    assert_eq!(format_f64(3.0), "3.0");
    // >= 10_000 gets thousands separators.
    assert!(format_f64(12_345.0).contains('_') || format_f64(12_345.0).contains(','));

    assert_eq!(format_int_with_separators("1000000"), "1_000_000");
    assert_eq!(format_int_with_separators("999"), "999");
    assert!(!format_u32(128_000).is_empty());
}

#[test]
fn format_f64_separates_the_integer_part_only() {
    // A large value with a fraction must get separators in the INTEGER part
    // and leave the fraction untouched — separating the fraction would emit
    // invalid Rust into the generated descriptors.
    let out = format_f64(12_345.75);
    assert!(out.contains('_'), "the integer part must be separated: {out}");
    assert!(out.ends_with(".75"), "the fraction must survive intact: {out}");
    assert!(
        !out.split('.').nth(1).unwrap_or("").contains('_'),
        "the fraction must not be separated: {out}"
    );
}

#[test]
fn format_int_with_separators_keeps_a_negative_sign_outside() {
    // The sign is stripped, the digits grouped, then the sign restored.
    // Grouping with the sign attached would produce "-_1_000".
    assert_eq!(format_int_with_separators("-1000000"), "-1_000_000");
    assert_eq!(format_int_with_separators("-999"), "-999");
}

#[test]
fn format_int_with_separators_groups_in_threes_from_the_right() {
    assert_eq!(format_int_with_separators("1000"), "1_000");
    assert_eq!(format_int_with_separators("10000"), "10_000");
    assert_eq!(format_int_with_separators("100000"), "100_000");
    assert_eq!(
        format_int_with_separators("1234567890"),
        "1_234_567_890",
        "grouping must start from the right, not the left"
    );
}

#[test]
fn format_int_with_separators_handles_degenerate_input() {
    assert_eq!(format_int_with_separators(""), "");
    assert_eq!(format_int_with_separators("0"), "0");
    assert_eq!(format_int_with_separators("-"), "-");
}

// -----------------------------------------------------------------
// generate_provider_file — the per-model branches
// -----------------------------------------------------------------

fn model_with(provider: &str, id: &str, image: bool, base_url: &str) -> ModelEntry {
    entry(
        id,
        id,
        "anthropic-messages",
        provider,
        base_url,
        false,
        image,
        (0.0, 0.0, 0.0, 0.0),
        4096,
        4096,
    )
}

#[test]
fn generated_file_marks_image_capable_models() {
    // `has_image_input` selects the MessageType variant written into the
    // descriptor. Getting it wrong means a vision model is advertised as
    // text-only (or vice versa) to every caller.
    let mut by_id = BTreeMap::new();
    by_id.insert(
        "vision".to_string(),
        model_with("openai", "vision", true, "https://x.test"),
    );
    let src = generate_provider_file("openai", &by_id);
    assert!(
        src.contains("MessageType::TextAndImages"),
        "an image-capable model must be marked: {src}"
    );
}

#[test]
fn generated_file_marks_text_only_models() {
    let mut by_id = BTreeMap::new();
    by_id.insert(
        "textonly".to_string(),
        model_with("openai", "textonly", false, "https://x.test"),
    );
    let src = generate_provider_file("openai", &by_id);
    assert!(src.contains("MessageType::Text"));
    assert!(
        !src.contains("MessageType::TextAndImages"),
        "a text-only model must not claim image support: {src}"
    );
}

#[test]
fn generated_file_emits_none_for_an_absent_base_url() {
    // An empty base_url must become `None`, not `Some("")` — the latter
    // would build requests against an empty host.
    let mut by_id = BTreeMap::new();
    by_id.insert(
        "nourl".to_string(),
        model_with("openai", "nourl", false, ""),
    );
    let src = generate_provider_file("openai", &by_id);
    assert!(
        src.contains("base_url: None"),
        "an empty base_url must render as None: {src}"
    );
    assert!(
        !src.contains(r#"base_url: Some("")"#),
        "an empty string base_url would target an empty host: {src}"
    );
}

#[test]
fn generated_file_emits_some_for_a_present_base_url() {
    let mut by_id = BTreeMap::new();
    by_id.insert(
        "withurl".to_string(),
        model_with("openai", "withurl", false, "https://api.example.test"),
    );
    let src = generate_provider_file("openai", &by_id);
    assert!(
        src.contains(r#"Some("https://api.example.test")"#),
        "a present base_url must be emitted: {src}"
    );
}

#[test]
fn api_variant_mapping_known_and_custom() {
    assert_eq!(api_to_variant("anthropic-messages"), "ModelAPI::AnthropicMessages");
    assert_eq!(api_to_variant("openai-completions"), "ModelAPI::OpenAICompletions");
    assert!(api_to_variant("something-else").contains("Custom"));
}

#[test]
fn provider_variant_and_filename_mapping() {
    assert_eq!(provider_to_variant("openai"), "ModelProviders::OPENAI");
    assert_eq!(provider_to_variant("anthropic"), "ModelProviders::ANTHROPIC");
    assert!(provider_to_variant("brand-new").contains("Custom"));
    assert_eq!(provider_to_filename("vercel-ai-gateway"), "vercel_ai_gateway");
}

#[test]
fn static_catalogs_are_nonempty() {
    assert!(!static_codex_models().is_empty());
    assert!(!static_cloud_code_assist().is_empty());
    assert!(!static_antigravity().is_empty());
    assert!(!static_vertex().is_empty());
    assert!(!static_kimi_fallbacks().is_empty());
}

#[test]
fn parse_openrouter_response_keeps_only_tool_models() {
    let body = r#"{"data":[
        {"id":"vendor/with-tools","name":"With Tools","context_length":8192,
         "supported_parameters":["tools","temperature"],
         "architecture":{"modality":"text"},
         "pricing":{"prompt":"0.000001","completion":"0.000002"}},
        {"id":"vendor/no-tools","name":"No Tools","context_length":4096,
         "supported_parameters":["temperature"],
         "architecture":{"modality":"text"},
         "pricing":{"prompt":"0","completion":"0"}}
    ]}"#;
    let models = parse_openrouter_response(body, "test");
    // Only the tool-capable model survives the filter.
    assert_eq!(models.len(), 1, "only tool-capable models are kept");
}

#[test]
fn parse_openrouter_response_bad_json_is_empty() {
    assert!(parse_openrouter_response("not json", "test").is_empty());
}

// -----------------------------------------------------------------
// parse_models_dev_response — the opencode/models.dev catalog
// -----------------------------------------------------------------

#[test]
fn parse_models_dev_keeps_only_tool_capable_non_deprecated() {
    // The catalog carries models we must not ship: no tool support (the
    // agent loop needs it) and deprecated ones (they 404 later).
    let body = r#"{"openai":{"models":{
        "good":       {"tool_call":true,  "name":"Good"},
        "no-tools":   {"tool_call":false, "name":"No Tools"},
        "unset":      {"name":"Unset"},
        "deprecated": {"tool_call":true,  "status":"deprecated", "name":"Old"}
    }}}"#;
    let models = parse_models_dev_response(body, "test");
    assert_eq!(
        models.len(),
        1,
        "only the tool-capable, non-deprecated model survives: {models:?}"
    );
}

#[test]
fn parse_models_dev_maps_npm_package_to_api_and_base_url() {
    // The npm package identifies which wire protocol the model speaks;
    // getting this wrong points a model at the wrong API shape.
    let body = r#"{"openai":{"models":{
        "resp":   {"tool_call":true, "provider":{"npm":"@ai-sdk/openai"}},
        "claude": {"tool_call":true, "provider":{"npm":"@ai-sdk/anthropic"}},
        "gemini": {"tool_call":true, "provider":{"npm":"@ai-sdk/google"}},
        "other":  {"tool_call":true, "provider":{"npm":"@ai-sdk/mystery"}}
    }}}"#;
    let models = parse_models_dev_response(body, "test");
    assert_eq!(models.len(), 4, "all four are tool-capable");

    let api_of = |id: &str| {
        models
            .iter()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("{id} missing from {models:?}"))
            .api
            .clone()
    };
    assert_eq!(api_of("resp"), "openai-responses");
    assert_eq!(api_of("claude"), "anthropic-messages");
    assert_eq!(api_of("gemini"), "google-generative-ai");
    assert_eq!(
        api_of("other"),
        "openai-completions",
        "an unknown npm package must fall back to the OpenAI-compatible API"
    );
}

#[test]
fn parse_models_dev_ignores_providers_other_than_openai() {
    // The parser only reads the `openai` key; a payload without it yields
    // nothing rather than mis-attributing models.
    let body = r#"{"anthropic":{"models":{"x":{"tool_call":true}}}}"#;
    assert!(parse_models_dev_response(body, "test").is_empty());
}

// -----------------------------------------------------------------
// apply_overrides — hand-maintained corrections to upstream data
// -----------------------------------------------------------------
//
// Upstream catalogs get some entries wrong (missing cache pricing, a
// context window reported far below the real one). These overrides patch
// them. A silently-dropped override understates cost or truncates prompts
// at a fraction of the model's real capacity.

fn model(provider: &str, id: &str) -> ModelEntry {
    entry(
        id,
        id,
        "anthropic-messages",
        provider,
        "https://example.test",
        false,
        false,
        (0.0, 0.0, 0.0, 0.0),
        4096,
        4096,
    )
}

#[test]
fn apply_overrides_sets_claude_opus_45_cache_pricing() {
    // Upstream omits cache read/write pricing for this model; without the
    // override every cached call is costed at zero.
    let mut models = vec![model("anthropic", "claude-opus-4-5")];
    apply_overrides(&mut models);
    assert!(
        (models[0].cost_cache_read - 0.5).abs() < 1e-9,
        "cache read pricing must be patched in, got {}",
        models[0].cost_cache_read
    );
    assert!(
        (models[0].cost_cache_write - 6.25).abs() < 1e-9,
        "cache write pricing must be patched in, got {}",
        models[0].cost_cache_write
    );
}

#[test]
fn apply_overrides_widens_claude_opus_46_context_window() {
    // Reported as a small window upstream; the real one is 200k. Trusting
    // the upstream value would truncate prompts at a fraction of capacity.
    for provider in ["anthropic", "opencode"] {
        let mut models = vec![model(provider, "claude-opus-4-6")];
        apply_overrides(&mut models);
        assert_eq!(
            models[0].context_window, 200_000,
            "{provider}/claude-opus-4-6 must be widened to 200k"
        );
    }
}

#[test]
fn apply_overrides_widens_opencode_sonnet_context_windows() {
    for id in ["claude-sonnet-4-5", "claude-sonnet-4"] {
        let mut models = vec![model("opencode", id)];
        apply_overrides(&mut models);
        assert_eq!(
            models[0].context_window, 200_000,
            "opencode/{id} must be widened to 200k"
        );
    }
}

#[test]
fn apply_overrides_patches_bedrock_by_substring() {
    // The bedrock id carries a region prefix, so the rule matches on a
    // substring rather than equality.
    let mut models = vec![model(
        "amazon-bedrock",
        "us.anthropic.claude-opus-4-6-v1:0",
    )];
    apply_overrides(&mut models);
    assert_eq!(models[0].context_window, 200_000);
    assert!((models[0].cost_cache_read - 0.5).abs() < 1e-9);
}

#[test]
fn apply_overrides_leaves_unrelated_models_alone() {
    // The contrast case: an over-broad rule would rewrite models it should
    // not touch, which is harder to notice than a missing override.
    let mut models = vec![model("openai", "gpt-4o")];
    apply_overrides(&mut models);
    assert_eq!(models[0].context_window, 4096, "unrelated model untouched");
    assert!((models[0].cost_cache_read - 0.0).abs() < 1e-9);
}

#[test]
fn apply_overrides_matches_on_provider_not_just_id() {
    // Same id under a provider the rule does not name must NOT be patched.
    let mut models = vec![model("some-other-vendor", "claude-opus-4-5")];
    apply_overrides(&mut models);
    assert!(
        (models[0].cost_cache_read - 0.0).abs() < 1e-9,
        "the override must be provider-scoped, not id-only"
    );
}

#[test]
fn has_finds_by_provider_and_id_together() {
    let models = vec![model("anthropic", "claude-opus-4-5")];
    assert!(has(&models, "anthropic", "claude-opus-4-5"));
    assert!(!has(&models, "openai", "claude-opus-4-5"), "provider must match");
    assert!(!has(&models, "anthropic", "other"), "id must match");
}

#[test]
fn verify_struct_shape_builds_a_descriptor() {
    // A compile-time shape guard for the generated code. Calling it proves
    // the literal still matches ModelProviderDescriptor's fields — if the
    // struct gains a field, this stops compiling, which is the point.
    let d = verify_struct_shape();
    assert_eq!(d.context_window, 0);
}

#[test]
fn parse_models_dev_bad_json_is_empty() {
    assert!(parse_models_dev_response("not json", "test").is_empty());
}

#[test]
fn parse_models_dev_empty_object_is_empty() {
    assert!(parse_models_dev_response("{}", "test").is_empty());
}

// -----------------------------------------------------------------
// parse_ai_gateway_response — the Vercel AI Gateway catalog
// -----------------------------------------------------------------

#[test]
fn parse_ai_gateway_keeps_only_tool_use_tagged() {
    let body = r#"{"data":[
        {"id":"v/with","name":"With","tags":["tool-use"],"context_window":8192},
        {"id":"v/without","name":"Without","tags":["chat"],"context_window":8192},
        {"id":"v/untagged","name":"Untagged","context_window":8192}
    ]}"#;
    let models = parse_ai_gateway_response(body, "test");
    assert_eq!(
        models.len(),
        1,
        "only the tool-use tagged model survives: {models:?}"
    );
    assert_eq!(models[0].id, "v/with");
}

#[test]
fn parse_ai_gateway_reads_reasoning_and_vision_tags() {
    let body = r#"{"data":[
        {"id":"v/full","name":"Full","tags":["tool-use","reasoning","vision"]},
        {"id":"v/plain","name":"Plain","tags":["tool-use"]}
    ]}"#;
    let models = parse_ai_gateway_response(body, "test");
    assert_eq!(models.len(), 2);

    let full = models.iter().find(|m| m.id == "v/full").expect("v/full");
    let plain = models.iter().find(|m| m.id == "v/plain").expect("v/plain");
    assert!(full.reasoning, "the reasoning tag must set the flag");
    assert!(!plain.reasoning, "absence of the tag must leave it unset");
}

#[test]
fn parse_ai_gateway_falls_back_to_id_when_name_is_absent() {
    let body = r#"{"data":[{"id":"v/anon","tags":["tool-use"]}]}"#;
    let models = parse_ai_gateway_response(body, "test");
    assert_eq!(models.len(), 1);
    assert_eq!(
        models[0].name, "v/anon",
        "a missing name must fall back to the id, never empty"
    );
}

#[test]
fn parse_ai_gateway_applies_context_and_token_defaults() {
    // Missing limits must not become 0 — a 0 context window would make the
    // model unusable rather than merely conservatively sized.
    let body = r#"{"data":[{"id":"v/bare","tags":["tool-use"]}]}"#;
    let models = parse_ai_gateway_response(body, "test");
    assert_eq!(models[0].context_window, 4096);
    assert_eq!(models[0].max_tokens, 4096);
}

#[test]
fn parse_ai_gateway_scales_pricing_to_per_million() {
    // Gateway prices are per-token; descriptors are per-million. A missing
    // multiplication here understates cost by 1e6.
    let body = r#"{"data":[{"id":"v/priced","tags":["tool-use"],
        "pricing":{"input":"0.000001","output":"0.000002"}}]}"#;
    let models = parse_ai_gateway_response(body, "test");
    assert_eq!(models.len(), 1);
    assert!(
        (models[0].cost_input - 1.0).abs() < 1e-9,
        "0.000001/token must scale to 1.0/M, got {}",
        models[0].cost_input
    );
    assert!(
        (models[0].cost_output - 2.0).abs() < 1e-9,
        "0.000002/token must scale to 2.0/M, got {}",
        models[0].cost_output
    );
}

#[test]
fn parse_ai_gateway_bad_json_is_empty() {
    assert!(parse_ai_gateway_response("not json", "test").is_empty());
}

#[test]
fn parse_ai_gateway_empty_data_is_empty() {
    assert!(parse_ai_gateway_response(r#"{"data":[]}"#, "test").is_empty());
}

#[test]
fn deduplicate_groups_by_provider_and_id() {
    let models = static_codex_models();
    let grouped = deduplicate(models);
    assert!(!grouped.is_empty(), "grouping must produce provider buckets");
    // Every bucket is non-empty.
    for (_provider, by_id) in &grouped {
        assert!(!by_id.is_empty());
    }
}

#[test]
fn models_dev_helpers() {
    // dev_cost: None → zeros; Some → the four cost fields.
    assert_eq!(dev_cost(None), (0.0, 0.0, 0.0, 0.0));
    let c = ModelsDevCost {
        input: Some(1.0),
        output: Some(2.0),
        cache_read: Some(0.5),
        cache_write: Some(3.0),
    };
    assert_eq!(dev_cost(Some(&c)), (1.0, 2.0, 0.5, 3.0));

    // ctx / max_tok fall back to the default when the limit is absent.
    assert_eq!(ctx(None, 4096), 4096);
    assert_eq!(max_tok(None, 2048), 2048);
    let limit = ModelsDevLimit {
        context: Some(128_000),
        output: Some(8192),
    };
    assert_eq!(ctx(Some(&limit), 1), 128_000);
    assert_eq!(max_tok(Some(&limit), 1), 8192);

    // pricing_val handles number, numeric string, and non-numeric.
    assert!((pricing_val(Some(&serde_json::json!(1.5))) - 1.5).abs() < 1e-9);
    assert!((pricing_val(Some(&serde_json::json!("2.5"))) - 2.5).abs() < 1e-9);
    assert_eq!(pricing_val(Some(&serde_json::json!("x"))), 0.0);
    assert_eq!(pricing_val(None), 0.0);

    // has_image: only true when "image" is among the input modalities.
    let with_image = ModelsDevModalities {
        input: Some(vec!["text".to_string(), "image".to_string()]),
    };
    assert!(has_image(Some(&with_image)));
    assert!(!has_image(None));
}

#[test]
fn from_dev_builds_entry_from_models_dev_shape() {
    let m: ModelsDevModel = serde_json::from_value(serde_json::json!({
        "name": "Test Model",
        "reasoning": true,
        "limit": {"context": 32000, "output": 4000},
        "cost": {"input": 1.0, "output": 2.0},
        "modalities": {"input": ["text", "image"]}
    }))
    .expect("models.dev model parses");

    let e = from_dev("vendor/test", &m, "anthropic-messages", "anthropic", "https://x", 1, 1);
    assert_eq!(e.id, "vendor/test");
    assert_eq!(e.name, "Test Model");
    assert!(e.reasoning);
    assert!(e.has_image_input);
    assert_eq!(e.context_window, 32000);
    assert_eq!(e.max_tokens, 4000);
    assert!((e.cost_input - 1.0).abs() < 1e-9);
}

#[test]
fn apply_overrides_adds_known_fallbacks() {
    // Starting from an empty catalog, apply_overrides must inject the
    // hard-coded fallback entries (e.g. Claude Opus 4.6).
    let mut models: Vec<ModelEntry> = Vec::new();
    apply_overrides(&mut models);
    assert!(has(&models, "anthropic", "claude-opus-4-6"), "fallback injected");
    assert!(!models.is_empty());

    // The `has()`-guarded fallback push is not duplicated on a second pass
    // (apply_overrides runs once in the real pipeline; only the guarded
    // pushes are idempotent).
    apply_overrides(&mut models);
    let opus_46 = models
        .iter()
        .filter(|m| m.provider == "anthropic" && m.id == "claude-opus-4-6")
        .count();
    assert_eq!(opus_46, 1, "guarded fallback is not duplicated");
}

#[test]
fn generate_provider_file_emits_descriptor_rust() {
    let mut by_id: BTreeMap<String, ModelEntry> = BTreeMap::new();
    by_id.insert(
        "m1".to_string(),
        entry(
            "m1",
            "Model One",
            "anthropic-messages",
            "anthropic",
            "https://api.anthropic.com",
            true,
            true,
            (1.0, 2.0, 0.0, 0.0),
            200_000,
            8192,
        ),
    );
    let src = generate_provider_file("anthropic", &by_id);
    assert!(src.contains("pub static MODELS"), "emits the MODELS slice");
    assert!(src.contains("ModelProviderDescriptor"));
    assert!(src.contains("\"m1\""), "includes the model id");
    assert!(src.contains("MessageType::TextAndImages"), "image input rendered");
    assert!(src.contains("ModelAPI::AnthropicMessages"));
}

#[test]
fn generate_providers_mod_lists_each_provider() {
    let mut providers: BTreeMap<String, BTreeMap<String, ModelEntry>> = BTreeMap::new();
    let mut anthropic: BTreeMap<String, ModelEntry> = BTreeMap::new();
    anthropic.insert(
        "m1".to_string(),
        entry("m1", "M1", "anthropic-messages", "anthropic", "", false, false,
              (0.0, 0.0, 0.0, 0.0), 1000, 500),
    );
    providers.insert("anthropic".to_string(), anthropic);

    let src = generate_providers_mod(&providers);
    assert!(src.contains("anthropic"), "references the provider module");
}
