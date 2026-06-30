//! Harness router/agent-preset tests.
//!
//! WHY: The harness presets exist to make multi-model routing "just work".
//! Their correctness hinges on one non-obvious property: the underlying
//! providers have inconsistent `serves()` behaviour — a local GGUF provider
//! claims *nothing* (`serves() == false`) while the Anthropic provider claims
//! *everything* (`serves() == true`). So the presets must pin each model to its
//! provider with explicit routing rules + distinct provider identities, and the
//! resulting router must resolve each model to the right provider regardless of
//! probe order.
//!
//! WHAT: offline tests (no model download, no network) that build the real
//! presets and assert the routing table, provider identities, and the
//! primary/memory/fallback model wiring — plus the unknown-model error path.
//!
//! HOW: cloud presets construct without any network call, so we build them
//! directly and inspect `RouterPreset`. GGUF presets construct a HuggingFace
//! client + cache dir but download nothing until `get_model`, so we point them
//! at a throwaway cache dir and exercise routing only.

use foundation_ai::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig;
use foundation_ai::harness::{
    self, CloudPresets, RouterMix, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI,
};
use foundation_ai::types::{ModelId, ModelProviders, RouterError};

fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

/// Cache dir that never holds a real download — keeps GGUF construction off the
/// user's real model cache.
fn throwaway_gguf_config() -> HuggingFaceGGUFConfig {
    let dir = std::env::temp_dir().join(format!("ewe-harness-test-{}", std::process::id()));
    HuggingFaceGGUFConfig::builder()
        .cache_dir(dir)
        .default_quantization("Q4_K_M")
        .build()
}

// ---------------------------------------------------------------------------
// Cloud presets — primary/memory wiring + per-model routing

#[test]
fn claude_router_wires_primary_and_memory() {
    let preset = harness::claude_router("test-key").expect("claude router builds offline");

    assert_eq!(preset.primary_model, named(CLAUDE_OPUS));
    assert_eq!(preset.memory_model, Some(named(CLAUDE_SONNET)));
    assert!(preset.fallback_models.is_empty());
    assert_eq!(preset.router.provider_count(), 2);

    let main = preset
        .router
        .resolve(&named(CLAUDE_OPUS))
        .expect("opus resolves");
    assert_eq!(main.name(), CLAUDE_OPUS);
    assert_eq!(main.provider_id(), ModelProviders::ANTHROPIC);

    let memory = preset
        .router
        .resolve(&named(CLAUDE_SONNET))
        .expect("sonnet resolves");
    assert_eq!(memory.name(), CLAUDE_SONNET);
}

#[test]
fn explicit_rules_beat_greedy_anthropic_serves() {
    // Anthropic's `serves()` returns true for EVERY id. Without explicit rules,
    // a probe would route the memory model to the first provider (the opus
    // instance). The preset's rules must override that so the memory id lands on
    // its own provider.
    let preset = harness::claude_router("test-key").expect("claude router builds offline");

    let memory = preset
        .router
        .resolve(&named(CLAUDE_SONNET))
        .expect("sonnet resolves");
    assert_eq!(
        memory.name(),
        CLAUDE_SONNET,
        "memory model must resolve to its own provider, not the greedy first one"
    );
}

#[test]
fn openai_chat_and_responses_have_distinct_provider_ids() {
    let chat = harness::openai_chat_router("test-key").expect("chat router builds");
    let responses = harness::openai_responses_router("test-key").expect("responses router builds");

    assert_eq!(chat.primary_model, named(OPENAI_GPT4O));
    assert_eq!(chat.memory_model, Some(named(OPENAI_GPT4O_MINI)));

    let chat_main = chat.router.resolve(&named(OPENAI_GPT4O)).expect("resolves");
    assert_eq!(chat_main.provider_id(), ModelProviders::OPENAI);

    let resp_main = responses
        .router
        .resolve(&named(OPENAI_GPT4O))
        .expect("resolves");
    assert_eq!(resp_main.provider_id(), ModelProviders::OPENAIRESPONSES);
}

// ---------------------------------------------------------------------------
// RouterMix — fully custom mixing across roles

#[test]
fn router_mix_composes_primary_memory_and_fallback() {
    let preset = RouterMix::new()
        .primary(
            CloudPresets::claude_opus("k").expect("opus provider"),
            named(CLAUDE_OPUS),
        )
        .memory(
            CloudPresets::claude_sonnet("k").expect("sonnet provider"),
            named(CLAUDE_SONNET),
        )
        .fallback(
            CloudPresets::openai_gpt4o("k").expect("gpt4o provider"),
            named(OPENAI_GPT4O),
        )
        .build();

    assert_eq!(preset.primary_model, named(CLAUDE_OPUS));
    assert_eq!(preset.memory_model, Some(named(CLAUDE_SONNET)));
    assert_eq!(preset.fallback_models, vec![named(OPENAI_GPT4O)]);
    assert_eq!(preset.router.provider_count(), 3);

    // The OpenAI fallback must keep its own identity even though an Anthropic
    // provider (greedy serves) is registered ahead of it.
    let fallback = preset
        .router
        .resolve(&named(OPENAI_GPT4O))
        .expect("fallback resolves");
    assert_eq!(fallback.provider_id(), ModelProviders::OPENAI);
}

// ---------------------------------------------------------------------------
// GGUF presets — offline routing + unknown-model error path

#[test]
fn gguf_router_routes_main_and_memory_and_rejects_unknown() {
    let preset = harness::glm52_gemma_router(
        Some(throwaway_gguf_config()),
        Some(throwaway_gguf_config()),
    )
    .expect("gguf router builds offline (no download)");

    assert_eq!(preset.primary_model, named(harness::Glm52::MODEL_ID));
    assert_eq!(
        preset.memory_model,
        Some(named(harness::Gemma4E2b::MODEL_ID))
    );
    assert_eq!(preset.router.provider_count(), 2);

    // Valid: each registered model resolves to its own provider via the rules
    // (GGUF `serves()` is always false, so rules are the only thing that works).
    let main = preset
        .router
        .resolve(&named(harness::Glm52::MODEL_ID))
        .expect("main model resolves");
    assert_eq!(main.name(), harness::Glm52::MODEL_ID);

    let memory = preset
        .router
        .resolve(&named(harness::Gemma4E2b::MODEL_ID))
        .expect("memory model resolves");
    assert_eq!(memory.name(), harness::Gemma4E2b::MODEL_ID);

    // Invalid: an unregistered model has no rule and no provider that serves it.
    // (`resolve` returns `&dyn RoutableProvider`, which isn't `Debug`, so match
    // rather than `expect_err`.)
    let unknown = preset.router.resolve(&named("nonexistent/repo"));
    assert!(
        matches!(unknown, Err(RouterError::NoProviderForModel(_))),
        "unknown model must not resolve"
    );
}
