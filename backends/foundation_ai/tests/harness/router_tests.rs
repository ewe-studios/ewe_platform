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

use foundation_ai::agentic::AgenticError;
use foundation_ai::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig;
use foundation_ai::harness::{
    self, CloudPresets, RouterMix, RouterPreset, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O,
    OPENAI_GPT4O_MINI,
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
    let preset = RouterPreset::claude("test-key").expect("claude router builds offline");

    assert_eq!(preset.primary_model, Some(named(CLAUDE_OPUS)));
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
    let preset = RouterPreset::claude("test-key").expect("claude router builds offline");

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
    let chat = RouterPreset::openai_chat("test-key").expect("chat router builds");
    let responses = RouterPreset::openai_responses("test-key").expect("responses router builds");

    assert_eq!(chat.primary_model, Some(named(OPENAI_GPT4O)));
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

    assert_eq!(preset.primary_model, Some(named(CLAUDE_OPUS)));
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
    let preset =
        RouterPreset::glm52_gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
            .expect("gguf router builds offline (no download)");

    assert_eq!(preset.primary_model, Some(named(harness::Glm52::MODEL_ID)));
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

// ---------------------------------------------------------------------------
// Preset builders (offline) — glm52 / qwen36 / gemma / candle_llama presets and
// their session bridges construct without any download. Coverage for
// harness/agents.rs (was ~33%).

use foundation_ai::harness::{Gemma4E2b, Gemma4_26b, Glm52, Qwen36};

#[test]
fn glm52_preset_wires_primary_and_memory() {
    let preset =
        RouterPreset::glm52_gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
            .expect("glm52 preset builds offline");
    assert_eq!(preset.primary_model, Some(named(Glm52::MODEL_ID)));
    assert_eq!(preset.memory_model, Some(named(Gemma4E2b::MODEL_ID)));
}

#[test]
fn qwen36_preset_wires_primary_and_memory() {
    let preset =
        RouterPreset::qwen36_gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
            .expect("qwen36 preset builds offline");
    assert_eq!(preset.primary_model, Some(named(Qwen36::MODEL_ID)));
    assert_eq!(preset.memory_model, Some(named(Gemma4E2b::MODEL_ID)));
}

#[test]
fn gemma_preset_wires_primary_and_memory() {
    let preset = RouterPreset::gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
        .expect("gemma preset builds offline");
    assert_eq!(preset.primary_model, Some(named(Gemma4_26b::MODEL_ID)));
    assert_eq!(preset.memory_model, Some(named(Gemma4E2b::MODEL_ID)));
}

#[test]
fn candle_llama_preset_has_primary_and_no_memory() {
    let preset = RouterPreset::candle_llama("HuggingFaceTB/SmolLM2-135M", None)
        .expect("candle preset builds offline");
    assert_eq!(
        preset.primary_model,
        Some(named("HuggingFaceTB/SmolLM2-135M"))
    );
    assert!(
        preset.memory_model.is_none(),
        "a single-model candle preset has no memory model"
    );
}

#[test]
fn gemma_preset_rejects_unknown_model() {
    let preset = RouterPreset::gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
        .expect("builds");
    // An unknown model must not resolve to any provider. (BoxModel is not Debug,
    // so match on the Result rather than formatting it.)
    assert!(
        preset.router.get_model(&named("nobody/serves-this")).is_err(),
        "an unknown model must not resolve"
    );
}

#[test]
fn preset_session_bridge_builds_offline() {
    use foundation_ai::agentic::{AgentSession, KvMemoryStore};
    use foundation_ai::types::SessionId;
    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    // `from_name` is only deterministic within the same millisecond (it embeds a
    // time-ordered timestamp — see SessionId docs), so mint the id ONCE and reuse
    // it on both sides. Comparing two independent `from_name` calls is racy.
    let sid = SessionId::from_name("harness-test");
    let builder = RouterPreset::gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
        .expect("preset builds offline")
        .into_agent_builder()
        .with_session_id(sid.clone());
    let session: AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>> =
        builder.build().expect("session builds");
    assert_eq!(*session.session_id(), sid);
}

// ---------------------------------------------------------------------------
// Every preset constructor bridges into an AgentSession offline, with the
// preset's models wired.

#[test]
fn all_presets_build_sessions_offline() {
    use foundation_ai::agentic::{AgentSession, ModelSelection};

    fn session(preset: RouterPreset) -> AgentSession {
        preset.into_agent_builder().build().expect("session builds")
    }

    // GGUF-backed presets (throwaway configs → no download at construction).
    let glm = session(
        RouterPreset::glm52_gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
            .expect("glm52_gemma builds"),
    );
    assert_eq!(
        *glm.models(),
        ModelSelection::new(named(Glm52::MODEL_ID)).with_memory(named(Gemma4E2b::MODEL_ID))
    );
    let qwen = session(
        RouterPreset::qwen36_gemma(Some(throwaway_gguf_config()), Some(throwaway_gguf_config()))
            .expect("qwen36_gemma builds"),
    );
    assert_eq!(qwen.models().primary, named(Qwen36::MODEL_ID));

    // Cloud presets (dummy key; no network at construction).
    let claude = session(RouterPreset::claude("test-key").expect("claude builds"));
    assert_eq!(
        *claude.models(),
        ModelSelection::new(CLAUDE_OPUS).with_memory(CLAUDE_SONNET)
    );
    let chat = session(RouterPreset::openai_chat("test-key").expect("openai_chat builds"));
    assert_eq!(chat.models().primary, named(OPENAI_GPT4O));
    let responses =
        session(RouterPreset::openai_responses("test-key").expect("openai_responses builds"));
    assert_eq!(responses.models().memory, Some(named(OPENAI_GPT4O_MINI)));
}

#[cfg(feature = "candle")]
#[test]
fn candle_llama_preset_builds_a_session_offline() {
    let session = RouterPreset::candle_llama("HuggingFaceTB/SmolLM2-135M", None)
        .expect("candle_llama builds")
        .into_agent_builder()
        .build()
        .expect("session builds");
    assert_eq!(
        session.models().primary,
        named("HuggingFaceTB/SmolLM2-135M")
    );
    assert_eq!(session.models().memory, None);
}

#[test]
fn a_mix_without_a_primary_model_does_not_build_a_session() {
    use foundation_ai::agentic::AgenticError;

    let preset = RouterMix::new()
        .memory(
            CloudPresets::claude_sonnet("test-key").expect("provider"),
            CLAUDE_SONNET,
        )
        .build();
    assert_eq!(preset.primary_model, None);
    let err = preset
        .into_agent_builder()
        .build()
        .err()
        .expect("no primary model, no session");
    assert_eq!(
        *err.current_context(),
        AgenticError::Session("no model set".into())
    );
}

// ---------------------------------------------------------------------------
// Every quantization constructor of every GGUF preset builds offline.
// Covers harness/providers.rs per-preset q3/q4/q5/q8 methods (was ~40%).

use foundation_ai::harness::{Gemma4E4b, Ornith10};

#[test]
fn all_preset_quantizations_build_offline() {
    fn cfg() -> HuggingFaceGGUFConfig {
        throwaway_gguf_config()
    }

    // Each preset × each quantization must construct a provider.
    macro_rules! check {
        ($p:ty) => {{
            assert!(<$p>::q3_k_m(Some(cfg())).is_ok(), "{} q3_k_m", stringify!($p));
            assert!(<$p>::q4_k_m(Some(cfg())).is_ok(), "{} q4_k_m", stringify!($p));
            assert!(<$p>::q5_k_m(Some(cfg())).is_ok(), "{} q5_k_m", stringify!($p));
            assert!(<$p>::q8_0(Some(cfg())).is_ok(), "{} q8_0", stringify!($p));
        }};
    }

    check!(Glm52);
    check!(Qwen36);
    check!(Ornith10);
    check!(Gemma4E4b);
    check!(Gemma4_26b);
    check!(Gemma4E2b);
}

#[test]
fn preset_model_ids_are_distinct_and_nonempty() {
    let ids = [
        Glm52::MODEL_ID,
        Qwen36::MODEL_ID,
        Ornith10::MODEL_ID,
        Gemma4E4b::MODEL_ID,
        Gemma4_26b::MODEL_ID,
        Gemma4E2b::MODEL_ID,
    ];
    for id in ids {
        assert!(!id.is_empty(), "a preset MODEL_ID must not be empty");
    }
    // All distinct.
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "preset MODEL_IDs must be distinct");
}

// ---------------------------------------------------------------------------
// Construction failures — one error type at the boundary (doc 15 item 15)

/// A GGUF config whose cache dir can't be created: its parent is a file.
fn unusable_gguf_config(tag: &str) -> HuggingFaceGGUFConfig {
    let blocker =
        std::env::temp_dir().join(format!("ewe-harness-blocker-{tag}-{}", std::process::id()));
    std::fs::write(&blocker, b"not a directory").expect("write blocker file");
    HuggingFaceGGUFConfig::builder()
        .cache_dir(blocker.join("cache"))
        .default_quantization("Q4_K_M")
        .build()
}

#[test]
fn provider_construction_failure_is_an_agentic_provider_error() {
    let err = Gemma4E2b::q4_k_m(Some(unusable_gguf_config("provider")))
        .err()
        .expect("a cache dir under a file cannot be created");
    match err.current_context() {
        AgenticError::Provider(msg) => assert!(
            msg.contains("cache directory"),
            "the message names the cause: {msg}"
        ),
        other => panic!("expected AgenticError::Provider, got {other:?}"),
    }
}

#[test]
fn preset_router_failure_propagates_the_provider_error() {
    let err = RouterPreset::gemma(Some(unusable_gguf_config("router")), None)
        .err()
        .expect("the main provider cannot be constructed");
    assert!(
        matches!(err.current_context(), AgenticError::Provider(_)),
        "got {:?}",
        err.current_context()
    );
}

#[test]
fn provider_error_displays_its_message() {
    let err = AgenticError::Provider("no key".into());
    assert_eq!(err.to_string(), "provider: no key");
}
