//! Coverage for the small pure helpers in `types::base_types`.
//!
//! WHY: `base_types.rs` is the widest-used module in the crate (every public
//! API touches it) but sat at 69% lines. The gaps were not exotic — they were
//! exhaustive `match` helpers and trait impls that nothing happened to call:
//! `Quantization::to_filename_format`, the `MessageRole` conversions,
//! `KVCacheType::bytes_per_element`, the `ChatMessage` constructors, `DeviceId`,
//! and `Args`'s hand-written `PartialEq`/`Serialize`/`Deserialize`. A wrong arm
//! in any of them is a silent data bug (e.g. a quantization resolving to the
//! wrong GGUF filename, so the wrong weights get downloaded).
//!
//! WHAT: exhaustive-by-construction tests over each helper.
//!
//! HOW: pure construction and formatting — no providers, no models, no network.

use foundation_ai::types::{
    Args, ChatMessage, DeviceId, KVCacheType, MessageRole, Quantization,
};

// ---------------------------------------------------------------------------
// DeviceId
// ---------------------------------------------------------------------------

#[test]
fn device_id_round_trips() {
    let d = DeviceId::new(7);
    assert_eq!(d.get_id(), 7);
}

#[test]
fn device_id_preserves_boundary_values() {
    assert_eq!(DeviceId::new(0).get_id(), 0);
    assert_eq!(DeviceId::new(u16::MAX).get_id(), u16::MAX);
}

// ---------------------------------------------------------------------------
// Quantization::to_filename_format
// ---------------------------------------------------------------------------

#[test]
fn quantization_none_and_default_render_empty() {
    // These two mean "no quantization suffix in the filename" — they must be
    // empty, not the literal "None"/"Default", or file lookup breaks.
    assert_eq!(Quantization::None.to_filename_format(), "");
    assert_eq!(Quantization::Default.to_filename_format(), "");
}

#[test]
fn quantization_k_quants_render_gguf_filename_spellings() {
    // The spellings that differ from the variant name are the ones most likely
    // to regress, so pin each explicitly.
    assert_eq!(Quantization::Q4_KM.to_filename_format(), "Q4_K_M");
    assert_eq!(Quantization::Q5_KM.to_filename_format(), "Q5_K_M");
    assert_eq!(Quantization::Q6_KM.to_filename_format(), "Q6_K_M");
    assert_eq!(Quantization::Q2K.to_filename_format(), "Q2_K");
    assert_eq!(Quantization::Q6_K.to_filename_format(), "Q6_K");
}

#[test]
fn quantization_iq_variants_render_iq_spellings() {
    assert_eq!(Quantization::IQ_4Nl.to_filename_format(), "IQ4_NL");
    assert_eq!(Quantization::IQ_4Xs.to_filename_format(), "IQ4_XS");
    assert_eq!(Quantization::Ud_IQ_1M.to_filename_format(), "IQ1_M");
    assert_eq!(Quantization::UD_IQ_1S.to_filename_format(), "IQ1_S");
    assert_eq!(Quantization::UD_IQ_2M.to_filename_format(), "IQ2_M");
    assert_eq!(Quantization::UD_IQ_2Xxs.to_filename_format(), "IQ2_XXS");
    assert_eq!(Quantization::UD_IQ_3Xxs.to_filename_format(), "IQ3_XXS");
}

#[test]
fn quantization_unsloth_xl_variants_render_xl_spellings() {
    assert_eq!(Quantization::UD_Q_2KXl.to_filename_format(), "Q2_K_XL");
    assert_eq!(Quantization::UD_Q_3KXl.to_filename_format(), "Q3_K_XL");
    assert_eq!(Quantization::UD_Q_4KXl.to_filename_format(), "Q4_K_XL");
    assert_eq!(Quantization::UD_Q_5KXl.to_filename_format(), "Q5_K_XL");
    assert_eq!(Quantization::UD_Q_6KXl.to_filename_format(), "Q6_K_XL");
    assert_eq!(Quantization::UD_Q_8KXl.to_filename_format(), "Q8_K_XL");
}

#[test]
fn quantization_simple_variants_render_their_own_names() {
    assert_eq!(Quantization::F16.to_filename_format(), "F16");
    assert_eq!(Quantization::Q2_KS.to_filename_format(), "Q2_KS");
    assert_eq!(Quantization::Q2_KM.to_filename_format(), "Q2_KM");
    assert_eq!(Quantization::Q2_KL.to_filename_format(), "Q2_KL");
    assert_eq!(Quantization::Q3_KS.to_filename_format(), "Q3_KS");
    assert_eq!(Quantization::Q3_KM.to_filename_format(), "Q3_KM");
    assert_eq!(Quantization::Q4_0.to_filename_format(), "Q4_0");
    assert_eq!(Quantization::Q4_1.to_filename_format(), "Q4_1");
    assert_eq!(Quantization::Q4_KS.to_filename_format(), "Q4_KS");
    assert_eq!(Quantization::Q5_KS.to_filename_format(), "Q5_KS");
    assert_eq!(Quantization::Q5_KL.to_filename_format(), "Q5_KL");
    assert_eq!(Quantization::Q6_KS.to_filename_format(), "Q6_KS");
    assert_eq!(Quantization::Q6_KL.to_filename_format(), "Q6_KL");
    assert_eq!(Quantization::Q8_0.to_filename_format(), "Q8_0");
    assert_eq!(Quantization::Q8_1.to_filename_format(), "Q8_1");
}

#[test]
fn quantization_custom_passes_the_string_through() {
    assert_eq!(
        Quantization::Custom("MXFP4_MOE".into()).to_filename_format(),
        "MXFP4_MOE"
    );
}

#[test]
fn quantization_filename_formats_are_unique_where_meaningful() {
    // Two distinct quantizations rendering to the same filename would silently
    // download the wrong weights. Empty (None/Default) is the allowed dupe.
    let all = [
        Quantization::F16,
        Quantization::Q2K,
        Quantization::Q4_0,
        Quantization::Q4_1,
        Quantization::Q4_KM,
        Quantization::Q4_KS,
        Quantization::Q5_KM,
        Quantization::Q5_KS,
        Quantization::Q6_K,
        Quantization::Q8_0,
        Quantization::IQ_4Nl,
        Quantization::IQ_4Xs,
    ];
    let mut seen = std::collections::HashSet::new();
    for q in all {
        let f = q.to_filename_format();
        assert!(
            seen.insert(f.clone()),
            "duplicate filename format {f:?} for {q:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// MessageRole
// ---------------------------------------------------------------------------

#[test]
fn message_role_from_string_maps_known_roles() {
    assert_eq!(MessageRole::from("user".to_string()), MessageRole::User);
    assert_eq!(MessageRole::from("agent".to_string()), MessageRole::Agent);
    assert_eq!(MessageRole::from("system".to_string()), MessageRole::System);
    assert_eq!(MessageRole::from("tool".to_string()), MessageRole::Tool);
}

#[test]
fn message_role_from_string_falls_back_to_custom() {
    assert_eq!(
        MessageRole::from("moderator".to_string()),
        MessageRole::Custom("moderator".to_string())
    );
}

#[test]
fn message_role_from_string_is_case_sensitive() {
    // The wire protocol is lowercase; "User" is NOT the User role, and silently
    // treating it as one would send a malformed role to the provider.
    assert_eq!(
        MessageRole::from("User".to_string()),
        MessageRole::Custom("User".to_string())
    );
}

#[test]
fn message_role_display_matches_the_wire_form() {
    assert_eq!(MessageRole::User.to_string(), "user");
    assert_eq!(MessageRole::Agent.to_string(), "agent");
    assert_eq!(MessageRole::System.to_string(), "system");
    assert_eq!(MessageRole::Tool.to_string(), "tool");
    assert_eq!(
        MessageRole::Custom("reviewer".into()).to_string(),
        "reviewer"
    );
}

#[test]
fn message_role_string_conversion_round_trips() {
    for role in [
        MessageRole::User,
        MessageRole::Agent,
        MessageRole::System,
        MessageRole::Tool,
        MessageRole::Custom("auditor".into()),
    ] {
        let wire = role.to_string();
        assert_eq!(
            MessageRole::from(wire.clone()),
            role,
            "role {role:?} must survive a Display → From round trip via {wire:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// KVCacheType
// ---------------------------------------------------------------------------

#[test]
fn kv_cache_bytes_per_element() {
    assert_eq!(KVCacheType::F32.bytes_per_element(), 4);
    assert_eq!(KVCacheType::F16.bytes_per_element(), 2);
    assert_eq!(KVCacheType::Q8_0.bytes_per_element(), 1);
    assert_eq!(KVCacheType::Q5_0.bytes_per_element(), 1);
}

#[test]
fn kv_cache_precision_ordering_is_monotonic() {
    // Higher precision must never claim fewer bytes — a KV-cache sizing bug
    // shows up as OOM at load time, far from the cause.
    assert!(
        KVCacheType::F32.bytes_per_element() > KVCacheType::F16.bytes_per_element(),
        "F32 must be wider than F16"
    );
    assert!(
        KVCacheType::F16.bytes_per_element() > KVCacheType::Q8_0.bytes_per_element(),
        "F16 must be wider than Q8_0"
    );
}

// ---------------------------------------------------------------------------
// ChatMessage
// ---------------------------------------------------------------------------

#[test]
fn chat_message_new_sets_both_fields() {
    let m = ChatMessage::new("system", "be brief");
    assert_eq!(m.role, "system");
    assert_eq!(m.content, "be brief");
}

#[test]
fn chat_message_role_constructors() {
    assert_eq!(ChatMessage::user("hi").role, "user");
    assert_eq!(ChatMessage::assistant("hello").role, "assistant");
    assert_eq!(ChatMessage::system("rules").role, "system");
}

#[test]
fn chat_message_constructors_preserve_content() {
    assert_eq!(ChatMessage::user("payload").content, "payload");
    assert_eq!(ChatMessage::assistant("payload").content, "payload");
    assert_eq!(ChatMessage::system("payload").content, "payload");
}

// ---------------------------------------------------------------------------
// Args — hand-written PartialEq / Serialize / Deserialize
// ---------------------------------------------------------------------------

#[test]
fn args_equality_is_by_schema() {
    let a = Args::from_value(serde_json::json!({"type": "object"}));
    let b = Args::from_value(serde_json::json!({"type": "object"}));
    let c = Args::from_value(serde_json::json!({"type": "string"}));

    assert_eq!(a, b, "identical schemas must compare equal");
    assert_ne!(a, c, "different schemas must not compare equal");
}

#[test]
fn args_serializes_to_its_bare_schema() {
    // Args must serialize transparently as the schema, not as a wrapper object,
    // because the value goes straight into provider request bodies.
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"q": {"type": "string"}},
        "required": ["q"]
    });
    let args = Args::from_value(schema.clone());

    let serialized = serde_json::to_value(&args).expect("Args must serialize");
    assert_eq!(serialized, schema);
}

#[test]
fn args_deserializes_from_a_bare_schema() {
    let schema = serde_json::json!({"type": "object", "properties": {"n": {"type": "integer"}}});
    let args: Args = serde_json::from_value(schema.clone()).expect("Args must deserialize");
    assert_eq!(args.schema, schema);
}

#[test]
fn args_serde_round_trips() {
    let original = Args::from_value(serde_json::json!({
        "type": "object",
        "properties": {"a": {"type": "string"}, "b": {"type": "number"}},
        "required": ["a"]
    }));

    let json = serde_json::to_value(&original).expect("serialize");
    let restored: Args = serde_json::from_value(json).expect("deserialize");

    assert_eq!(
        original, restored,
        "an Args must survive a serialize → deserialize round trip"
    );
}

#[test]
fn args_empty_is_usable_and_serializes() {
    let empty = Args::empty();
    let json = serde_json::to_value(&empty).expect("empty Args must serialize");
    // Round trips like any other.
    let restored: Args = serde_json::from_value(json).expect("deserialize");
    assert_eq!(empty, restored);
}

// ---------------------------------------------------------------------------
// Tool::definitions / arg_summary / ToolShed::with_tools
// ---------------------------------------------------------------------------
//
// `arg_summary` is what a TEXT-BASED model (llama.cpp / Candle) is shown to
// describe a tool's arguments — those models have no native function-calling
// API, so this string is the only thing telling the model what to emit. A wrong
// summary means the model guesses at argument names.

use foundation_ai::types::{Tool, ToolDefinition, ToolShed};

fn schema_opts(json: serde_json::Value) -> foundation_jsonschema::ValidationOptions {
    foundation_jsonschema::ValidationOptions::with_schema(json)
}

fn single(name: &str, props: serde_json::Value) -> Tool {
    Tool::SingleCommand(ToolDefinition {
        name: name.into(),
        category: "test".into(),
        description: "d".into(),
        arguments: Args::new(schema_opts(
            serde_json::json!({"type": "object", "properties": props}),
        )),
        returns: None,
    })
}

fn multi(name: &str, cmds: &[&str]) -> Tool {
    Tool::MultiCommands(
        name.into(),
        cmds.iter()
            .map(|c| ToolDefinition {
                name: (*c).into(),
                category: "test".into(),
                description: "d".into(),
                arguments: Args::new(schema_opts(serde_json::json!({"type": "object"}))),
                returns: None,
            })
            .collect(),
    )
}

#[test]
fn definitions_yields_one_entry_for_a_single_command() {
    let t = single("read", serde_json::json!({"path": {"type": "string"}}));
    let defs: Vec<_> = t.definitions().collect();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "read");
}

#[test]
fn definitions_yields_every_command_for_a_multi_command() {
    let t = multi("agent", &["start", "check", "stop"]);
    let names: Vec<&str> = t.definitions().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["start", "check", "stop"]);
}

#[test]
fn arg_summary_lists_property_names_for_a_single_command() {
    let t = single(
        "search",
        serde_json::json!({"query": {"type": "string"}, "limit": {"type": "integer"}}),
    );
    let summary = t.arg_summary();
    assert!(summary.contains("query"), "got: {summary}");
    assert!(summary.contains("limit"), "got: {summary}");
}

#[test]
fn arg_summary_is_empty_when_a_command_takes_no_arguments() {
    let t = single("ping", serde_json::json!({}));
    assert_eq!(
        t.arg_summary(),
        "",
        "a no-argument tool must summarise as empty, not as a stray separator"
    );
}

#[test]
fn arg_summary_lists_the_command_discriminator_for_a_multi_command() {
    // A MultiCommands tool's first argument is always `command`, so the summary
    // shows the choices rather than the union of every branch's properties.
    let t = multi("memory", &["add", "remove", "replace"]);
    assert_eq!(t.arg_summary(), "command: add|remove|replace");
}

#[test]
fn arg_summary_handles_a_single_command_group() {
    let t = multi("solo", &["only"]);
    assert_eq!(
        t.arg_summary(),
        "command: only",
        "one command must not produce a trailing separator"
    );
}

#[test]
fn toolshed_with_tools_replaces_the_list() {
    let shed = ToolShed::default().with_tools(vec![
        single("a", serde_json::json!({})),
        single("b", serde_json::json!({})),
    ]);
    let names: Vec<&str> = shed.tools.iter().map(Tool::name).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn toolshed_with_tools_overwrites_rather_than_appends() {
    // Appending would silently double-register tools across two calls.
    let shed = ToolShed::default()
        .with_tools(vec![single("first", serde_json::json!({}))])
        .with_tools(vec![single("second", serde_json::json!({}))]);
    let names: Vec<&str> = shed.tools.iter().map(Tool::name).collect();
    assert_eq!(names, vec!["second"], "the second call must replace the first");
}

#[test]
fn toolshed_all_tools_includes_the_shed_metatool_first() {
    // `all_tools` is what a provider iterates to build its tool list; the shed
    // meta-tool must lead so discovery is offered before the tools it finds.
    let shed = ToolShed {
        shed: Some(single("shed", serde_json::json!({}))),
        tools: vec![single("read", serde_json::json!({}))],
    };
    let all = shed.all_tools();
    let names: Vec<&str> = all.iter().map(|t| t.name()).collect();
    assert_eq!(names, vec!["shed", "read"]);
}

#[test]
fn toolshed_all_tools_omits_an_absent_metatool() {
    let shed = ToolShed {
        shed: None,
        tools: vec![single("read", serde_json::json!({}))],
    };
    let all = shed.all_tools();
    let names: Vec<&str> = all.iter().map(|t| t.name()).collect();
    assert_eq!(names, vec!["read"]);
}
