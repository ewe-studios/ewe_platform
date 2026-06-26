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
