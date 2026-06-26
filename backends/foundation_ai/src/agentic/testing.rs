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

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use foundation_core::valtron::Stream;

use crate::errors::GenerationResult;
use crate::types::{
    BoxModel, CostStatus, MessageRole, Messages, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState, ModelStreamBox,
    StopReason, TextBasedFormatter, TextContent, ToolFormatter, UsageCosting, UsageReport,
    UserModelContent,
};
use crate::types::{ProviderRouter, RoutableProvider};

use super::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};

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

/// A deterministic model provider driven by `ModelInteraction` matchers.
///
/// Implements `RoutableProvider` (F12) so it plugs directly into
/// `ProviderRouter`. Responses are scripted: the first matching script wins.
pub struct MockModelProvider {
    scripts: Vec<Script>,
    failures: Vec<Failure>,
    call_count: AtomicUsize,
    name: String,
}

impl MockModelProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scripts: Vec::new(),
            failures: Vec::new(),
            call_count: AtomicUsize::new(0),
            name: "mock".into(),
        }
    }

    /// Respond with `reply` when `matcher` returns true for the interaction.
    pub fn on(
        &mut self,
        matcher: impl Fn(&ModelInteraction) -> bool + Send + Sync + 'static,
        reply: Vec<Messages>,
    ) -> &mut Self {
        self.scripts.push(Script {
            matcher: Box::new(move |mi, _| matcher(mi)),
            replies: reply,
        });
        self
    }

    /// Respond with `reply` on the `n`th call (0-indexed).
    pub fn on_nth_call(&mut self, n: usize, reply: Vec<Messages>) -> &mut Self {
        self.scripts.push(Script {
            matcher: Box::new(move |_, call| call == n),
            replies: reply,
        });
        self
    }

    /// Respond with `reply` for any interaction (catch-all, add last).
    pub fn on_any(&mut self, reply: Vec<Messages>) -> &mut Self {
        self.scripts.push(Script {
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
        self.failures.push(Failure {
            matcher: Box::new(move |mi, _| matcher(mi)),
            error: error.into(),
        });
        self
    }

    /// Number of generate/stream calls so far.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    pub fn resolve(&self, mi: &ModelInteraction) -> GenerationResult<Vec<Messages>> {
        let n = self.call_count.fetch_add(1, Ordering::Relaxed);

        for f in &self.failures {
            if (f.matcher)(mi, n) {
                return Err(crate::errors::GenerationError::Generic(f.error.clone()));
            }
        }

        for s in &self.scripts {
            if (s.matcher)(mi, n) {
                return Ok(s.replies.clone());
            }
        }

        Err(crate::errors::GenerationError::Generic(
            "MockModelProvider: no matching script for interaction".into(),
        ))
    }

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

    fn get_model(&self, _model_id: &ModelId) -> Option<BoxModel> {
        None
    }
}

// ---------------------------------------------------------------------------
// MockModel — Model impl with scripted responses
// ---------------------------------------------------------------------------

/// A `Model` that replays scripted responses from a shared `MockModelProvider`.
/// Created internally; tests interact through `MockModelProvider`.
pub struct MockModel {
    provider: Arc<MockModelProvider>,
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
        self.provider.resolve(&interaction)
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        let messages = self.provider.resolve(&interaction)?;
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
    pub fn new(items: Vec<Messages>) -> Self {
        Self {
            items,
            pos: 0,
            sent_init: false,
        }
    }
}

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
#[allow(clippy::implicit_hasher)]
pub fn mock_tool_call(name: &str, args: HashMap<String, crate::types::ArgType>) -> Messages {
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
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: s.into(),
            signature: None,
        }),
        signature: None,
    }
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

/// True when the `ToolShed` contains a tool named `name`.
pub fn tools_shed_has(name: impl Into<String>) -> impl Fn(&ModelInteraction) -> bool + Send + Sync {
    let name = name.into();
    move |mi: &ModelInteraction| mi.tools_shed.all_tools().iter().any(|t| t.name == name)
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
    pub behavior: ToolBehavior,
    attempt: AtomicU32,
}

impl MockTool {
    #[must_use]
    pub fn new(name: impl Into<String>, behavior: ToolBehavior) -> Self {
        Self {
            name: name.into(),
            description: "mock tool".into(),
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
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments: crate::types::Args::from_value(serde_json::json!({})),
            category: "mock".into(),
        }
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, crate::types::ArgType>,
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

