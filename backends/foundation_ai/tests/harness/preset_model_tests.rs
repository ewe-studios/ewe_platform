//! A preset carries its models into the session.
//!
//! WHY: `AgentSession::builder(preset)` (and `preset.into_agent_builder()`)
//! should need no `with_model`: the preset already names its primary, memory
//! and fallback models. `with_model` and friends stay as overrides, and a
//! session with no model anywhere still fails to build.
//!
//! HOW: cloud presets and `RouterMix` over Anthropic providers construct
//! offline, so every check here builds a real session without network.

use foundation_ai::agentic::{AgentSession, AgenticError};
use foundation_ai::backends::anthropic_messages_provider::AnthropicMessagesProvider;
use foundation_ai::harness::{RouterMix, RouterPreset, CLAUDE_OPUS, CLAUDE_SONNET};
use foundation_ai::types::{ModelId, ProviderRouter};

fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

fn assert_no_model_set(err: &foundation_errstacks::ErrorTrace<AgenticError>) {
    assert_eq!(
        *err.current_context(),
        AgenticError::Session("no model set".into())
    );
}

#[test]
fn preset_models_reach_the_session_without_with_model() {
    let preset = RouterPreset::claude("test-key").expect("preset builds");

    let session = AgentSession::builder(preset)
        .build()
        .expect("the preset's primary model is enough to build");

    let models = session.models();
    assert_eq!(models.primary, named(CLAUDE_OPUS));
    assert_eq!(models.memory, Some(named(CLAUDE_SONNET)));
    assert_eq!(models.fallbacks, Vec::<ModelId>::new());
}

#[test]
fn into_agent_builder_carries_the_same_models_as_builder() {
    let session = RouterPreset::claude("test-key")
        .expect("preset builds")
        .into_agent_builder()
        .build()
        .expect("session builds");

    assert_eq!(session.models().primary, named(CLAUDE_OPUS));
    assert_eq!(session.models().memory, Some(named(CLAUDE_SONNET)));
}

#[test]
fn preset_fallback_models_reach_the_session() {
    let preset = RouterMix::new()
        .primary(AnthropicMessagesProvider::api_key("k"), "main-model")
        .fallback(AnthropicMessagesProvider::api_key("k"), "fallback-a")
        .fallback(AnthropicMessagesProvider::api_key("k"), "fallback-b")
        .build();

    let session = AgentSession::builder(preset)
        .build()
        .expect("session builds");

    assert_eq!(session.models().primary, named("main-model"));
    assert_eq!(
        session.models().fallbacks,
        vec![named("fallback-a"), named("fallback-b")]
    );
    assert_eq!(session.models().memory, None);
}

#[test]
fn with_model_overrides_the_preset_models() {
    let preset = RouterPreset::claude("test-key").expect("preset builds");

    let session = AgentSession::builder(preset)
        .with_model(CLAUDE_SONNET)
        .with_memory_model("memory-override")
        .with_fallback_models(["fallback-override"])
        .build()
        .expect("session builds");

    assert_eq!(session.models().primary, named(CLAUDE_SONNET));
    assert_eq!(session.models().memory, Some(named("memory-override")));
    assert_eq!(session.models().fallbacks, vec![named("fallback-override")]);
}

#[test]
fn build_fails_when_neither_preset_nor_builder_sets_a_model() {
    // A mix with no primary carries no primary model into the session.
    let preset = RouterMix::new()
        .memory(AnthropicMessagesProvider::api_key("k"), "memory-only")
        .build();
    assert_eq!(preset.primary_model, None);

    let err = AgentSession::builder(preset)
        .build()
        .err()
        .expect("no primary model anywhere must not build");
    assert_no_model_set(&err);
}

#[test]
fn bare_router_and_provider_carry_no_model() {
    let err = AgentSession::builder(ProviderRouter::builder().build())
        .build()
        .err()
        .expect("a bare router sets no model");
    assert_no_model_set(&err);

    let err = AgentSession::builder(AnthropicMessagesProvider::api_key("k"))
        .build()
        .err()
        .expect("a bare provider sets no model");
    assert_no_model_set(&err);

    // `with_model` completes either one.
    let session = AgentSession::builder(AnthropicMessagesProvider::api_key("k"))
        .with_model(CLAUDE_OPUS)
        .build()
        .expect("with_model supplies the missing model");
    assert_eq!(session.models().primary, named(CLAUDE_OPUS));
}
