use std::borrow::Cow;

use foundation_ai::agentic::progress::*;
use foundation_ai::types::{
    CostStatus, MessageRole, Messages, ModelId, ModelState, SessionRecord, TextContent,
    UsageCosting, UsageReport, UserModelContent,
};
use foundation_core::valtron::Stream;

fn user_message() -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "hi".into(),
            signature: None,
        }),
        signature: None,
    }
}

#[test]
fn agent_progress_serde_round_trips() {
    let progresses = vec![
        AgentProgress::Initializing {
            step: Cow::Borrowed("loading"),
        },
        AgentProgress::ToolCallRequested {
            name: "read".into(),
        },
        AgentProgress::ExecutingTools {
            total: 2,
            completed: 1,
        },
        AgentProgress::ProcessingMemory {
            kind: MemoryKind::Observation,
        },
        AgentProgress::SessionEnding,
    ];
    for p in progresses {
        let json = serde_json::to_string(&p).unwrap();
        let back: AgentProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}

#[test]
fn lift_next_message_becomes_conversation() {
    let model = ModelId::Name("m".into(), None);
    let lifted = lift_model_item(Stream::Next(user_message()), &model);
    match lifted {
        Stream::Next(SessionRecord::Conversation { message }) => {
            assert!(matches!(message, Messages::User { .. }));
        }
        other => panic!("expected Conversation Next, got {other:?}"),
    }
}

#[test]
fn lift_model_error_becomes_failed_action_record() {
    let model = ModelId::Name("m".into(), None);
    let lifted = lift_model_item(Stream::Pending(ModelState::Error("boom".into())), &model);
    match lifted {
        Stream::Next(SessionRecord::FailedAction { error, trace }) => {
            assert_eq!(error, foundation_ai::agentic::errors::AgenticError::Unexpected("boom".into()));
            // The structured trace's current context is the error's Display.
            assert!(trace.current_context.contains("boom"));
        }
        other => panic!("expected FailedAction Next, got {other:?}"),
    }
}

#[test]
fn lift_generating_tokens_becomes_generating_progress() {
    let model = ModelId::Name("m".into(), None);
    let usage = UsageReport {
        input: 10.0,
        output: 5.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 15.0,
        cost: UsageCosting::zero(CostStatus::Actual),
    };
    let lifted = lift_model_item(
        Stream::Pending(ModelState::GeneratingTokens(Some(usage))),
        &model,
    );
    match lifted {
        Stream::Pending(AgentProgress::Generating {
            model: m,
            tokens_so_far,
        }) => {
            assert_eq!(m, model);
            assert_eq!(tokens_so_far, Some(15));
        }
        other => panic!("expected Generating progress, got {other:?}"),
    }
}

#[test]
fn lift_control_variants_pass_through() {
    let model = ModelId::Name("m".into(), None);
    assert!(matches!(
        lift_model_item(Stream::Ignore, &model),
        Stream::Ignore
    ));
    assert!(matches!(
        lift_model_item(Stream::Wait, &model),
        Stream::Wait
    ));
    assert!(matches!(
        lift_model_item(Stream::Init, &model),
        Stream::Init
    ));
}

// ---------------------------------------------------------------------------
// Additional lift + From coverage (matrix 5.1 support; progress.rs was 36%).

#[test]
fn lift_delayed_passes_through() {
    let model = ModelId::Name("m".into(), None);
    let d = std::time::Duration::from_millis(5);
    assert!(matches!(
        lift_model_item(Stream::Delayed(d), &model),
        Stream::Delayed(_)
    ));
}

#[test]
fn lift_spread_lifts_each_element() {
    use foundation_core::valtron::StreamSpread;
    let model = ModelId::Name("m".into(), None);
    let items = vec![
        StreamSpread::Done(user_message()),
        StreamSpread::Pending(ModelState::Finished),
    ];
    match lift_model_item(Stream::Spread(items), &model) {
        Stream::Spread(lifted) => {
            assert_eq!(lifted.len(), 2, "both spread elements must be lifted");
            assert!(matches!(lifted[0], StreamSpread::Done(_)));
            assert!(matches!(lifted[1], StreamSpread::Pending(_)));
        }
        other => panic!("expected Spread, got {other:?}"),
    }
}

#[test]
fn from_model_state_finished_is_session_ending() {
    let p = AgentProgress::from(&ModelState::Finished);
    assert!(matches!(p, AgentProgress::SessionEnding));
}

#[test]
fn from_model_state_embeddings_is_initializing() {
    let p = AgentProgress::from(&ModelState::GeneratingEmbeddings);
    assert!(matches!(p, AgentProgress::Initializing { .. }));
}

#[test]
fn from_model_state_error_is_initializing_with_message() {
    let p = AgentProgress::from(&ModelState::Error("boom".into()));
    match p {
        AgentProgress::Initializing { step } => {
            assert!(step.contains("boom"), "error message should carry through: {step}");
        }
        other => panic!("expected Initializing, got {other:?}"),
    }
}
