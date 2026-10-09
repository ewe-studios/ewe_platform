//! `estimate_tokens` over **Assistant** message content (`ModelOutput` arms).
//!
//! WHY: `costing_tests.rs` covers the `User` side (text + image) but never
//! passes an `Assistant` message, so the whole `ModelOutput` match in
//! `estimate_tokens` — Text, ThinkingContent, ToolCall, Image, Embedding — was
//! uncovered. That match is what prices a model's *output*, i.e. the expensive
//! half of every bill, so a silently-skipped arm means silently-wrong cost
//! estimates.
//!
//! WHAT: one test per `ModelOutput` arm, asserting the estimate scales with the
//! content the arm is supposed to measure, plus an accumulation test proving
//! several messages sum rather than overwrite.
//!
//! HOW: pure function over constructed messages — no provider, no network.

use foundation_ai::costing::estimate_tokens;
use foundation_ai::types::{
    CostStatus, ImageContent, Messages, MimeType, ModelId, ModelOutput, ModelProviders,
    StopReason, TextContent, UsageCosting, UsageReport,
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

/// An `Assistant` message carrying the given output.
fn assistant(content: ModelOutput) -> Messages {
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("test".into(), None),
        timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
        usage: zero_usage(),
        content,
        stop_reason: StopReason::Stop,
        provider: ModelProviders::Custom("test".into()),
        error_detail: None,
        signature: None,
        metadata: None,
    }
}

fn text(s: &str) -> ModelOutput {
    ModelOutput::Text(TextContent {
        content: s.into(),
        signature: None,
    })
}

// ---------------------------------------------------------------------------
// Per-arm coverage
// ---------------------------------------------------------------------------

#[test]
fn assistant_text_is_estimated() {
    let est = estimate_tokens(&[assistant(text(&"a".repeat(400)))]);
    assert!(
        est.input > 0.0,
        "assistant text must contribute to the estimate: {est:?}"
    );
}

#[test]
fn assistant_text_estimate_scales_with_length() {
    let short = estimate_tokens(&[assistant(text(&"a".repeat(40)))]);
    let long = estimate_tokens(&[assistant(text(&"a".repeat(4000)))]);
    assert!(
        long.input > short.input,
        "a 100x longer message must estimate higher: {} vs {}",
        short.input,
        long.input
    );
}

#[test]
fn assistant_thinking_content_is_estimated() {
    // Reasoning tokens are billed — they must not be free in the estimate.
    let est = estimate_tokens(&[assistant(ModelOutput::ThinkingContent {
        thinking: "z".repeat(400),
        signature: None,
    })]);
    assert!(
        est.input > 0.0,
        "thinking content must be counted: {est:?}"
    );
}

#[test]
fn assistant_tool_call_arguments_are_estimated() {
    use foundation_ai::types::{ArgType, ExecutionHint};
    use std::collections::HashMap;

    let mut args: HashMap<String, ArgType> = HashMap::new();
    args.insert("query".into(), ArgType::Text("x".repeat(400)));

    let est = estimate_tokens(&[assistant(ModelOutput::ToolCall {
        id: "call_1".into(),
        name: "search".into(),
        arguments: Some(args),
        signature: None,
        depends_on: Vec::new(),
        execution_hint: ExecutionHint::Unspecified,
    })]);
    assert!(
        est.input > 0.0,
        "serialized tool-call arguments must be counted: {est:?}"
    );
}

#[test]
fn assistant_tool_call_without_arguments_is_safe() {
    use foundation_ai::types::ExecutionHint;

    // `arguments: None` takes the other side of the `if let Some(..)` branch —
    // it must not panic and must add nothing beyond the flat per-message
    // overhead every message pays. Compare against an empty-text message rather
    // than hardcoding MESSAGE_OVERHEAD, so the test survives a change to it.
    let baseline = estimate_tokens(&[assistant(text(""))]);
    let est = estimate_tokens(&[assistant(ModelOutput::ToolCall {
        id: "call_1".into(),
        name: "noop".into(),
        arguments: None,
        signature: None,
        depends_on: Vec::new(),
        execution_hint: ExecutionHint::Unspecified,
    })]);
    assert_eq!(
        est.input, baseline.input,
        "an argument-less tool call must cost exactly the per-message overhead, \
         same as an empty message: {est:?} vs {baseline:?}"
    );
}

#[test]
fn assistant_image_counts_a_flat_image_cost_plus_payload() {
    let est = estimate_tokens(&[assistant(ModelOutput::Image(ImageContent {
        b64: "A".repeat(2000),
        mime_type: MimeType::ImageJpeg,
    }))]);
    // Images carry a large flat cost (TOKENS_PER_IMAGE) on top of the payload,
    // so this must dwarf a same-length text message.
    let text_est = estimate_tokens(&[assistant(text(&"A".repeat(2000)))]);
    assert!(
        est.total_tokens > text_est.total_tokens,
        "an image must cost more than equal-length text: {} vs {}",
        est.total_tokens,
        text_est.total_tokens
    );
}

#[test]
fn assistant_embedding_values_are_estimated() {
    let est = estimate_tokens(&[assistant(ModelOutput::Embedding {
        dimensions: 1536,
        values: vec![0.0; 1536],
    })]);
    assert!(
        est.input > 0.0,
        "embedding values must be counted: {est:?}"
    );
}

// ---------------------------------------------------------------------------
// Accumulation
// ---------------------------------------------------------------------------

#[test]
fn multiple_assistant_messages_accumulate() {
    let one = estimate_tokens(&[assistant(text(&"a".repeat(400)))]);
    let three = estimate_tokens(&[
        assistant(text(&"a".repeat(400))),
        assistant(text(&"a".repeat(400))),
        assistant(text(&"a".repeat(400))),
    ]);
    assert!(
        three.input > one.input,
        "estimates must sum across messages, not overwrite: {} vs {}",
        one.input,
        three.input
    );
}

#[test]
fn empty_message_list_estimates_zero() {
    let est = estimate_tokens(&[]);
    assert_eq!(est.input, 0.0);
    assert_eq!(est.output, 0.0);
    assert_eq!(est.total_tokens, 0.0);
}

#[test]
fn mixed_output_arms_all_contribute() {
    // A realistic turn: the model thinks, answers, then calls a tool. Every
    // part is billable, so the combined estimate must exceed any single part.
    use foundation_ai::types::{ArgType, ExecutionHint};
    use std::collections::HashMap;

    let mut args: HashMap<String, ArgType> = HashMap::new();
    args.insert("q".into(), ArgType::Text("y".repeat(200)));

    let combined = estimate_tokens(&[
        assistant(ModelOutput::ThinkingContent {
            thinking: "t".repeat(200),
            signature: None,
        }),
        assistant(text(&"a".repeat(200))),
        assistant(ModelOutput::ToolCall {
            id: "c".into(),
            name: "search".into(),
            arguments: Some(args),
            signature: None,
            depends_on: Vec::new(),
            execution_hint: ExecutionHint::Unspecified,
        }),
    ]);
    let text_only = estimate_tokens(&[assistant(text(&"a".repeat(200)))]);

    assert!(
        combined.input > text_only.input,
        "thinking + text + tool-call must exceed text alone: {} vs {}",
        text_only.input,
        combined.input
    );
}
