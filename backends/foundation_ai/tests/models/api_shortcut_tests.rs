//! Proposal 15, Tier 2 shortcuts: message constructors, string model ids,
//! router conversions and API-key config constructors. All offline.

use foundation_ai::types::{MessageRole, Messages, TextContent, UserModelContent};

fn text_and_role(message: &Messages) -> (String, MessageRole) {
    match message {
        Messages::User {
            role,
            content: UserModelContent::Text(TextContent { content, .. }),
            signature: None,
            ..
        } => (content.clone(), role.clone()),
        other => panic!("expected a text user-variant message, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 6 — message constructors
// ---------------------------------------------------------------------------

#[test]
fn message_constructors_set_the_role_and_text() {
    assert_eq!(
        text_and_role(&Messages::user("hi")),
        ("hi".into(), MessageRole::User)
    );
    assert_eq!(
        text_and_role(&Messages::system("be brief")),
        ("be brief".into(), MessageRole::System)
    );
    assert_eq!(
        text_and_role(&Messages::agent("stop and summarise")),
        ("stop and summarise".into(), MessageRole::Agent)
    );
}

#[test]
fn strings_convert_into_user_messages() {
    let from_str: Messages = "hello".into();
    let from_string: Messages = String::from("hello").into();
    assert_eq!(
        text_and_role(&from_str),
        ("hello".into(), MessageRole::User)
    );
    assert_eq!(
        text_and_role(&from_string),
        ("hello".into(), MessageRole::User)
    );
}

#[test]
fn each_constructed_message_gets_a_fresh_id() {
    assert_ne!(Messages::user("a").id(), Messages::user("a").id());
}

// ---------------------------------------------------------------------------
// Item 9 — model ids from strings
// ---------------------------------------------------------------------------

#[test]
fn strings_convert_into_named_model_ids() {
    use foundation_ai::types::ModelId;

    let from_str: ModelId = "claude-sonnet-4-6".into();
    let from_string: ModelId = String::from("gpt-4o").into();
    assert_eq!(from_str, ModelId::Name("claude-sonnet-4-6".into(), None));
    assert_eq!(from_string, ModelId::Name("gpt-4o".into(), None));
}

// ---------------------------------------------------------------------------
// Item 10 — router conversions
// ---------------------------------------------------------------------------

fn anthropic() -> foundation_ai::backends::anthropic_messages_provider::AnthropicMessagesProvider {
    use foundation_ai::backends::anthropic_messages_provider::{
        AnthropicConfig, AnthropicMessagesProvider,
    };
    use foundation_auth::{AuthCredential, ConfidentialText};
    AnthropicMessagesProvider::with_config(AnthropicConfig::new().with_auth(
        AuthCredential::SecretOnly(ConfidentialText::new("k".into())),
    ))
}

fn openai() -> foundation_ai::backends::openai_provider::OpenAIProvider {
    use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
    use foundation_auth::{AuthCredential, ConfidentialText};
    OpenAIProvider::with_config(OpenAIConfig::new().with_auth(AuthCredential::SecretOnly(
        ConfidentialText::new("k".into()),
    )))
}

#[test]
fn a_provider_converts_into_a_single_provider_router() {
    use foundation_ai::types::{ModelId, ProviderRouter};

    let router: ProviderRouter = anthropic().into();
    let resolved = router
        .resolve(&ModelId::from("claude-sonnet-4-6"))
        .expect("the single provider serves the model");
    assert_eq!(resolved.name(), anthropic_name());
}

fn anthropic_name() -> String {
    use foundation_ai::types::ModelProvider;
    anthropic()
        .describe()
        .expect("anthropic describes itself")
        .name
        .to_string()
}

#[test]
fn router_builder_takes_providers_directly() {
    use foundation_ai::types::{ProviderRouter, RoutingRule};

    let router = ProviderRouter::builder()
        .provider(anthropic())
        .provider(openai())
        .rule(RoutingRule {
            model: "gpt-4o".into(),
            provider_name: openai_name(),
        })
        .build();
    let resolved = router.resolve(&"gpt-4o".into()).expect("gpt-4o routes");
    assert_eq!(resolved.name(), openai_name());
}

fn openai_name() -> String {
    use foundation_ai::types::ModelProvider;
    openai()
        .describe()
        .expect("openai describes itself")
        .name
        .to_string()
}

#[test]
fn agent_session_builder_takes_a_provider() {
    use foundation_ai::agentic::AgentSession;

    let session = AgentSession::builder(anthropic())
        .with_model("claude-sonnet-4-6")
        .build()
        .expect("a session over a bare provider builds");
    assert!(session
        .router()
        .resolve(&"claude-sonnet-4-6".into())
        .is_ok());
}
