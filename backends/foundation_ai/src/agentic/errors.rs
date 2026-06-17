//! Unified agentic error taxonomy (Feature 02).
//!
//! WHY: The agentic loop spans LLM generation, tools, memory, storage, auth,
//! and steering — each fails differently. Failures travel the agent stream as
//! `SessionRecord::FailedAction { error: AgenticError, trace: StructuredErrorTrace }`
//! (a **record**, not a `Result`), which forces `AgenticError` to be
//! `Clone + PartialEq + Debug + Serialize + Deserialize`.
//!
//! WHAT: [`AgenticError`] (the context `C` in `foundation_errstacks::ErrorTrace<C>`),
//! its boundary constructor [`AgenticError::from_generation`] (flattens the
//! non-`Clone` live `GenerationError` and classifies overflow/rate-limit by
//! detection), the [`ErrorPolicy`] → [`AgentAction`] classifier, and the
//! [`CircuitBreaker`] model-fallback decision.
//!
//! HOW: the real `GenerationError` is `Debug`-only (it wraps `BoxedError`,
//! llama.cpp, and candle errors), so it cannot be `#[from]`'d into a
//! `Clone + PartialEq` enum. Instead, at the failure boundary it is
//! `to_string()`-flattened into [`GenerationFailure`] and classified into a
//! [`GenKind`]. Context overflow is detected with `Messages::is_context_overflow`
//! (which already encodes every provider's overflow patterns); rate limits are
//! detected by string match on the formatted error (providers already format
//! and retry 429s internally). See OD-02-1 / OD-02-2.

use serde::{Deserialize, Serialize};

use crate::errors::GenerationError;
use crate::types::{Messages, ModelId};

// ============================================================================
// Forward-reference stubs (filled by F17 / F18)
// ============================================================================

/// Forward-reference stub for the F17 loop-detection result.
///
/// `AgenticError::LoopDetected` carries it; F17 owns the real type. Pinned to
/// `Clone + PartialEq + Debug + Serialize + Deserialize` so `AgenticError`
/// derives cleanly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopDetection {
    /// What kind of repetition was detected (e.g. "tool_call", "assistant_text").
    pub kind: String,
    /// How many times the repeated unit was seen.
    pub occurrences: u32,
}

/// Forward-reference stub for the F18 auth/access error.
///
/// `AgenticError::Auth` carries it; F18 owns the real type. Pinned to
/// `Clone + PartialEq + Debug + Serialize + Deserialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthError {
    /// Matchable reason (e.g. "unauthorized", "expired", "forbidden_tool").
    pub reason: String,
}

/// A user/principal identifier (forward-reference stub; F18 owns the real type).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub String);

// ============================================================================
// AgenticError
// ============================================================================

/// The unified agentic error taxonomy — the matchable context `C` in
/// `foundation_errstacks::ErrorTrace<C>`.
///
/// Every variant is `Clone + PartialEq` so the error can ride the agent stream
/// inside `SessionRecord::FailedAction`. Sources that are not themselves
/// `Clone`/`PartialEq` (notably `GenerationError`) are **flattened** to owned
/// data at the failure boundary (OD-02-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgenticError {
    /// A model generation failure, flattened from the non-`Clone` `GenerationError`.
    Generation(GenerationFailure),
    /// A tool invocation failed in a way that must terminate (the common,
    /// recoverable case is surfaced as `Messages::ToolResult` to the LLM, not here).
    ToolCall { tool_name: String, reason: String },
    /// A tool the agent tried to use is not authorized for this user.
    ToolNotAuthorized { tool_name: String, user: UserId },
    /// Message-log / `DocumentStore` failure (flattened).
    MessageStore(String),
    /// Memory tier failure (flattened).
    Memory(String),
    /// Vector store failure (flattened).
    VectorStore(String),
    /// Embedding provider failure (flattened).
    Embedding(String),
    /// Session-management failure (flattened).
    Session(String),
    /// Steering-queue failure (flattened).
    Queue(String),
    /// A repetition/loop was detected (F17 owns redirect/escalation).
    LoopDetected(LoopDetection),
    /// Authentication/access failure (F18).
    Auth(AuthError),
    /// A budget limit was exceeded.
    Budget { limit: u64 },
    /// Catch-all for anything unmapped (`ErrorTrace<T>` flattenings, etc.).
    Unexpected(String),
}

/// A model generation failure flattened to owned data at the boundary.
///
/// `GenerationError` is `Debug`-only (wraps `BoxedError`/llama.cpp/candle), so it
/// is `to_string()`-flattened here and classified into a [`GenKind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationFailure {
    /// Classified kind (drives `ErrorPolicy`).
    pub kind: GenKind,
    /// The flattened message (`GenerationError::to_string()`).
    pub message: String,
}

/// Classification of a generation failure.
///
/// `ContextOverflow`/`RateLimit` are NOT variants of the real `GenerationError`
/// (it has none) — they are detected at the boundary (OD-02-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenKind {
    /// Input exceeded the model's context window (detected via
    /// `Messages::is_context_overflow`).
    ContextOverflow,
    /// Provider returned a rate limit / 429 (detected by string match).
    RateLimit,
    /// Other provider-side failure (5xx, refusal, bad response).
    Provider,
    /// Network/transport failure.
    Network,
    /// Anything else.
    Other,
}

impl AgenticError {
    /// Boundary constructor: flatten a live `GenerationError` and classify
    /// overflow / rate-limit (OD-02-2).
    ///
    /// `last` is the most recent message (used to detect context overflow against
    /// `context_window`); pass `None` if unavailable.
    #[must_use]
    pub fn from_generation(
        error: &GenerationError,
        last: Option<&Messages>,
        context_window: u64,
    ) -> Self {
        let kind = if last
            .map(|m| m.is_context_overflow(context_window))
            .unwrap_or(false)
        {
            GenKind::ContextOverflow
        } else if detect_rate_limit(error) {
            GenKind::RateLimit
        } else if detect_network(error) {
            GenKind::Network
        } else {
            GenKind::Provider
        };
        AgenticError::Generation(GenerationFailure {
            kind,
            message: error.to_string(),
        })
    }
}

impl std::fmt::Display for AgenticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgenticError::Generation(g) => write!(f, "generation ({:?}): {}", g.kind, g.message),
            AgenticError::ToolCall { tool_name, reason } => {
                write!(f, "tool '{tool_name}' failed: {reason}")
            }
            AgenticError::ToolNotAuthorized { tool_name, user } => {
                write!(f, "tool '{tool_name}' not authorized for user '{}'", user.0)
            }
            AgenticError::MessageStore(m) => write!(f, "message store: {m}"),
            AgenticError::Memory(m) => write!(f, "memory: {m}"),
            AgenticError::VectorStore(m) => write!(f, "vector store: {m}"),
            AgenticError::Embedding(m) => write!(f, "embedding: {m}"),
            AgenticError::Session(m) => write!(f, "session: {m}"),
            AgenticError::Queue(m) => write!(f, "queue: {m}"),
            AgenticError::LoopDetected(l) => {
                write!(f, "loop detected: {} x{}", l.kind, l.occurrences)
            }
            AgenticError::Auth(a) => write!(f, "auth: {}", a.reason),
            AgenticError::Budget { limit } => write!(f, "budget exceeded (limit {limit})"),
            AgenticError::Unexpected(m) => write!(f, "unexpected: {m}"),
        }
    }
}

impl std::error::Error for AgenticError {}

/// Detect a rate-limit failure by string match on the formatted error.
///
/// Providers already format 429s ("Rate limit exceeded: …") and retry internally;
/// by the time a `GenerationError` reaches the boundary, a surviving rate-limit is
/// detectable on its message. (OD-02-2.)
fn detect_rate_limit(error: &GenerationError) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("rate limit") || msg.contains("429") || msg.contains("too many requests")
}

/// Detect a network/transport failure by string match.
fn detect_network(error: &GenerationError) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("connection")
        || msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("dns")
        || msg.contains("network")
        || msg.contains("connect")
}

// ============================================================================
// ErrorPolicy → AgentAction (Decision 16 dispatcher)
// ============================================================================

/// What the agent loop should do in response to an `AgenticError`.
///
/// The loop (F19) executes the action; F02 only decides it.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    /// Keep going (e.g. a tool error already surfaced to the LLM as a `ToolResult`).
    Continue,
    /// Re-run generation with a reduced context window (F16 repacks; F15 ensures
    /// reflections are current). Triggered by `GenKind::ContextOverflow`.
    RetryWithReducedContext,
    /// Switch to the next fallback model (via the circuit breaker / F12).
    SwitchModel,
    /// End the session, carrying the terminating error.
    Terminate(AgenticError),
}

/// Maps `AgenticError`s to `AgentAction`s (Decision 16, line 170).
///
/// Retry/backoff is owned by model + tool tasks (OD-02-4); this policy only
/// flow-controls the loop.
#[derive(Debug, Clone, Default)]
pub struct ErrorPolicy {
    _private: (),
}

impl ErrorPolicy {
    /// Create a default policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Classify an error into the loop action to take.
    #[must_use]
    pub fn classify(&self, error: AgenticError) -> AgentAction {
        match error {
            AgenticError::Generation(GenerationFailure {
                kind: GenKind::ContextOverflow,
                ..
            }) => AgentAction::RetryWithReducedContext,
            AgenticError::Generation(GenerationFailure {
                kind: GenKind::RateLimit,
                ..
            }) => AgentAction::SwitchModel,
            // A tool failure that reached here is the rare non-recoverable case;
            // the recoverable path is a ToolResult to the LLM (F11), surfaced before
            // this point. We keep going and let the LLM react.
            AgenticError::ToolCall { .. } => AgentAction::Continue,
            // F17 owns loop redirect/escalation; the loop continues.
            AgenticError::LoopDetected(_) => AgentAction::Continue,
            // Hard stops.
            AgenticError::Auth(_)
            | AgenticError::ToolNotAuthorized { .. }
            | AgenticError::Budget { .. } => AgentAction::Terminate(error),
            // Other generation failures + all infrastructure failures terminate.
            _ => AgentAction::Terminate(error),
        }
    }
}

// ============================================================================
// CircuitBreaker (Decision 16 §Circuit Breaker)
// ============================================================================

/// Tracks consecutive generation failures and, past a threshold, walks a list of
/// fallback models (resolved through the router, F12).
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failures: u32,
    threshold: u32,
    fallbacks: Vec<ModelId>,
    idx: usize,
}

impl CircuitBreaker {
    /// Create a breaker that trips after `threshold` consecutive failures and
    /// then iterates `fallbacks` (in order) on each subsequent trip.
    #[must_use]
    pub fn new(threshold: u32, fallbacks: Vec<ModelId>) -> Self {
        Self {
            failures: 0,
            threshold: threshold.max(1),
            fallbacks,
            idx: 0,
        }
    }

    /// Record a generation failure. Returns the next fallback model once the
    /// threshold is reached, or `None` if the threshold isn't reached yet OR the
    /// fallbacks are exhausted (caller should `Terminate`).
    pub fn on_failure(&mut self) -> Option<ModelId> {
        self.failures += 1;
        if self.failures < self.threshold {
            return None;
        }
        // Threshold reached — advance to the next fallback (if any).
        let next = self.fallbacks.get(self.idx).cloned();
        if next.is_some() {
            self.idx += 1;
            // Reset the failure counter so the new model gets a fresh budget.
            self.failures = 0;
        }
        next
    }

    /// Record a success — resets the consecutive-failure counter.
    pub fn on_success(&mut self) {
        self.failures = 0;
    }

    /// Whether all fallbacks have been consumed.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.idx >= self.fallbacks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // A plain provider failure terminates.
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

        // First failure: below threshold → no switch.
        assert_eq!(cb.on_failure(), None);
        // Second failure: threshold hit → first fallback.
        assert_eq!(cb.on_failure(), Some(ModelId::Name("backup-1".into(), None)));
        // Counter reset; two more failures → second fallback.
        assert_eq!(cb.on_failure(), None);
        assert_eq!(cb.on_failure(), Some(ModelId::Name("backup-2".into(), None)));
        // Now exhausted; further trips yield None → caller terminates.
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
        // Counter was reset, so two more failures still don't trip (need 3 consecutive).
        assert_eq!(cb.on_failure(), None);
        assert_eq!(cb.on_failure(), None);
        assert_eq!(cb.on_failure(), Some(ModelId::Name("b".into(), None)));
    }
}
