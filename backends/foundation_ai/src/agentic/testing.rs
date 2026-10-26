//! Deterministic test substrate (F21) — `MockModelProvider`, `MockModel`,
//! `MockTool`, message builders, and `ModelInteraction` matchers.
//!
//! WHY: Real LLMs are non-deterministic and slow. The agentic loop, memory
//! triggers, tool DAG, steering, loop detection, circuit breaker, and resume
//! all need deterministic tests with scripted model responses.
//!
//! WHAT: A `MockModelProvider` implementing `RoutableProvider` (F12), driven by
//! `Fn(&ModelInteraction) -> bool` matchers (not regex over strings). Mock tools
//! implementing `ToolImpl` (F09) with `Returns/Fails/FailsThenSucceeds` behaviors.
//! Message builder helpers for readable test setup.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use foundation_core::valtron::{Stream, StreamSpread};

use crate::errors::GenerationResult;
use crate::types::{
    BoxModel, CostStatus, Messages, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState, ModelStreamBox, StopReason,
    TextBasedFormatter, TextContent, ToolFormatter, UsageCosting, UsageReport, UserModelContent,
};
use crate::types::{ProviderRouter, RoutableProvider};

use super::tool_impl::{ToolCallResult, ToolError, ToolImpl};
use crate::types::ToolDefinition;

// ---------------------------------------------------------------------------
// Matchers — Fn(&ModelInteraction) -> bool closures
// ---------------------------------------------------------------------------

type Matcher = Box<dyn Fn(&ModelInteraction, usize) -> bool + Send + Sync>;

// ---------------------------------------------------------------------------
// MockModelProvider
// ---------------------------------------------------------------------------

struct Script {
    matcher: Matcher,
    replies: Vec<Messages>,
}

struct Failure {
    matcher: Matcher,
    error: String,
}

/// A stream that has already emitted some text and then fails in-band — the
/// wire shape the real providers produce for an error after partial output.
struct StreamFailure {
    matcher: Matcher,
    partial: Vec<Messages>,
    error: String,
}

/// A deterministic model provider driven by `ModelInteraction` matchers.
///
/// Implements `RoutableProvider` (F12) so it plugs directly into
/// `ProviderRouter`. Responses are scripted: the first matching script wins.
pub struct MockModelProvider {
    /// Shared so `RoutableProvider::get_model` can hand out a `MockModel` that
    /// resolves against the SAME scripts. Previously the state was owned
    /// outright and `get_model` returned `None`, so the mock could never drive
    /// a routed `AgentLoop` at all — every routed lookup failed with
    /// "no provider registered", which is why the loop's inner states had no
    /// coverage. See specifications/60-agentic-reliability.
    inner: Arc<MockInner>,
    name: String,
}

/// Script state shared between a `MockModelProvider` and the `MockModel`s it
/// hands to the router.
#[derive(Default)]
struct MockInner {
    scripts: RwLock<Vec<Script>>,
    failures: RwLock<Vec<Failure>>,
    stream_failures: RwLock<Vec<StreamFailure>>,
    call_count: AtomicUsize,
}

impl MockModelProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MockInner::default()),
            name: "mock".into(),
        }
    }

    /// Respond with `reply` when `matcher` returns true for the interaction.
    pub fn on(
        &mut self,
        matcher: impl Fn(&ModelInteraction) -> bool + Send + Sync + 'static,
        reply: Vec<Messages>,
    ) -> &mut Self {
        self.inner.scripts.write().expect("mock scripts poisoned").push(Script {
            matcher: Box::new(move |mi, _| matcher(mi)),
            replies: reply,
        });
        self
    }

    /// Respond with `reply` on the `n`th call (0-indexed).
    pub fn on_nth_call(&mut self, n: usize, reply: Vec<Messages>) -> &mut Self {
        self.inner.scripts.write().expect("mock scripts poisoned").push(Script {
            matcher: Box::new(move |_, call| call == n),
            replies: reply,
        });
        self
    }

    /// Respond with `reply` for any interaction (catch-all, add last).
    pub fn on_any(&mut self, reply: Vec<Messages>) -> &mut Self {
        self.inner.scripts.write().expect("mock scripts poisoned").push(Script {
            matcher: Box::new(|_, _| true),
            replies: reply,
        });
        self
    }

    /// Fail with `error` when `matcher` matches (checked before scripts).
    pub fn fail_with(
        &mut self,
        matcher: impl Fn(&ModelInteraction) -> bool + Send + Sync + 'static,
        error: impl Into<String>,
    ) -> &mut Self {
        self.inner.failures.write().expect("mock failures poisoned").push(Failure {
            matcher: Box::new(move |mi, _| matcher(mi)),
            error: error.into(),
        });
        self
    }

    /// Fail `stream()` with `error` AFTER emitting `partial` text, when
    /// `matcher` matches. This models an in-band provider error that arrives
    /// after some text has already streamed (F06): the stream yields
    /// `Spread([Done(partial), Pending(ModelState::Error(error))])`, so the
    /// loop must hand back the partial text and THEN the failure — not drop
    /// either. `generate()` is unaffected.
    pub fn stream_fails_after_partial(
        &mut self,
        matcher: impl Fn(&ModelInteraction) -> bool + Send + Sync + 'static,
        partial: Vec<Messages>,
        error: impl Into<String>,
    ) -> &mut Self {
        self.inner
            .stream_failures
            .write()
            .expect("mock stream failures poisoned")
            .push(StreamFailure {
                matcher: Box::new(move |mi, _| matcher(mi)),
                partial,
                error: error.into(),
            });
        self
    }

    /// Number of generate/stream calls so far.
    pub fn call_count(&self) -> usize {
        self.inner.call_count.load(Ordering::Relaxed)
    }

    /// # Errors
    /// Returns [`ToolError`] if the tool execution fails.
    pub fn resolve(&self, mi: &ModelInteraction) -> GenerationResult<Vec<Messages>> {
        self.inner.resolve(mi)
    }
}

impl MockInner {
    /// Take the next call number (0-indexed) for this interaction.
    fn next_call(&self) -> usize {
        self.call_count.fetch_add(1, Ordering::Relaxed)
    }

    /// Resolve scripts/failures for call `n` without bumping the counter, so a
    /// stream can check its own `stream_failures` first and fall back to the
    /// shared scripts on the same call number.
    fn resolve_at(&self, mi: &ModelInteraction, n: usize) -> GenerationResult<Vec<Messages>> {
        for f in self.failures.read().expect("mock failures poisoned").iter() {
            if (f.matcher)(mi, n) {
                return Err(crate::errors::GenerationError::Generic(f.error.clone()));
            }
        }

        for s in self.scripts.read().expect("mock scripts poisoned").iter() {
            if (s.matcher)(mi, n) {
                return Ok(s.replies.clone());
            }
        }

        Err(crate::errors::GenerationError::Generic(
            "MockModelProvider: no matching script for interaction".into(),
        ))
    }

    fn resolve(&self, mi: &ModelInteraction) -> GenerationResult<Vec<Messages>> {
        let n = self.next_call();
        self.resolve_at(mi, n)
    }
}

impl MockModelProvider {
    /// Wrap this mock into a `ProviderRouter` (single-provider mode).
    #[must_use]
    pub fn into_router(self) -> ProviderRouter {
        ProviderRouter::single(Box::new(self))
    }
}

impl Default for MockModelProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RoutableProvider for MockModelProvider
// ---------------------------------------------------------------------------

impl RoutableProvider for MockModelProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_id(&self) -> ModelProviders {
        ModelProviders::Custom("mock".into())
    }

    fn describe(&self) -> Option<ModelProviderDescriptor> {
        None
    }

    fn serves(&self, _model_id: &ModelId) -> bool {
        true
    }

    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec> {
        Some(ModelSpec {
            name: model_id.name().to_owned(),
            id: model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        })
    }

    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec> {
        self.get_one(model_id).into_iter().collect()
    }

    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel> {
        Some(Box::new(MockModel {
            inner: Arc::clone(&self.inner),
            model_id: model_id.clone(),
        }))
    }
}

// ---------------------------------------------------------------------------
// MockModel — Model impl with scripted responses
// ---------------------------------------------------------------------------

/// A `Model` that replays scripted responses from a shared `MockModelProvider`.
/// Created internally; tests interact through `MockModelProvider`.
pub struct MockModel {
    inner: Arc<MockInner>,
    model_id: ModelId,
}

impl crate::types::Model for MockModel {
    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: self.model_id.name().to_owned(),
            id: self.model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn tool_formatter(&self) -> Box<dyn ToolFormatter> {
        Box::new(TextBasedFormatter)
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        None
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        Ok(zero_usage())
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        self.inner.resolve(&interaction)
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        let n = self.inner.next_call();
        for f in self
            .inner
            .stream_failures
            .read()
            .expect("mock stream failures poisoned")
            .iter()
        {
            if (f.matcher)(&interaction, n) {
                return Ok(Box::new(MockSpreadStream::new(
                    f.partial.clone(),
                    f.error.clone(),
                )));
            }
        }
        let messages = self.inner.resolve_at(&interaction, n)?;
        Ok(Box::new(MockStreamIterator::new(messages)))
    }
}

// ---------------------------------------------------------------------------
// MockStreamIterator — replay messages as Stream items
// ---------------------------------------------------------------------------

pub struct MockStreamIterator {
    items: Vec<Messages>,
    pos: usize,
    sent_init: bool,
}

impl MockStreamIterator {
    #[must_use]
    pub fn new(items: Vec<Messages>) -> Self {
        Self {
            items,
            pos: 0,
            sent_init: false,
        }
    }
}

/// No readiness signal: this stream never yields `Wait` while waiting on
/// external I/O, so the default `wake` (`NotSupported`) is correct.
impl foundation_core::valtron::WakeableStream for MockStreamIterator {}

impl Iterator for MockStreamIterator {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.sent_init {
            self.sent_init = true;
            return Some(Stream::Init);
        }
        if self.pos < self.items.len() {
            let msg = self.items[self.pos].clone();
            self.pos += 1;
            Some(Stream::Next(msg))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// MockSpreadStream — an in-band error after partial output
// ---------------------------------------------------------------------------

/// A model stream that emits partial assistant text, then a
/// `Pending(ModelState::Error)`. This mirrors the wire shape the real providers
/// produce for an error that arrives after streaming began (F06):
/// `Spread([Done(partial), Pending(Error)])`.
pub struct MockSpreadStream {
    partial: Vec<Messages>,
    error: String,
    sent_init: bool,
    sent_spread: bool,
}

impl MockSpreadStream {
    #[must_use]
    pub fn new(partial: Vec<Messages>, error: impl Into<String>) -> Self {
        Self {
            partial,
            error: error.into(),
            sent_init: false,
            sent_spread: false,
        }
    }
}

/// No readiness signal, as with `MockStreamIterator`.
impl foundation_core::valtron::WakeableStream for MockSpreadStream {}

impl Iterator for MockSpreadStream {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.sent_init {
            self.sent_init = true;
            return Some(Stream::Init);
        }
        if !self.sent_spread {
            self.sent_spread = true;
            let mut items: Vec<StreamSpread<Messages, ModelState>> = self
                .partial
                .iter()
                .cloned()
                .map(StreamSpread::Done)
                .collect();
            items.push(StreamSpread::Pending(ModelState::Error(self.error.clone())));
            return Some(Stream::Spread(items));
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Message builder helpers
// ---------------------------------------------------------------------------

/// Build an `Assistant` text message with zeroed usage.
#[must_use]
pub fn mock_text(s: &str) -> Messages {
    mock_text_usage(s, zero_usage())
}

/// Build an `Assistant` text message with custom `UsageReport`.
#[must_use]
pub fn mock_text_usage(s: &str, usage: UsageReport) -> Messages {
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("mock".into(), None),
        timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
        usage,
        content: ModelOutput::Text(TextContent {
            content: s.into(),
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::Custom("mock".into()),
        error_detail: None,
        signature: None,
        metadata: None,
    }
}

/// Build an `Assistant` tool-call message.
#[must_use]
pub fn mock_tool_call(name: &str, args: crate::types::ToolArguments) -> Messages {
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("mock".into(), None),
        timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
        usage: zero_usage(),
        content: ModelOutput::ToolCall {
            id: format!("call_{name}"),
            name: name.into(),
            arguments: Some(args),
            signature: None,
            depends_on: Vec::new(),
            execution_hint: crate::types::ExecutionHint::Unspecified,
        },
        stop_reason: StopReason::ToolUse,
        provider: ModelProviders::Custom("mock".into()),
        error_detail: None,
        signature: None,
        metadata: None,
    }
}

/// Build a `User` text message (convenience for test prompts).
#[must_use]
pub fn mock_user(s: &str) -> Messages {
    Messages::user(s)
}

/// Zeroed `UsageReport` for test messages.
#[must_use]
pub fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting::zero(CostStatus::Estimated),
    }
}

// ---------------------------------------------------------------------------
// Standard matchers
// ---------------------------------------------------------------------------

/// True when the last `User` message content contains `needle`.
pub fn last_user_contains(needle: &str) -> impl Fn(&ModelInteraction) -> bool + Send + Sync + '_ {
    move |mi: &ModelInteraction| {
        mi.messages.iter().rev().any(|m| matches!(
            m,
            Messages::User { content: UserModelContent::Text(t), .. } if t.content.contains(needle)
        ))
    }
}

/// True when the `ToolDeclarations` contains a tool named `name`.
pub fn tools_shed_has(name: impl Into<String>) -> impl Fn(&ModelInteraction) -> bool + Send + Sync {
    let name = name.into();
    move |mi: &ModelInteraction| mi.tools_shed.all_tools().iter().any(|t| t.name() == name)
}

/// True when the system prompt contains `needle`.
pub fn system_prompt_contains(
    needle: impl Into<String>,
) -> impl Fn(&ModelInteraction) -> bool + Send + Sync {
    let needle = needle.into();
    move |mi: &ModelInteraction| {
        mi.system_prompt
            .as_ref()
            .is_some_and(|sp| sp.contains(&needle))
    }
}

// ---------------------------------------------------------------------------
// MockTool — ToolImpl with scripted behaviors
// ---------------------------------------------------------------------------

/// Scripted behavior for a `MockTool`.
pub enum ToolBehavior {
    /// Always return this result.
    Returns(ToolCallResult),
    /// Always fail with this error.
    Fails(ToolError),
    /// Fail `failures` times, then succeed (drives F11 retry tests).
    FailsThenSucceeds {
        failures: u32,
        error: ToolError,
        result: ToolCallResult,
    },
}

/// A mock tool implementing `ToolImpl` (F09) with scripted behavior.
pub struct MockTool {
    pub name: String,
    pub description: String,
    pub category: String,
    pub behavior: ToolBehavior,
    attempt: AtomicU32,
}

impl MockTool {
    #[must_use]
    pub fn new(name: impl Into<String>, behavior: ToolBehavior) -> Self {
        Self {
            name: name.into(),
            description: "mock tool".into(),
            // Default to a recognized category so the tool actually populates
            // the ToolDeclarations handed to the model (build_toolshed groups by
            // category). "mock" is not a recognized slot; "shell" is.
            category: "shell".into(),
            behavior,
            attempt: AtomicU32::new(0),
        }
    }

    /// Builder: set custom description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Builder: set the tool category (drives which `ToolDeclarations` slot it fills).
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// A tool that always returns text content.
    #[must_use]
    pub fn returning(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(
            name,
            ToolBehavior::Returns(ToolCallResult {
                content: UserModelContent::Text(TextContent {
                    content: text.into(),
                    signature: None,
                }),
                error_detail: None,
            }),
        )
    }

    /// A tool that always fails.
    #[must_use]
    pub fn failing(name: impl Into<String>, reason: impl Into<String>) -> Self {
        let n: String = name.into();
        Self::new(
            n.clone(),
            ToolBehavior::Fails(ToolError::Execution {
                tool: n,
                reason: reason.into(),
            }),
        )
    }
}

#[async_trait]
impl ToolImpl for MockTool {
    fn definition(&self) -> crate::types::Tool {
        crate::types::Tool::SingleCommand(ToolDefinition {
            name: self.name.clone(),
            category: self.category.clone(),
            description: self.description.clone(),
            arguments: crate::types::Args::from_value(serde_json::json!({})),
            returns: None,
        })
    }

    async fn execute(
        &self,
        _arguments: crate::types::ToolArguments,
    ) -> Result<ToolCallResult, ToolError> {
        match &self.behavior {
            ToolBehavior::Returns(r) => Ok(r.clone()),
            ToolBehavior::Fails(e) => Err(e.clone()),
            ToolBehavior::FailsThenSucceeds {
                failures,
                error,
                result,
            } => {
                let n = self.attempt.fetch_add(1, Ordering::Relaxed);
                if n < *failures {
                    Err(error.clone())
                } else {
                    Ok(result.clone())
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests for the mock substrate itself
// ---------------------------------------------------------------------------

