//! Offline coverage for `types/base_types.rs` — the string↔enum conversions,
//! `Messages::is_context_overflow`, the numeric/bool/null/array branches of the
//! text tool-call parser, and the `LlamaConfig` builder chain. All pure, no I/O.

use std::time::SystemTime;

use foundation_ai::agentic::testing::zero_usage;
use foundation_ai::types::{
    ArgType, CacheRetention, KVCacheType, LlamaConfig, Messages, MimeType, ModelAPI, ModelId,
    ModelOutput, ModelProviders, SplitMode, StopReason, TextBasedFormatter, ThinkingLevels, Tool,
    ToolFormatter, UsageReport,
};

// ---------------------------------------------------------------------------
// String → enum conversions (From<String> and From<&'static str>)
// ---------------------------------------------------------------------------

#[test]
fn cache_retention_from_strings() {
    assert_eq!(CacheRetention::from("none"), CacheRetention::None);
    assert_eq!(CacheRetention::from("short"), CacheRetention::Short);
    assert_eq!(CacheRetention::from("long"), CacheRetention::Long);
    assert_eq!(
        CacheRetention::from("2h".to_string()),
        CacheRetention::Custom("2h".to_string())
    );
    // owned-String path
    assert_eq!(CacheRetention::from("none".to_string()), CacheRetention::None);
}

#[test]
fn thinking_levels_from_strings() {
    assert_eq!(ThinkingLevels::from("low"), ThinkingLevels::Low);
    assert_eq!(ThinkingLevels::from("high"), ThinkingLevels::High);
    assert_eq!(ThinkingLevels::from("medium"), ThinkingLevels::Medium);
    assert_eq!(ThinkingLevels::from("minimal"), ThinkingLevels::Minimal);
    assert_eq!(
        ThinkingLevels::from("ultra".to_string()),
        ThinkingLevels::Custom("ultra".to_string())
    );
}

#[test]
fn model_providers_from_strings() {
    assert_eq!(ModelProviders::from("openai"), ModelProviders::OPENAI);
    assert_eq!(ModelProviders::from("anthropic"), ModelProviders::ANTHROPIC);
    assert_eq!(ModelProviders::from("openrouter"), ModelProviders::OPENROUTER);
    assert_eq!(ModelProviders::from("huggingface"), ModelProviders::HUGGINGFACE);
    assert_eq!(ModelProviders::from("candle"), ModelProviders::CANDLE);
    // owned-String path + unknown → Custom
    assert_eq!(
        ModelProviders::from("some-vendor".to_string()),
        ModelProviders::Custom("some-vendor".to_string())
    );
    assert_eq!(ModelProviders::from("mistral"), ModelProviders::MISTRAL);
}

#[test]
fn model_api_from_strings() {
    assert_eq!(ModelAPI::from("candle"), ModelAPI::Candle);
    assert_eq!(
        ModelAPI::from("openai-completions"),
        ModelAPI::OpenAICompletions
    );
    assert_eq!(
        ModelAPI::from("anthropic-messages"),
        ModelAPI::AnthropicMessages
    );
    assert_eq!(
        ModelAPI::from("google-vertex".to_string()),
        ModelAPI::GoogleVertex
    );
    assert_eq!(
        ModelAPI::from("weird".to_string()),
        ModelAPI::Custom("weird".to_string())
    );
}

#[test]
fn mime_type_from_strings() {
    assert_eq!(MimeType::from("application/json"), MimeType::ApplicationJson);
    assert_eq!(MimeType::from("image/png"), MimeType::ImagePng);
    assert_eq!(MimeType::from("image/svg+xml"), MimeType::ImageSvgXml);
    assert_eq!(MimeType::from("audio/mpeg"), MimeType::AudioMpeg);
    assert_eq!(MimeType::from("video/mp4"), MimeType::VideoMp4);
    assert_eq!(
        MimeType::from("application/x-custom".to_string()),
        MimeType::Custom("application/x-custom".to_string())
    );
}

#[test]
fn stop_reason_from_strings_is_case_insensitive() {
    assert_eq!(StopReason::from("STOP"), StopReason::Stop);
    assert_eq!(StopReason::from("Length"), StopReason::Length);
    assert_eq!(StopReason::from("tool_calls"), StopReason::ToolUse);
    assert_eq!(StopReason::from("tooluse"), StopReason::ToolUse);
    assert_eq!(StopReason::from("error"), StopReason::Error);
    assert_eq!(StopReason::from("aborted"), StopReason::Aborted);
    assert_eq!(
        StopReason::from("guardrail".to_string()),
        StopReason::Message("guardrail".to_string())
    );
}

// ---------------------------------------------------------------------------
// Messages::is_context_overflow
// ---------------------------------------------------------------------------

fn assistant(stop_reason: StopReason, error_detail: Option<String>, input: f64) -> Messages {
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("m".to_string(), None),
        timestamp: SystemTime::now(),
        usage: UsageReport {
            input,
            total_tokens: input,
            ..zero_usage()
        },
        content: ModelOutput::Text(foundation_ai::types::TextContent {
            content: String::new(),
            signature: None,
        }),
        stop_reason,
        provider: ModelProviders::OPENAI,
        error_detail,
        signature: None,
        metadata: None,
    }
}

#[test]
fn context_overflow_detected_from_error_pattern() {
    let msg = assistant(
        StopReason::Error,
        Some("This model's maximum context length is 8192 tokens".to_string()),
        0.0,
    );
    assert!(msg.is_context_overflow(8192));
}

#[test]
fn context_overflow_detected_from_silent_400_no_body() {
    let msg = assistant(
        StopReason::Error,
        Some("400 status code (no body)".to_string()),
        0.0,
    );
    assert!(msg.is_context_overflow(4096));
}

#[test]
fn context_overflow_detected_from_usage_exceeding_window() {
    // z.ai style: a successful stop, but the input tokens exceed the window.
    let msg = assistant(StopReason::Stop, None, 5000.0);
    assert!(msg.is_context_overflow(4096));
}

#[test]
fn no_context_overflow_for_normal_stop() {
    let msg = assistant(StopReason::Stop, None, 100.0);
    assert!(!msg.is_context_overflow(4096));
    // An unrelated error is not an overflow.
    let other = assistant(StopReason::Error, Some("rate limited".to_string()), 0.0);
    assert!(!other.is_context_overflow(4096));
    // A non-assistant message is never an overflow.
    let user = foundation_ai::agentic::testing::mock_user("hi");
    assert!(!user.is_context_overflow(1));
}

// ---------------------------------------------------------------------------
// TextBasedFormatter — exercises json_value_to_arg_type's numeric/bool/null/
// array branches (string+object branches are already covered elsewhere).
// ---------------------------------------------------------------------------

#[test]
fn text_formatter_parses_all_json_arg_types() {
    let formatter = TextBasedFormatter;
    let response = r#"<ToolCall>{"name":"do_it","arguments":{"count":3,"ratio":1.5,"flag":true,"nothing":null,"items":[1,2,3]}}</ToolCall>"#;
    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);

    let ModelOutput::ToolCall { arguments, name, .. } = &result.calls[0] else {
        panic!("expected a tool call");
    };
    assert_eq!(name, "do_it");
    let top = arguments.as_ref().expect("arguments present");

    // The parser keeps `name` + `arguments` at the top level; the real params
    // live in the nested `arguments` JSONMap.
    let Some(ArgType::JSONMap(args)) = top.get("arguments") else {
        panic!("nested arguments map: {top:?}");
    };

    // integer → I64
    assert!(matches!(args.get("count"), Some(ArgType::I64(3))));
    // float → Float64
    assert!(matches!(args.get("ratio"), Some(ArgType::Float64(f)) if (*f - 1.5).abs() < 1e-9));
    // bool → Text("true")
    assert!(matches!(args.get("flag"), Some(ArgType::Text(t)) if t == "true"));
    // null → Text("")
    assert!(matches!(args.get("nothing"), Some(ArgType::Text(t)) if t.is_empty()));
    // array → Text("1, 2, 3")
    assert!(matches!(args.get("items"), Some(ArgType::Text(t)) if t == "1, 2, 3"));
}

#[test]
fn text_formatter_format_tools_and_instructions() {
    let formatter = TextBasedFormatter;
    let tools = vec![Tool {
        name: "ping".to_string(),
        description: "pong".to_string(),
        arguments: None,
        returns: None,
    }];
    let formatted = formatter.format_tools(&tools).unwrap();
    let arr = formatted.as_array().expect("array of tool schemas");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "ping");
    // No args → default empty object schema.
    assert_eq!(arr[0]["parameters"]["type"], "object");

    let instructions = formatter.tool_calling_instructions().unwrap();
    assert!(instructions.contains("<ToolCall>"));
}

// ---------------------------------------------------------------------------
// LlamaConfig builder chain
// ---------------------------------------------------------------------------

#[test]
fn llama_config_builder_sets_every_field() {
    let cfg = LlamaConfig::new()
        .with_n_gpu_layers(24)
        .with_main_gpu(1)
        .with_split_mode(SplitMode::Row)
        .with_kv_cache_type(KVCacheType::Q8_0)
        .with_mmap(false)
        .with_mlock(true);

    assert_eq!(cfg.n_gpu_layers, 24);
    assert_eq!(cfg.main_gpu, 1);
    assert_eq!(cfg.split_mode, SplitMode::Row);
    assert_eq!(cfg.kv_cache_type, KVCacheType::Q8_0);
    assert!(!cfg.use_mmap);
    assert!(cfg.use_mlock);

    // Default is distinct from the customised config.
    assert_ne!(LlamaConfig::default(), cfg);
}
