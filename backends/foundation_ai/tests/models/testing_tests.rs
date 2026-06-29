#![allow(unused_imports)]
use std::collections::HashMap;

use foundation_ai::agentic::testing::*;
use foundation_ai::types::{
    ArgType, CostStatus, ModelInteraction, ModelOutput, Messages, StopReason, TextContent,
    ToolShed, UsageCosting, UsageReport, UserModelContent,
};
use foundation_ai::agentic::tool_impl::ToolImpl;

#[test]
fn mock_provider_returns_scripted_reply() {
    let mut mock = MockModelProvider::new();
    mock.on(|_| true, vec![mock_text("hello")]);

    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![mock_user("hi")],
        chat_template: None,
        tool_choice: None,
    };

    let result = mock.resolve(&mi).expect("should match");
    assert_eq!(result.len(), 1);
    match &result[0] {
        Messages::Assistant { content, .. } => match content {
            ModelOutput::Text(t) => assert_eq!(t.content, "hello"),
            other => panic!("expected text, got {other:?}"),
        },
        other => panic!("expected assistant, got {other:?}"),
    }
}

#[test]
fn mock_provider_nth_call_matching() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_text("first")])
        .on_nth_call(1, vec![mock_text("second")])
        .on_any(vec![mock_text("fallback")]);

    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![mock_user("test")],
        chat_template: None,
        tool_choice: None,
    };

    let r0 = mock.resolve(&mi).unwrap();
    assert!(
        matches!(&r0[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "first")
    );

    let r1 = mock.resolve(&mi).unwrap();
    assert!(
        matches!(&r1[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "second")
    );

    let r2 = mock.resolve(&mi).unwrap();
    assert!(
        matches!(&r2[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "fallback")
    );
}

#[test]
fn mock_provider_failure_injection() {
    let mut mock = MockModelProvider::new();
    mock.fail_with(|_| true, "injected error");

    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![mock_user("hi")],
        chat_template: None,
        tool_choice: None,
    };

    let err = mock.resolve(&mi).unwrap_err();
    assert!(err.to_string().contains("injected error"));
}

#[test]
fn mock_provider_no_match_errors() {
    let mock = MockModelProvider::new();

    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let err = mock.resolve(&mi).unwrap_err();
    assert!(err.to_string().contains("no matching script"));
}

#[test]
fn mock_provider_call_count() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);

    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![],
        chat_template: None,
        tool_choice: None,
    };

    assert_eq!(mock.call_count(), 0);
    let _ = mock.resolve(&mi);
    assert_eq!(mock.call_count(), 1);
    let _ = mock.resolve(&mi);
    assert_eq!(mock.call_count(), 2);
}

#[test]
fn last_user_contains_matcher() {
    let mi = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![mock_user("fix the bug")],
        chat_template: None,
        tool_choice: None,
    };

    assert!(last_user_contains("fix")(&mi));
    assert!(!last_user_contains("deploy")(&mi));
}

#[test]
fn system_prompt_contains_matcher() {
    let mi = ModelInteraction {
        system_prompt: Some("You are a coding assistant.".into()),
        soul: None,
        tools_shed: ToolShed::default(),
        messages: vec![],
        chat_template: None,
        tool_choice: None,
    };

    assert!(system_prompt_contains("coding")(&mi));
    assert!(!system_prompt_contains("chef")(&mi));
}

#[test]
fn mock_text_builds_assistant_message() {
    let msg = mock_text("hello world");
    match msg {
        Messages::Assistant {
            content: ModelOutput::Text(t),
            stop_reason,
            ..
        } => {
            assert_eq!(t.content, "hello world");
            assert_eq!(stop_reason, StopReason::Stop);
        }
        other => panic!("expected Assistant text, got {other:?}"),
    }
}

#[test]
fn mock_text_usage_carries_report() {
    let usage = UsageReport {
        input: 100.0,
        output: 50.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 150.0,
        cost: UsageCosting::zero(CostStatus::Actual),
    };
    let msg = mock_text_usage("hi", usage.clone());
    match msg {
        Messages::Assistant { usage: u, .. } => {
            assert_eq!(u.input, 100.0);
            assert_eq!(u.total_tokens, 150.0);
        }
        other => panic!("expected Assistant, got {other:?}"),
    }
}

#[test]
fn mock_tool_call_builds_message() {
    let mut args = HashMap::new();
    args.insert("path".into(), foundation_ai::types::ArgType::Text("/tmp".into()));
    let msg = mock_tool_call("read_file", args);
    match msg {
        Messages::Assistant {
            content:
                ModelOutput::ToolCall {
                    name, arguments, ..
                },
            stop_reason,
            ..
        } => {
            assert_eq!(name, "read_file");
            assert!(arguments.is_some());
            assert_eq!(stop_reason, StopReason::ToolUse);
        }
        other => panic!("expected tool call, got {other:?}"),
    }
}

#[test]
fn mock_tool_returning_succeeds() {
    let tool = MockTool::returning("echo", "echoed");
    let result = futures_lite::future::block_on(tool.execute(HashMap::new())).unwrap();
    match &result.content {
        UserModelContent::Text(t) => assert_eq!(t.content, "echoed"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn mock_tool_failing_errors() {
    let tool = MockTool::failing("broken", "always fails");
    let err = futures_lite::future::block_on(tool.execute(HashMap::new())).unwrap_err();
    assert!(err.to_string().contains("always fails"));
}

#[test]
fn mock_tool_fails_then_succeeds() {
    use foundation_ai::agentic::tool_impl::{ToolCallResult, ToolError};

    let tool = MockTool::new(
        "flaky",
        ToolBehavior::FailsThenSucceeds {
            failures: 2,
            error: ToolError::Execution {
                tool: "flaky".into(),
                reason: "transient".into(),
            },
            result: ToolCallResult {
                content: UserModelContent::Text(TextContent {
                    content: "ok".into(),
                    signature: None,
                }),
                error_detail: None,
            },
        },
    );

    use futures_lite::future::block_on;

    assert!(block_on(tool.execute(HashMap::new())).is_err());
    assert!(block_on(tool.execute(HashMap::new())).is_err());
    assert!(block_on(tool.execute(HashMap::new())).is_ok());
    assert!(block_on(tool.execute(HashMap::new())).is_ok());
}

#[test]
fn mock_provider_into_router() {
    use foundation_ai::types::ModelId;

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("routed")]);
    let router = mock.into_router();

    assert!(router.is_single());
    let provider = router
        .resolve(&ModelId::Name("anything".into(), None))
        .unwrap();
    assert_eq!(provider.name(), "mock");
}

#[test]
fn mock_stream_iterator_yields_messages() {
    use foundation_core::valtron::Stream;

    let messages = vec![mock_text("a"), mock_text("b")];
    let mut iter = MockStreamIterator::new(messages);

    assert!(matches!(iter.next(), Some(Stream::Init)));
    assert!(matches!(iter.next(), Some(Stream::Next(_))));
    assert!(matches!(iter.next(), Some(Stream::Next(_))));
    assert!(iter.next().is_none());
}

#[test]
fn routable_provider_serves_all() {
    use foundation_ai::types::ModelId;
    use foundation_ai::types::RoutableProvider;

    let mock = MockModelProvider::new();
    assert!(mock.serves(&ModelId::Name("gpt-4".into(), None)));
    assert!(mock.serves(&ModelId::Name("claude-3".into(), None)));
}

#[test]
fn tool_definition_has_correct_name() {
    let tool = MockTool::returning("my_tool", "result");
    let def = tool.definition();
    assert_eq!(def.name, "my_tool");
    assert_eq!(def.category, "mock");
}
