//! Loop detection — inline exact/SimHash/tool-call pattern detection (F17).
//!
//! WHY: LLMs loop — repeating text or tool calls, burning tokens. The agent
//! must detect early and escalate: redirect from memory, switch model/temp,
//! then terminate.
//!
//! WHAT: `LoopDetector` with a sliding window of `ModelOutput`. Cheap inline
//! checks (exact, `SimHash`, tool-call pattern) run synchronously every turn in
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
/// WHAT: Sliding-window size, `SimHash` similarity cutoff, tool-call repeat
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
    /// Whether a bare number is judged against the question that prompted it.
    ///
    /// A reply of `0` to "hello" is junk; the same `0` answering "how many are
    /// left" is correct. With this on, a number-only reply is only rejected
    /// when the question shows no sign of wanting a number. See
    /// [`question_expects_a_number`] for the (English-only) heuristic.
    pub judge_bare_numbers_against_question: bool,
    /// Whether a turn that produced no usable answer earns another attempt.
    ///
    /// Small models routinely end a turn with nothing but punctuation — a bare
    /// `.` or `,` — which is not a loop but is just as useless to the caller,
    /// and the remedy is the same: say so and let the model try again.
    pub detect_vacuous_answers: bool,
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
            detect_vacuous_answers: true,
            judge_bare_numbers_against_question: true,
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
    #[must_use]
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
/// match with repetition count), `FuzzyLoop` (`SimHash` similarity above
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
    /// The turn finished without saying anything usable.
    ///
    /// Carries the offending text so the caller can log what was rejected.
    VacuousAnswer {
        text: String,
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
        for (bit, count) in counts.iter_mut().enumerate() {
            if (h >> bit) & 1 == 1 {
                *count += 1;
            } else {
                *count -= 1;
            }
        }
    }
    let mut hash = 0u64;
    for (bit, count) in counts.iter().enumerate() {
        if *count > 0 {
            hash |= 1 << bit;
        }
    }
    hash
}

#[must_use]
pub fn hamming_similarity(a: u64, b: u64) -> f32 {
    let diff = (a ^ b).count_ones();
    // diff is at most 64 — fits in f32 without precision loss.
    #[allow(clippy::cast_precision_loss)]
    let result = 1.0 - (diff as f32 / 64.0);
    result
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
// Vacuous answers

/// Whether `text` is an answer in name only.
///
/// WHY: models — small ones especially — sometimes end a turn having emitted
/// nothing but a stray punctuation token. It is not a loop, but it is just as
/// useless to whoever asked, and it is worth another attempt.
///
/// WHAT: true when, after trimming, the text is empty or contains no
/// alphanumeric character at all. `.`, `...`, `,`, `-` and whitespace qualify.
///
/// HOW: deliberately conservative. A bare `0` or `4` is NOT vacuous, because it
/// is a perfectly good answer to "how many" or "what is 2+2", and discarding it
/// would burn a turn to replace a correct reply with a possibly worse one. Only
/// text carrying no alphanumeric information at all is rejected.
///
/// # Examples
///
/// ```
/// use foundation_ai::agentic::is_vacuous_answer;
///
/// assert!(is_vacuous_answer("."));
/// assert!(is_vacuous_answer("  ...  "));
/// assert!(is_vacuous_answer(""));
///
/// assert!(!is_vacuous_answer("0"));
/// assert!(!is_vacuous_answer("Paris"));
/// assert!(!is_vacuous_answer(". Paris"));
/// ```
#[must_use]
pub fn is_vacuous_answer(text: &str) -> bool {
    !text.chars().any(char::is_alphanumeric)
}

/// Whether `text` is nothing but a number.
///
/// Digits with punctuation around them (`0`, `-1`, `42%`, `3.5`) count; anything
/// carrying a letter does not, because the letters are the answer.
#[must_use]
pub fn is_bare_number(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit()) && !text.chars().any(char::is_alphabetic)
}

/// Phrases that mean the asker wants a quantity back.
///
/// English only, and deliberately short. This is a heuristic guarding a retry,
/// so a miss costs one wasted turn rather than a wrong answer — that budget does
/// not justify shipping a phrasebook.
const QUANTITY_PHRASES: [&str; 19] = [
    "how many",
    "how much",
    "how old",
    "how long",
    "how far",
    "how tall",
    "number of",
    "number",
    "integer",
    "float",
    "date",
    "count",
    "total",
    "sum",
    "percent",
    "average",
    "quantity",
    "calculate",
    "compute",
];

/// Whether `question` looks like it wants a number for an answer.
///
/// WHY it exists: a bare `0` cannot be judged on its own — it is junk in reply
/// to "hello" and correct in reply to "how many are left". The question is the
/// only context available to tell those apart.
///
/// WHAT: true when the question contains a digit, or one of a short list of
/// English quantity phrases.
///
/// HOW: substring match on the lowercased question.
///
/// # Limitations
/// English only. "¿Cuántos planetas?" answered `8` will be judged as not
/// expecting a number and earn a needless retry. Set
/// [`LoopDetectorConfig::judge_bare_numbers_against_question`] to `false` for
/// non-English deployments.
///
/// # Examples
///
/// ```
/// use foundation_ai::agentic::question_expects_a_number;
///
/// assert!(question_expects_a_number("What is 2+2?"));
/// assert!(question_expects_a_number("How many planets are there?"));
/// assert!(!question_expects_a_number("hello"));
/// ```
#[must_use]
pub fn question_expects_a_number(question: &str) -> bool {
    if question.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    let lowered = question.to_lowercase();
    QUANTITY_PHRASES
        .iter()
        .any(|phrase| lowered.contains(phrase))
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
/// text-level checks (exact match + `SimHash` fuzzy), one of
/// `ToolCallSignature` for tool-call pattern checks. Returns a
/// `LoopDetection` on each `check` and an `Escalation` via `escalate`.
///
/// HOW: `check` pushes the latest output into the windows, runs three
/// detectors in order (exact → `SimHash` → tool-call pattern), and returns
/// the first match. `escalate` counts cumulative redirects and escalates
/// through Redirect → `SwitchModel` → Terminate.
pub struct LoopDetector {
    window: VecDeque<ModelOutput>,
    tool_signatures: VecDeque<ToolCallSignature>,
    redirect_count: usize,
    config: LoopDetectorConfig,
}

impl LoopDetector {
    #[must_use]
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

    /// Check a whole turn's assembled answer for vacuity.
    ///
    /// WHY this is separate from [`LoopDetector::check`]: `check` runs per
    /// model output, and a streaming model emits one output per token. A single
    /// token is almost always "vacuous" on its own, so the question can only be
    /// asked of the assembled turn.
    ///
    /// `question` is the user's own message this turn is answering; it is what
    /// makes a bare `0` judgeable. Pass an empty string when there is none.
    pub fn check_answer(&mut self, answer: &str, question: &str) -> LoopDetection {
        if !self.config.detect_vacuous_answers {
            return LoopDetection::NoLoop;
        }

        let vacuous = is_vacuous_answer(answer)
            || (self.config.judge_bare_numbers_against_question
                && is_bare_number(answer)
                && !question_expects_a_number(question));

        if vacuous {
            return LoopDetection::VacuousAnswer {
                text: answer.to_string(),
            };
        }
        LoopDetection::NoLoop
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

    #[must_use]
    pub fn redirect_count(&self) -> usize {
        self.redirect_count
    }

    #[must_use]
    pub fn config(&self) -> &LoopDetectorConfig {
        &self.config
    }
}
