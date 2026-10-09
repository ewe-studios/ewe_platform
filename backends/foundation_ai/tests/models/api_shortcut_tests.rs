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
    assert_eq!(text_and_role(&from_str), ("hello".into(), MessageRole::User));
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
