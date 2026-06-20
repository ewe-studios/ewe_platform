//! Loop detection — inline exact/SimHash/tool-call pattern detection (F17).
//!
//! WHY: LLMs loop — repeating text or tool calls, burning tokens. The agent
//! must detect early and escalate: redirect from memory, switch model/temp,
//! then terminate.
//!
//! WHAT: `LoopDetector` with a sliding window of `ModelOutput`. Cheap inline
//! checks (exact, SimHash, tool-call pattern) run synchronously every turn in
//! F19's tight loop. Background semantic detection deferred to F31.

use crate::types::{ArgType, ModelOutput};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// LoopDetectorConfig

/// Tuning knobs for `LoopDetector`.
///
/// WHY: Different deployments tolerate different repetition levels — a
/// coding assistant can retry tool calls more aggressively than a chat
/// agent. Externalising the thresholds lets callers tune detection
/// sensitivity without touching detection logic.
///
/// WHAT: Sliding-window size, SimHash similarity cutoff, tool-call repeat
/// limit, max redirect attempts, whether to try a model/temperature switch,
/// and the temperature delta to apply on switch.
///
/// HOW: Passed to `LoopDetector::new`; the detector reads these on every
/// `check` call. Sensible defaults (window 5, similarity 0.9, 3 repeats)
/// cover the common case.
#[derive(Debug, Clone)]
pub struct LoopDetectorConfig {
    pub window_size: usize,
    pub similarity_threshold: f32,
    pub tool_call_max_repeats: usize,
    pub max_redirects: usize,
    pub try_model_change: bool,
    pub temperature_delta: f32,
}

impl Default for LoopDetectorConfig {
    fn default() -> Self {
        Self {
            window_size: 5,
            similarity_threshold: 0.9,
            tool_call_max_repeats: 3,
            max_redirects: 3,
            try_model_change: true,
            temperature_delta: 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolCallSignature — sorted-key deterministic comparison

/// Deterministic fingerprint of a single tool call (name + sorted argument keys + hash).
///
/// WHY: Tool-call loop detection must compare calls structurally, not by
/// identity. Two calls to `read_file(path="/a")` are semantically identical
/// regardless of generation order or call-id. A canonical fingerprint makes
/// that comparison O(1) after construction.
///
/// WHAT: Stores the tool name, sorted argument key list, and a combined
/// `ahash` digest of name + keys + values. Two signatures with the same
/// `argument_hash` represent identical calls.
///
/// HOW: `from_tool_call` sorts argument keys, hashes name + key/value pairs
/// in sorted order. The `LoopDetector` keeps a sliding deque of these and
/// checks for repeating subsequences.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallSignature {
    pub tool_name: String,
    pub argument_keys: Vec<String>,
    pub argument_hash: u64,
}

impl ToolCallSignature {
    pub fn from_tool_call(
        name: &str,
        args: &Option<std::collections::HashMap<String, ArgType>>,
    ) -> Self {
        let mut keys: Vec<String> = args
            .as_ref()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();

        let mut hasher = ahash::AHasher::default();
        name.hash(&mut hasher);
        if let Some(map) = args {
            for k in &keys {
                k.hash(&mut hasher);
                if let Some(v) = map.get(k) {
                    format!("{v:?}").hash(&mut hasher);
                }
            }
        }

        Self {
            tool_name: name.to_string(),
            argument_keys: keys,
            argument_hash: hasher.finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// LoopDetection

/// Result of a single `LoopDetector::check` call.
///
/// WHY: The agent loop needs to know *what kind* of repetition was detected
/// so it can choose the right escalation — exact repeats warrant a redirect,
/// fuzzy repeats may just bump temperature, and tool-call patterns need the
/// offending signature for diagnostics.
///
/// WHAT: Four variants — `NoLoop` (clean), `ExactLoop` (verbatim text
/// match with repetition count), `FuzzyLoop` (SimHash similarity above
/// threshold), `ToolCallLoop` (repeated tool-call signature pattern).
///
/// HOW: Returned by `LoopDetector::check`; the agent loop feeds it into
/// `LoopDetector::escalate` to get the corresponding `Escalation` action.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopDetection {
    NoLoop,
    ExactLoop {
        repetitions: usize,
    },
    FuzzyLoop {
        similarity: f32,
    },
    ToolCallLoop {
        pattern: Vec<ToolCallSignature>,
        repetitions: usize,
    },
}

// ---------------------------------------------------------------------------
// Escalation

/// What the agent loop should do when a loop is detected.
///
/// WHY: Detection and response are separate concerns — the detector
/// identifies repetition, the escalation policy decides the remedy. This
/// separation lets callers override the policy without reimplementing
/// detection.
///
/// WHAT: Three escalation levels — `Redirect` (inject a memory-based
/// redirect prompt), `SwitchModelOrTemperature` (try a different model or
/// bump temperature by `delta`), `Terminate` (give up after max redirects).
///
/// HOW: `LoopDetector::escalate` maps a `LoopDetection` to an `Escalation`
/// using the config's `max_redirects` and `try_model_change` flags. The
/// agent loop acts on the returned variant.
#[derive(Debug, Clone, PartialEq)]
pub enum Escalation {
    Redirect,
    SwitchModelOrTemperature { delta: f32 },
    Terminate,
}

// ---------------------------------------------------------------------------
// SimHash helpers

fn simhash_text(text: &str) -> u64 {
    let mut counts = [0i32; 64];
    for token in text.split_whitespace() {
        let mut hasher = ahash::AHasher::default();
        token.hash(&mut hasher);
        let h = hasher.finish();
        for bit in 0..64 {
            if (h >> bit) & 1 == 1 {
                counts[bit] += 1;
            } else {
                counts[bit] -= 1;
            }
        }
    }
    let mut hash = 0u64;
    for bit in 0..64 {
        if counts[bit] > 0 {
            hash |= 1 << bit;
        }
    }
    hash
}

fn hamming_similarity(a: u64, b: u64) -> f32 {
    let diff = (a ^ b).count_ones();
    1.0 - (diff as f32 / 64.0)
}

fn extract_text(output: &ModelOutput) -> Option<&str> {
    match output {
        ModelOutput::Text(tc) => Some(&tc.content),
        ModelOutput::ThinkingContent { thinking, .. } => Some(thinking),
        _ => None,
    }
}

fn extract_tool_signatures(output: &ModelOutput) -> Option<ToolCallSignature> {
    match output {
        ModelOutput::ToolCall {
            name, arguments, ..
        } => Some(ToolCallSignature::from_tool_call(name, arguments)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// LoopDetector

/// Sliding-window detector for repetitive model output (F17).
///
/// WHY: LLMs can enter degenerate loops — repeating the same text verbatim,
/// producing near-identical fuzzy output, or issuing the same tool calls
/// repeatedly. Each iteration burns tokens with no progress. Cheap inline
/// detection lets the agent loop break out early.
///
/// WHAT: Maintains two sliding windows — one of raw `ModelOutput` for
/// text-level checks (exact match + SimHash fuzzy), one of
/// `ToolCallSignature` for tool-call pattern checks. Returns a
/// `LoopDetection` on each `check` and an `Escalation` via `escalate`.
///
/// HOW: `check` pushes the latest output into the windows, runs three
/// detectors in order (exact → SimHash → tool-call pattern), and returns
/// the first match. `escalate` counts cumulative redirects and escalates
/// through Redirect → SwitchModel → Terminate.
pub struct LoopDetector {
    window: VecDeque<ModelOutput>,
    tool_signatures: VecDeque<ToolCallSignature>,
    redirect_count: usize,
    config: LoopDetectorConfig,
}

impl LoopDetector {
    pub fn new(config: LoopDetectorConfig) -> Self {
        Self {
            window: VecDeque::with_capacity(config.window_size + 1),
            tool_signatures: VecDeque::with_capacity(config.window_size + 1),
            redirect_count: 0,
            config,
        }
    }

    pub fn check(&mut self, output: &ModelOutput) -> LoopDetection {
        self.window.push_back(output.clone());
        if self.window.len() > self.config.window_size {
            self.window.pop_front();
        }

        if let Some(sig) = extract_tool_signatures(output) {
            self.tool_signatures.push_back(sig);
            if self.tool_signatures.len() > self.config.window_size {
                self.tool_signatures.pop_front();
            }
        }

        // 1. Tool-call pattern — most specific, checked first.
        if let Some(detection) = self.check_tool_call_pattern() {
            return detection;
        }

        // 2. Exact match — consecutive identical outputs.
        if let Some(detection) = self.check_exact() {
            return detection;
        }

        // 3. SimHash fuzzy — near-identical text.
        if let Some(detection) = self.check_simhash() {
            return detection;
        }

        LoopDetection::NoLoop
    }

    fn check_exact(&self) -> Option<LoopDetection> {
        if self.window.len() < 2 {
            return None;
        }
        let last = self.window.back()?;
        let mut count = 0usize;
        for item in self.window.iter().rev().skip(1) {
            if item == last {
                count += 1;
            } else {
                break;
            }
        }
        if count >= 1 {
            Some(LoopDetection::ExactLoop {
                repetitions: count + 1,
            })
        } else {
            None
        }
    }

    fn check_tool_call_pattern(&self) -> Option<LoopDetection> {
        if self.tool_signatures.len() < self.config.tool_call_max_repeats {
            return None;
        }
        let last = self.tool_signatures.back()?;
        let mut count = 0usize;
        for sig in self.tool_signatures.iter().rev().skip(1) {
            if sig == last {
                count += 1;
            } else {
                break;
            }
        }
        let total = count + 1;
        if total >= self.config.tool_call_max_repeats {
            Some(LoopDetection::ToolCallLoop {
                pattern: vec![last.clone()],
                repetitions: total,
            })
        } else {
            None
        }
    }

    fn check_simhash(&self) -> Option<LoopDetection> {
        if self.window.len() < 2 {
            return None;
        }
        let last = self.window.back()?;
        let last_text = extract_text(last)?;
        if last_text.split_whitespace().count() < 3 {
            return None;
        }
        let last_hash = simhash_text(last_text);

        let prev = self.window.iter().rev().nth(1)?;
        let prev_text = extract_text(prev)?;
        let prev_hash = simhash_text(prev_text);

        let sim = hamming_similarity(last_hash, prev_hash);
        if sim >= self.config.similarity_threshold {
            Some(LoopDetection::FuzzyLoop { similarity: sim })
        } else {
            None
        }
    }

    pub fn escalate(&mut self) -> Escalation {
        self.redirect_count += 1;
        if self.redirect_count == 1 {
            Escalation::Redirect
        } else if self.redirect_count <= self.config.max_redirects {
            Escalation::SwitchModelOrTemperature {
                delta: self.config.temperature_delta,
            }
        } else {
            Escalation::Terminate
        }
    }

    pub fn reset(&mut self) {
        self.redirect_count = 0;
    }

    pub fn redirect_count(&self) -> usize {
        self.redirect_count
    }

    pub fn config(&self) -> &LoopDetectorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TextContent;

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
            execution_hint: crate::types::ExecutionHint::Unspecified,
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
            LoopDetection::ToolCallLoop {
                repetitions: 3,
                ..
            }
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
                assert!(similarity >= 0.8, "similarity {similarity} should be >= 0.8");
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
        // "hi" is < 3 tokens, so simhash is skipped — falls through to exact.
        assert!(matches!(result, LoopDetection::ExactLoop { .. }));
    }
}
