use foundation_ai::agentic::errors::*;
use foundation_ai::errors::GenerationError;
use foundation_ai::types::base_types::ModelId;

fn gen_err(msg: &str) -> GenerationError {
    GenerationError::Generic(msg.to_string())
}

#[test]
fn agentic_error_round_trips_through_json() {
    let errors = vec![
        AgenticError::Generation(GenerationFailure {
            kind: GenKind::Provider,
            message: "boom".into(),
        }),
        AgenticError::ToolCall {
            tool_name: "read".into(),
            reason: "no file".into(),
        },
        AgenticError::ToolNotAuthorized {
            tool_name: "shell".into(),
            user: UserId("u1".into()),
        },
        AgenticError::Budget { limit: 1000 },
        AgenticError::LoopDetected(LoopDetection {
            kind: "tool_call".into(),
            occurrences: 3,
        }),
        AgenticError::Auth(AuthError {
            reason: "expired".into(),
        }),
        AgenticError::Routing("no provider for model x".into()),
        AgenticError::Unexpected("???".into()),
    ];
    for e in errors {
        let json = serde_json::to_string(&e).unwrap();
        let back: AgenticError = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }
}

#[test]
fn from_generation_classifies_rate_limit() {
    let e = gen_err("Rate limit exceeded: retry after 5s");
    let ae = AgenticError::from_generation(&e, None, 8192);
    match ae {
        AgenticError::Generation(g) => assert_eq!(g.kind, GenKind::RateLimit),
        other => panic!("expected Generation, got {other:?}"),
    }
}

#[test]
fn from_generation_classifies_network() {
    let e = gen_err("connection timed out");
    let ae = AgenticError::from_generation(&e, None, 8192);
    match ae {
        AgenticError::Generation(g) => assert_eq!(g.kind, GenKind::Network),
        other => panic!("expected Generation, got {other:?}"),
    }
}

#[test]
fn from_generation_defaults_to_provider() {
    let e = gen_err("the model refused");
    let ae = AgenticError::from_generation(&e, None, 8192);
    match ae {
        AgenticError::Generation(g) => {
            assert_eq!(g.kind, GenKind::Provider);
            assert!(g.message.contains("refused"));
        }
        other => panic!("expected Generation, got {other:?}"),
    }
}

#[test]
fn policy_maps_each_kind_to_the_right_action() {
    let p = ErrorPolicy::new();
    assert_eq!(
        p.classify(AgenticError::Generation(GenerationFailure {
            kind: GenKind::ContextOverflow,
            message: String::new(),
        })),
        AgentAction::RetryWithReducedContext
    );
    assert_eq!(
        p.classify(AgenticError::Generation(GenerationFailure {
            kind: GenKind::RateLimit,
            message: String::new(),
        })),
        AgentAction::SwitchModel
    );
    assert_eq!(
        p.classify(AgenticError::ToolCall {
            tool_name: "x".into(),
            reason: "y".into(),
        }),
        AgentAction::Continue
    );
    assert_eq!(
        p.classify(AgenticError::Budget { limit: 1 }),
        AgentAction::Terminate(AgenticError::Budget { limit: 1 })
    );
    assert!(matches!(
        p.classify(AgenticError::Generation(GenerationFailure {
            kind: GenKind::Provider,
            message: String::new(),
        })),
        AgentAction::Terminate(_)
    ));
}

#[test]
fn circuit_breaker_switches_then_exhausts() {
    let fallbacks = vec![
        ModelId::Name("backup-1".into(), None),
        ModelId::Name("backup-2".into(), None),
    ];
    let mut cb = CircuitBreaker::new(2, fallbacks);

    assert_eq!(cb.on_failure(), None);
    assert_eq!(
        cb.on_failure(),
        Some(ModelId::Name("backup-1".into(), None))
    );
    assert_eq!(cb.on_failure(), None);
    assert_eq!(
        cb.on_failure(),
        Some(ModelId::Name("backup-2".into(), None))
    );
    assert!(cb.is_exhausted());
    assert_eq!(cb.on_failure(), None);
    assert_eq!(cb.on_failure(), None);
}

#[test]
fn circuit_breaker_reset_on_success() {
    let mut cb = CircuitBreaker::new(3, vec![ModelId::Name("b".into(), None)]);
    cb.on_failure();
    cb.on_failure();
    cb.on_success();
    assert_eq!(cb.on_failure(), None);
    assert_eq!(cb.on_failure(), None);
    assert_eq!(cb.on_failure(), Some(ModelId::Name("b".into(), None)));
}
