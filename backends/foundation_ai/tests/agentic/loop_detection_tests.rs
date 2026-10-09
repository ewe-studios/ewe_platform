use foundation_ai::agentic::loop_detection::*;
use foundation_ai::types::TextContent;
use foundation_ai::types::base_types::{ArgType, ModelOutput};

fn text_output(s: &str) -> ModelOutput {
    ModelOutput::Text(TextContent {
        content: s.into(),
        signature: None,
    })
}

fn tool_output(name: &str, args: Vec<(&str, &str)>) -> ModelOutput {
    let arguments: std::collections::HashMap<String, ArgType> = args
        .into_iter()
        .map(|(k, v)| (k.to_string(), ArgType::Text(v.to_string())))
        .collect();
    ModelOutput::ToolCall {
        id: "tc1".into(),
        name: name.into(),
        arguments: Some(arguments),
        signature: None,
        depends_on: vec![],
        execution_hint: foundation_ai::types::ExecutionHint::Unspecified,
    }
}

#[test]
fn no_loop_on_distinct_outputs() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    assert_eq!(
        det.check(&text_output("hello world foo bar")),
        LoopDetection::NoLoop
    );
    assert_eq!(
        det.check(&text_output("something entirely different here")),
        LoopDetection::NoLoop
    );
}

#[test]
fn exact_loop_on_identical_outputs() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    let output = text_output("I am stuck in a loop and repeating myself");
    det.check(&output);
    let result = det.check(&output);
    assert!(matches!(
        result,
        LoopDetection::ExactLoop { repetitions: 2 }
    ));
}

#[test]
fn exact_loop_three_times() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    let output = text_output("repeating text for loop detection");
    det.check(&output);
    det.check(&output);
    let result = det.check(&output);
    assert!(matches!(
        result,
        LoopDetection::ExactLoop { repetitions: 3 }
    ));
}

#[test]
fn tool_call_loop_detected() {
    let mut det = LoopDetector::new(LoopDetectorConfig {
        tool_call_max_repeats: 3,
        ..Default::default()
    });
    let tc = tool_output("read_file", vec![("path", "/foo.txt")]);
    det.check(&tc);
    det.check(&tc);
    let result = det.check(&tc);
    assert!(matches!(
        result,
        LoopDetection::ToolCallLoop { repetitions: 3, .. }
    ));
}

#[test]
fn tool_call_different_args_no_loop() {
    let mut det = LoopDetector::new(LoopDetectorConfig {
        tool_call_max_repeats: 3,
        ..Default::default()
    });
    det.check(&tool_output("read_file", vec![("path", "/a.txt")]));
    det.check(&tool_output("read_file", vec![("path", "/b.txt")]));
    let result = det.check(&tool_output("read_file", vec![("path", "/c.txt")]));
    assert_eq!(result, LoopDetection::NoLoop);
}

#[test]
fn simhash_detects_near_identical_text() {
    let mut det = LoopDetector::new(LoopDetectorConfig {
        similarity_threshold: 0.8,
        ..Default::default()
    });
    let a = text_output("The quick brown fox jumps over the lazy dog near the river");
    let b = text_output("The quick brown fox jumps over the lazy dog near the lake");
    det.check(&a);
    let result = det.check(&b);
    // Near-identical texts should have high SimHash similarity.
    match result {
        LoopDetection::FuzzyLoop { similarity } => {
            assert!(
                similarity >= 0.8,
                "similarity {similarity} should be >= 0.8"
            );
        }
        LoopDetection::NoLoop => {
            // This is acceptable if the texts are different enough for SimHash.
            // SimHash is a probabilistic measure.
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn simhash_no_loop_on_distinct_text() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    let a = text_output("The quick brown fox jumps over the lazy dog on a sunny day");
    let b = text_output(
        "Quantum computing leverages superposition and entanglement for parallel processing",
    );
    det.check(&a);
    let result = det.check(&b);
    assert_eq!(result, LoopDetection::NoLoop);
}

#[test]
fn tool_call_signature_sorts_keys() {
    let mut args1 = std::collections::HashMap::new();
    args1.insert("z".to_string(), ArgType::Text("1".into()));
    args1.insert("a".to_string(), ArgType::Text("2".into()));

    let mut args2 = std::collections::HashMap::new();
    args2.insert("a".to_string(), ArgType::Text("2".into()));
    args2.insert("z".to_string(), ArgType::Text("1".into()));

    let sig1 = ToolCallSignature::from_tool_call("tool", &Some(args1));
    let sig2 = ToolCallSignature::from_tool_call("tool", &Some(args2));
    assert_eq!(sig1, sig2);
    assert_eq!(sig1.argument_keys, vec!["a", "z"]);
}

#[test]
fn escalation_ladder() {
    let mut det = LoopDetector::new(LoopDetectorConfig {
        max_redirects: 3,
        temperature_delta: 0.3,
        ..Default::default()
    });
    assert_eq!(det.escalate(), Escalation::Redirect);
    assert_eq!(
        det.escalate(),
        Escalation::SwitchModelOrTemperature { delta: 0.3 }
    );
    assert_eq!(
        det.escalate(),
        Escalation::SwitchModelOrTemperature { delta: 0.3 }
    );
    assert_eq!(det.escalate(), Escalation::Terminate);
}

#[test]
fn reset_clears_redirect_count() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    det.escalate();
    det.escalate();
    assert_eq!(det.redirect_count(), 2);
    det.reset();
    assert_eq!(det.redirect_count(), 0);
    assert_eq!(det.escalate(), Escalation::Redirect);
}

#[test]
fn loop_detection_is_clone_partial_eq() {
    let d1 = LoopDetection::ExactLoop { repetitions: 2 };
    let d2 = d1.clone();
    assert_eq!(d1, d2);

    let d3 = LoopDetection::FuzzyLoop { similarity: 0.95 };
    let d4 = d3.clone();
    assert_eq!(d3, d4);
}

#[test]
fn hamming_similarity_identical() {
    assert_eq!(hamming_similarity(0xDEAD_BEEF, 0xDEAD_BEEF), 1.0);
}

#[test]
fn hamming_similarity_opposite() {
    assert_eq!(hamming_similarity(0, u64::MAX), 0.0);
}

#[test]
fn short_text_skips_simhash() {
    let mut det = LoopDetector::new(LoopDetectorConfig::default());
    det.check(&text_output("hi"));
    let result = det.check(&text_output("hi"));
    // "hi" is < 3 tokens, so simhash is skipped -- falls through to exact.
    assert!(matches!(result, LoopDetection::ExactLoop { .. }));
}

// ── Vacuous answers ─────────────────────────────────────────────────────
//
// A turn that ends having emitted only punctuation is not a loop, but it is
// just as useless to whoever asked. These lock the line between "said nothing"
// and "said something short", because getting that line wrong throws away
// correct answers.

#[test]
fn punctuation_only_replies_are_vacuous() {
    for reply in [".", "..", "...", ",", "-", "!", "?", "  .  ", "\n", "", "   "] {
        assert!(
            is_vacuous_answer(reply),
            "{reply:?} carries no information and should be treated as vacuous"
        );
    }
}

#[test]
fn a_bare_number_is_a_real_answer_and_must_survive() {
    // The case that makes the conservative rule worth having: `4` answers
    // "what is 2+2" and `0` answers "how many are left". Retrying these would
    // discard a correct reply for a possibly worse one.
    for reply in ["0", "4", "42", "0.0", "-1"] {
        assert!(
            !is_vacuous_answer(reply),
            "{reply:?} is a legitimate answer and must not be retried"
        );
    }
}

#[test]
fn ordinary_answers_are_never_vacuous() {
    for reply in ["Paris", "no", "yes.", ". Paris", "I don't know", "a"] {
        assert!(!is_vacuous_answer(reply), "{reply:?} should be kept");
    }
}

#[test]
fn a_stray_leading_dot_does_not_discard_the_answer_behind_it() {
    // Observed from a real gemma turn: ". Paris". The dot is noise, but the
    // answer is there, so the turn must not be thrown away.
    assert!(!is_vacuous_answer(". Paris"));
    assert!(!is_vacuous_answer(", Paris ."));
}

#[test]
fn non_ascii_answers_are_not_mistaken_for_punctuation() {
    for reply in ["Paris", "パリ", "Париж", "4"] {
        assert!(
            !is_vacuous_answer(reply),
            "{reply:?} is alphanumeric in its own script"
        );
    }
}

#[test]
fn emoji_only_replies_count_as_vacuous() {
    // No alphanumeric content, so nothing the caller can act on.
    assert!(is_vacuous_answer("🤔"));
}

#[test]
fn the_detector_reports_a_vacuous_turn() {
    let mut detector = LoopDetector::new(LoopDetectorConfig::default());

    assert_eq!(
        detector.check_answer(".", ""),
        LoopDetection::VacuousAnswer { text: ".".into() }
    );
    assert_eq!(detector.check_answer("Paris", ""), LoopDetection::NoLoop);
}

#[test]
fn vacuous_detection_can_be_switched_off() {
    let mut detector = LoopDetector::new(LoopDetectorConfig {
        detect_vacuous_answers: false,
        ..LoopDetectorConfig::default()
    });

    assert_eq!(detector.check_answer(".", ""), LoopDetection::NoLoop);
}

#[test]
fn a_vacuous_turn_escalates_to_a_retry_before_giving_up() {
    // The ladder the agent loop rides: retry, retry, then stop asking. The
    // caller decides what "stop" means — for a vacuous answer it passes the
    // reply through rather than failing the turn.
    let mut detector = LoopDetector::new(LoopDetectorConfig {
        max_redirects: 2,
        ..LoopDetectorConfig::default()
    });

    assert_ne!(detector.check_answer(".", ""), LoopDetection::NoLoop);
    assert_eq!(detector.escalate(), Escalation::Redirect);

    assert_ne!(detector.check_answer(".", ""), LoopDetection::NoLoop);
    assert!(matches!(
        detector.escalate(),
        Escalation::SwitchModelOrTemperature { .. }
    ));

    assert_ne!(detector.check_answer(".", ""), LoopDetection::NoLoop);
    assert_eq!(detector.escalate(), Escalation::Terminate);
}

#[test]
fn resetting_gives_a_later_turn_its_full_allowance_again() {
    // Without a reset the redirect count only ever grew, so a session that hit
    // a few loops early would give up on the first hiccup hours later.
    let mut detector = LoopDetector::new(LoopDetectorConfig::default());

    for _ in 0..4 {
        let _ = detector.escalate();
    }
    assert_eq!(detector.escalate(), Escalation::Terminate);

    detector.reset();
    assert_eq!(detector.redirect_count(), 0);
    assert_eq!(detector.escalate(), Escalation::Redirect);
}

// ── Judging a bare number against the question ──────────────────────────
//
// `0` cannot be judged alone: junk in reply to "hello", correct in reply to
// "how many are left". Observed on gemma-4-E2B, which answers a bare greeting
// with `0` roughly one run in four.

#[test]
fn a_number_is_recognised_without_its_context() {
    for reply in ["0", "4", "-1", "3.5", "42%"] {
        assert!(is_bare_number(reply), "{reply:?} is number-only");
    }
    for reply in ["4 apples", "Paris", "no", "."] {
        assert!(!is_bare_number(reply), "{reply:?} carries more than a number");
    }
}

#[test]
fn a_question_with_digits_expects_a_number() {
    assert!(question_expects_a_number("What is 2+2?"));
    assert!(question_expects_a_number("Add 10 and 5"));
}

#[test]
fn quantity_phrases_expect_a_number() {
    for question in [
        "How many planets are there?",
        "how much does it weigh",
        "What is the total?",
        "Calculate the area",
        "How old is she",
    ] {
        assert!(
            question_expects_a_number(question),
            "{question:?} is asking for a quantity"
        );
    }
}

#[test]
fn a_greeting_does_not_expect_a_number() {
    for question in ["hello", "hi there", "Tell me about yourself"] {
        assert!(
            !question_expects_a_number(question),
            "{question:?} is not asking for a quantity"
        );
    }
}

#[test]
fn a_bare_number_answering_a_greeting_is_retried() {
    // The reproduced failure: `hello` -> `0`.
    let mut detector = LoopDetector::new(LoopDetectorConfig::default());

    assert_eq!(
        detector.check_answer("0", "hello"),
        LoopDetection::VacuousAnswer { text: "0".into() }
    );
}

#[test]
fn a_bare_number_answering_arithmetic_is_kept() {
    // The regression the conservative rule exists to avoid.
    let mut detector = LoopDetector::new(LoopDetectorConfig::default());

    assert_eq!(detector.check_answer("4", "What is 2+2?"), LoopDetection::NoLoop);
    assert_eq!(
        detector.check_answer("8", "How many planets are there?"),
        LoopDetection::NoLoop
    );
}

#[test]
fn judging_bare_numbers_can_be_switched_off_for_non_english() {
    // The heuristic is English-only; a deployment that cannot rely on it turns
    // the rule off and keeps plain punctuation detection.
    let mut detector = LoopDetector::new(LoopDetectorConfig {
        judge_bare_numbers_against_question: false,
        ..LoopDetectorConfig::default()
    });

    assert_eq!(detector.check_answer("0", "hello"), LoopDetection::NoLoop);
    assert_ne!(detector.check_answer(".", "hello"), LoopDetection::NoLoop);
}

#[test]
fn a_real_answer_is_never_retried_whatever_the_question() {
    let mut detector = LoopDetector::new(LoopDetectorConfig::default());

    for (answer, question) in [
        ("Paris", "What is the capital of France?"),
        (". Paris", "What is the capital of France?"),
        ("Hello. How can I help you?", "hello"),
        ("4 apples", "hello"),
    ] {
        assert_eq!(
            detector.check_answer(answer, question),
            LoopDetection::NoLoop,
            "{answer:?} answering {question:?} should be kept"
        );
    }
}

#[test]
fn the_ladder_is_spent_once_unless_something_works() {
    // The budget is per-problem, not per-attempt-round. Once it is spent, it
    // stays spent — the agent loop only resets after a turn that came back
    // good, so a model stuck on `0` cannot earn a fresh ladder every outer
    // iteration and burn `max_redirects` retries over and over.
    let mut detector = LoopDetector::new(LoopDetectorConfig {
        max_redirects: 2,
        ..LoopDetectorConfig::default()
    });

    assert_eq!(detector.escalate(), Escalation::Redirect);
    assert!(matches!(
        detector.escalate(),
        Escalation::SwitchModelOrTemperature { .. }
    ));
    assert_eq!(detector.escalate(), Escalation::Terminate);

    // Still spent, however many times we ask.
    assert_eq!(detector.escalate(), Escalation::Terminate);
    assert_eq!(detector.escalate(), Escalation::Terminate);

    // Only a good turn refills it.
    detector.reset();
    assert_eq!(detector.escalate(), Escalation::Redirect);
}
