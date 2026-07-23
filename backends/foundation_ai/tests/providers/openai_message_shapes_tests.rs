//! `build_chat_request` / `parse_chat_response` message-shape handling.
//!
//! WHY: these two functions are the wire boundary — everything the model is
//! told and everything it says passes through them. The happy text path is well
//! covered, but the multimodal serialization (assistant image parts) and the
//! response-side `Parts` / `finish_reason` mapping were not: an assistant image
//! silently dropped means a vision turn loses its content, and an unmapped
//! `finish_reason` misreports why generation stopped (e.g. a truncated answer
//! read as a complete one).
//!
//! WHAT: image mime-type -> data-URL serialization for every supported type and
//! the fallback; response content given as `Parts`; and every `finish_reason`
//! arm including the unknown-value catch-all.
//!
//! HOW: pure functions over constructed values — no server, no credentials.

use foundation_ai::backends::openai_provider::{
    build_chat_request, parse_chat_response, ChatCompletionResponse,
};
use foundation_ai::types::{
    CostStatus, ImageContent, Messages, MimeType, ModelId, ModelInteraction,
    ModelOutput, ModelParams, ModelProviders, ModelUsageCosting, StopReason, ToolShed,
    UsageCosting, UsageReport,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting::zero(CostStatus::Estimated),
    }
}

fn free_pricing() -> ModelUsageCosting {
    ModelUsageCosting {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
    }
}

/// An interaction whose history contains one assistant image.
fn interaction_with_assistant_image(mime: MimeType) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("gpt-4o".into(), None),
            timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
            usage: zero_usage(),
            content: ModelOutput::Image(ImageContent {
                b64: "AAAA".into(),
                mime_type: mime,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
            metadata: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

/// Serialize the built request and return it as JSON for shape assertions.
fn request_json(interaction: &ModelInteraction) -> serde_json::Value {
    let req = build_chat_request("gpt-4o", interaction, &ModelParams::default(), false);
    serde_json::to_value(&req).expect("request serializes")
}

fn response_with(content: serde_json::Value, finish_reason: serde_json::Value) -> ChatCompletionResponse {
    let raw = serde_json::json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": finish_reason
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    serde_json::from_value(raw).expect("response deserializes")
}

fn parse(resp: &ChatCompletionResponse) -> (Messages, UsageReport) {
    parse_chat_response(resp, &ModelId::Name("gpt-4o".into(), None), &free_pricing())
        .expect("parse succeeds")
}

fn text_of(msg: &Messages) -> String {
    match msg {
        Messages::Assistant {
            content: ModelOutput::Text(t),
            ..
        } => t.content.clone(),
        other => panic!("expected assistant text, got {other:?}"),
    }
}

fn stop_reason_of(msg: &Messages) -> StopReason {
    match msg {
        Messages::Assistant { stop_reason, .. } => stop_reason.clone(),
        other => panic!("expected assistant message, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// build_chat_request — assistant image serialization
// ---------------------------------------------------------------------------

#[test]
fn assistant_image_is_serialized_as_a_data_url() {
    let json = request_json(&interaction_with_assistant_image(MimeType::ImagePng));
    let rendered = json.to_string();
    assert!(
        rendered.contains("data:image/png;base64,AAAA"),
        "the image must reach the wire as a data URL: {rendered}"
    );
    assert!(
        rendered.contains("image_url"),
        "it must be sent as an image_url content part: {rendered}"
    );
}

#[test]
fn every_supported_image_mime_maps_to_its_own_type() {
    // A mime collapsed to the wrong value makes the vendor reject or
    // misinterpret the image.
    for (mime, expected) in [
        (MimeType::ImagePng, "image/png"),
        (MimeType::ImageJpeg, "image/jpeg"),
        (MimeType::ImageGif, "image/gif"),
        (MimeType::ImageWebp, "image/webp"),
    ] {
        let rendered = request_json(&interaction_with_assistant_image(mime.clone())).to_string();
        assert!(
            rendered.contains(&format!("data:{expected};base64,")),
            "{mime:?} must serialize as {expected}: {rendered}"
        );
    }
}

#[test]
fn an_unsupported_image_mime_falls_back_to_png() {
    // The catch-all keeps a non-image or unknown mime from producing a
    // malformed data URL the vendor cannot parse.
    let rendered =
        request_json(&interaction_with_assistant_image(MimeType::AudioMpeg)).to_string();
    assert!(
        rendered.contains("data:image/png;base64,"),
        "an unsupported mime must fall back to image/png: {rendered}"
    );
}

#[test]
fn an_assistant_embedding_contributes_no_message() {
    // Embeddings are not conversational content — emitting one would send
    // meaningless vector data to the chat endpoint.
    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("gpt-4o".into(), None),
            timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
            usage: zero_usage(),
            content: ModelOutput::Embedding {
                dimensions: 3,
                values: vec![0.1, 0.2, 0.3],
            },
            stop_reason: StopReason::Stop,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
            metadata: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };
    let json = request_json(&interaction);
    let messages = json["messages"].as_array().expect("messages array");
    assert!(
        messages.is_empty(),
        "an embedding must not become a chat message: {messages:?}"
    );
}

// ---------------------------------------------------------------------------
// parse_chat_response — content given as Parts
// ---------------------------------------------------------------------------

#[test]
fn response_content_parts_are_joined_into_text() {
    let resp = response_with(
        serde_json::json!([
            {"type": "text", "text": "first"},
            {"type": "text", "text": "second"}
        ]),
        serde_json::json!("stop"),
    );
    let (msg, _) = parse(&resp);
    assert_eq!(
        text_of(&msg),
        "first\nsecond",
        "multiple text parts must be joined, not truncated to the first"
    );
}

#[test]
fn response_content_parts_drop_image_entries() {
    // Image parts carry no text; including them would inject a URL into the
    // assistant's spoken content.
    let resp = response_with(
        serde_json::json!([
            {"type": "text", "text": "described"},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
        ]),
        serde_json::json!("stop"),
    );
    let (msg, _) = parse(&resp);
    assert_eq!(text_of(&msg), "described");
}

#[test]
fn response_with_null_content_is_empty_not_an_error() {
    // A tool-call-only reply has null content; that is normal, not a failure.
    let resp = response_with(serde_json::Value::Null, serde_json::json!("stop"));
    let (msg, _) = parse(&resp);
    assert_eq!(text_of(&msg), "");
}

// ---------------------------------------------------------------------------
// parse_chat_response — finish_reason mapping
// ---------------------------------------------------------------------------

#[test]
fn finish_reason_maps_every_known_value() {
    let cases = [
        (serde_json::json!("stop"), StopReason::Stop),
        (serde_json::json!("length"), StopReason::Length),
        (serde_json::json!("tool_calls"), StopReason::ToolUse),
        (serde_json::json!("content_filter"), StopReason::Error),
    ];
    for (raw, expected) in cases {
        let resp = response_with(serde_json::json!("hi"), raw.clone());
        let (msg, _) = parse(&resp);
        assert_eq!(
            stop_reason_of(&msg),
            expected,
            "finish_reason {raw} must map to {expected:?}"
        );
    }
}

#[test]
fn absent_finish_reason_is_treated_as_stop() {
    let resp = response_with(serde_json::json!("hi"), serde_json::Value::Null);
    let (msg, _) = parse(&resp);
    assert_eq!(stop_reason_of(&msg), StopReason::Stop);
}

#[test]
fn an_unknown_finish_reason_is_preserved_verbatim() {
    // A new vendor stop reason must reach the caller as text rather than being
    // silently reported as a normal completion — "length" vs "stop" is the
    // difference between a truncated answer and a finished one.
    let resp = response_with(serde_json::json!("hi"), serde_json::json!("vendor_specific"));
    let (msg, _) = parse(&resp);
    assert_eq!(
        stop_reason_of(&msg),
        StopReason::Message("vendor_specific".to_string())
    );
}

#[test]
fn a_response_with_no_choices_is_an_error() {
    let raw = serde_json::json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4o",
        "choices": [],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    let resp: ChatCompletionResponse =
        serde_json::from_value(raw).expect("response deserializes");
    assert!(
        parse_chat_response(&resp, &ModelId::Name("gpt-4o".into(), None), &free_pricing())
            .is_err(),
        "an empty choices array must error rather than yield an empty completion"
    );
}
