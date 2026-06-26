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
    /// What kind of repetition was detected (e.g. "`tool_call`", "`assistant_text`").
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
    /// Matchable reason (e.g. "unauthorized", "expired", "`forbidden_tool`").
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
    /// The session token budget is exhausted — generation halts until the budget
    /// is raised (recoverable). Carries the ledger snapshot at the halt point (F04).
    BudgetExhausted {
        snapshot: crate::agentic::token_ledger::TokenSnapshot,
    },
    /// Model routing failure (no provider found, rule mismatch — F12).
    Routing(String),
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
        let kind = if last.is_some_and(|m| m.is_context_overflow(context_window)) {
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
            AgenticError::BudgetExhausted { snapshot } => write!(
                f,
                "token budget exhausted ({} / {:?} tokens)",
                snapshot.total, snapshot.budget
            ),
            AgenticError::Routing(m) => write!(f, "routing: {m}"),
            AgenticError::Unexpected(m) => write!(f, "unexpected: {m}"),
        }
    }
}

impl std::error::Error for AgenticError {}

impl From<crate::types::RouterError> for AgenticError {
    fn from(e: crate::types::RouterError) -> Self {
        AgenticError::Routing(e.to_string())
    }
}

impl AgenticError {
    /// Build a `SessionRecord::FailedAction` carrying this error and the
    /// structured projection of an `ErrorTrace` built from it.
    ///
    /// This is the boundary helper the agent loop (F19) and the model-stream
    /// lift (F03) use to put an error onto the agent stream as a record. The
    /// live `ErrorTrace` is logged via `tracing` at the failure site; only the
    /// owned `StructuredErrorTrace` projection is carried in the record (the live
    /// trace is not `Deserialize`).
    #[must_use]
    pub fn into_failed_action(self) -> crate::types::SessionRecord {
        // A single-frame trace from this error. Callers that already hold a
        // richer ErrorTrace can build FailedAction directly instead.
        let trace = foundation_errstacks::ErrorTrace::new(self.clone()).to_structured();
        crate::types::SessionRecord::FailedAction { error: self, trace }
    }
}

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
            // F17 owns loop redirect/escalation; the loop also continues.
            AgenticError::ToolCall { .. } | AgenticError::LoopDetected(_) => AgentAction::Continue,
            // Hard stops + all other generation/infrastructure failures terminate.
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

