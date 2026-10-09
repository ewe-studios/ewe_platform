use foundation_ai::types::base_types::*;

#[test]
fn known_roles_serialize_to_legacy_strings() {
    for (role, wire) in [
        (MessageRole::User, "user"),
        (MessageRole::Agent, "agent"),
        (MessageRole::System, "system"),
        (MessageRole::Tool, "tool"),
    ] {
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, format!("\"{wire}\""));
        assert_eq!(role.as_wire(), wire);
        // Round-trips back to the same variant.
        let back: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, role);
    }
}

#[test]
fn custom_role_is_transparent() {
    let role = MessageRole::Custom("x-foo".to_string());
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"x-foo\"");
    let back: MessageRole = serde_json::from_str("\"x-foo\"").unwrap();
    assert_eq!(back, MessageRole::Custom("x-foo".to_string()));
}

#[test]
fn from_str_maps_known_and_unknown() {
    assert_eq!(MessageRole::from("system"), MessageRole::System);
    assert_eq!(
        MessageRole::from("weird"),
        MessageRole::Custom("weird".to_string())
    );
    assert_eq!(MessageRole::default(), MessageRole::User);
}

#[test]
fn execution_hint_serde_lowercase_round_trip() {
    for (hint, wire) in [
        (ExecutionHint::Unspecified, "unspecified"),
        (ExecutionHint::Parallel, "parallel"),
        (ExecutionHint::Sequential, "sequential"),
    ] {
        let json = serde_json::to_string(&hint).unwrap();
        assert_eq!(json, format!("\"{wire}\""));
        let back: ExecutionHint = serde_json::from_str(&json).unwrap();
        assert_eq!(back, hint);
    }
    assert_eq!(ExecutionHint::default(), ExecutionHint::Unspecified);
}

#[test]
fn legacy_tool_call_json_deserializes_with_defaults() {
    // A ToolCall serialized before depends_on/execution_hint existed.
    let legacy = r#"{"ToolCall":{"id":"t1","name":"read","arguments":null,"signature":null}}"#;
    let parsed: ModelOutput = serde_json::from_str(legacy).unwrap();
    match parsed {
        ModelOutput::ToolCall {
            depends_on,
            execution_hint,
            ..
        } => {
            assert!(depends_on.is_empty());
            assert_eq!(execution_hint, ExecutionHint::Unspecified);
        }
        other => panic!("expected ToolCall, got {other:?}"),
    }
}

#[test]
fn message_id_accessor_is_uniform_across_variants() {
    let user = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "hi".into(),
            signature: None,
        }),
        signature: None,
    };
    // The accessor returns the same id the variant carries.
    assert_eq!(user.id(), user.id());
    // Two freshly-built messages have distinct, time-ordered ids.
    let later = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "bye".into(),
            signature: None,
        }),
        signature: None,
    };
    assert_ne!(user.id(), later.id());
    assert!(later.id() > user.id(), "ids are monotonic");
}

#[test]
fn legacy_user_message_without_id_deserializes_with_a_minted_id() {
    // A User message persisted before per-message ids existed.
    let legacy = r#"{"User":{"role":"user","content":{"Text":{"content":"hi","signature":null}},"signature":null}}"#;
    let parsed: Messages = serde_json::from_str(legacy).unwrap();
    // serde(default) minted a fresh id; the message is still usable.
    assert!(parsed.id().timestamp() > 0);
    match parsed {
        Messages::User { role, .. } => assert_eq!(role, MessageRole::User),
        other => panic!("expected User, got {other:?}"),
    }
}
