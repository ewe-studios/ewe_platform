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

    fn resolve(&self, mi: &ModelInteraction) -> GenerationResult<Vec<Messages>> {
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

struct MockStreamIterator {
    items: Vec<Messages>,
    pos: usize,
    sent_init: bool,
}

impl MockStreamIterator {
    fn new(items: Vec<Messages>) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolShed;

    #[test]
    fn mock_provider_returns_scripted_reply() {
        let mut mock = MockModelProvider::new();
        mock.on(|_| true, vec![mock_text("hello")]);

        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![mock_user("hi")],
            chat_template: None,
            tool_choice: None,
        };

        let result = mock.resolve(&mi).expect("should match");
        assert_eq!(result.len(), 1);
        match &result[0] {
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(t) => assert_eq!(t.content, "hello"),
                other => panic!("expected text, got {other:?}"),
            },
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn mock_provider_nth_call_matching() {
        let mut mock = MockModelProvider::new();
        mock.on_nth_call(0, vec![mock_text("first")])
            .on_nth_call(1, vec![mock_text("second")])
            .on_any(vec![mock_text("fallback")]);

        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![mock_user("test")],
            chat_template: None,
            tool_choice: None,
        };

        let r0 = mock.resolve(&mi).unwrap();
        assert!(
            matches!(&r0[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "first")
        );

        let r1 = mock.resolve(&mi).unwrap();
        assert!(
            matches!(&r1[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "second")
        );

        let r2 = mock.resolve(&mi).unwrap();
        assert!(
            matches!(&r2[0], Messages::Assistant { content: ModelOutput::Text(t), .. } if t.content == "fallback")
        );
    }

    #[test]
    fn mock_provider_failure_injection() {
        let mut mock = MockModelProvider::new();
        mock.fail_with(|_| true, "injected error");

        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![mock_user("hi")],
            chat_template: None,
            tool_choice: None,
        };

        let err = mock.resolve(&mi).unwrap_err();
        assert!(err.to_string().contains("injected error"));
    }

    #[test]
    fn mock_provider_no_match_errors() {
        let mock = MockModelProvider::new();

        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![],
            chat_template: None,
            tool_choice: None,
        };

        let err = mock.resolve(&mi).unwrap_err();
        assert!(err.to_string().contains("no matching script"));
    }

    #[test]
    fn mock_provider_call_count() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);

        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![],
            chat_template: None,
            tool_choice: None,
        };

        assert_eq!(mock.call_count(), 0);
        let _ = mock.resolve(&mi);
        assert_eq!(mock.call_count(), 1);
        let _ = mock.resolve(&mi);
        assert_eq!(mock.call_count(), 2);
    }

    #[test]
    fn last_user_contains_matcher() {
        let mi = ModelInteraction {
            system_prompt: None,
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![mock_user("fix the bug")],
            chat_template: None,
            tool_choice: None,
        };

        assert!(last_user_contains("fix")(&mi));
        assert!(!last_user_contains("deploy")(&mi));
    }

    #[test]
    fn system_prompt_contains_matcher() {
        let mi = ModelInteraction {
            system_prompt: Some("You are a coding assistant.".into()),
            soul: None,
            tools_shed: ToolShed::default(),
            messages: vec![],
            chat_template: None,
            tool_choice: None,
        };

        assert!(system_prompt_contains("coding")(&mi));
        assert!(!system_prompt_contains("chef")(&mi));
    }

    #[test]
    fn mock_text_builds_assistant_message() {
        let msg = mock_text("hello world");
        match msg {
            Messages::Assistant {
                content: ModelOutput::Text(t),
                stop_reason,
                ..
            } => {
                assert_eq!(t.content, "hello world");
                assert_eq!(stop_reason, StopReason::Stop);
            }
            other => panic!("expected Assistant text, got {other:?}"),
        }
    }

    #[test]
    fn mock_text_usage_carries_report() {
        let usage = UsageReport {
            input: 100.0,
            output: 50.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 150.0,
            cost: UsageCosting::zero(CostStatus::Actual),
        };
        let msg = mock_text_usage("hi", usage.clone());
        match msg {
            Messages::Assistant { usage: u, .. } => {
                assert_eq!(u.input, 100.0);
                assert_eq!(u.total_tokens, 150.0);
            }
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn mock_tool_call_builds_message() {
        let mut args = HashMap::new();
        args.insert("path".into(), crate::types::ArgType::Text("/tmp".into()));
        let msg = mock_tool_call("read_file", args);
        match msg {
            Messages::Assistant {
                content:
                    ModelOutput::ToolCall {
                        name, arguments, ..
                    },
                stop_reason,
                ..
            } => {
                assert_eq!(name, "read_file");
                assert!(arguments.is_some());
                assert_eq!(stop_reason, StopReason::ToolUse);
            }
            other => panic!("expected tool call, got {other:?}"),
        }
    }

    #[test]
    fn mock_tool_returning_succeeds() {
        let tool = MockTool::returning("echo", "echoed");
        let result = futures_lite::future::block_on(tool.execute(HashMap::new())).unwrap();
        match &result.content {
            UserModelContent::Text(t) => assert_eq!(t.content, "echoed"),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn mock_tool_failing_errors() {
        let tool = MockTool::failing("broken", "always fails");
        let err = futures_lite::future::block_on(tool.execute(HashMap::new())).unwrap_err();
        assert!(err.to_string().contains("always fails"));
    }

    #[test]
    fn mock_tool_fails_then_succeeds() {
        let tool = MockTool::new(
            "flaky",
            ToolBehavior::FailsThenSucceeds {
                failures: 2,
                error: ToolError::Execution {
                    tool: "flaky".into(),
                    reason: "transient".into(),
                },
                result: ToolCallResult {
                    content: UserModelContent::Text(TextContent {
                        content: "ok".into(),
                        signature: None,
                    }),
                    error_detail: None,
                },
            },
        );

        use futures_lite::future::block_on;

        assert!(block_on(tool.execute(HashMap::new())).is_err());
        assert!(block_on(tool.execute(HashMap::new())).is_err());
        assert!(block_on(tool.execute(HashMap::new())).is_ok());
        assert!(block_on(tool.execute(HashMap::new())).is_ok());
    }

    #[test]
    fn mock_provider_into_router() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("routed")]);
        let router = mock.into_router();

        assert!(router.is_single());
        let provider = router
            .resolve(&ModelId::Name("anything".into(), None))
            .unwrap();
        assert_eq!(provider.name(), "mock");
    }

    #[test]
    fn mock_stream_iterator_yields_messages() {
        let messages = vec![mock_text("a"), mock_text("b")];
        let mut iter = MockStreamIterator::new(messages);

        assert!(matches!(iter.next(), Some(Stream::Init)));
        assert!(matches!(iter.next(), Some(Stream::Next(_))));
        assert!(matches!(iter.next(), Some(Stream::Next(_))));
        assert!(iter.next().is_none());
    }

    #[test]
    fn routable_provider_serves_all() {
        let mock = MockModelProvider::new();
        assert!(mock.serves(&ModelId::Name("gpt-4".into(), None)));
        assert!(mock.serves(&ModelId::Name("claude-3".into(), None)));
    }

    #[test]
    fn tool_definition_has_correct_name() {
        let tool = MockTool::returning("my_tool", "result");
        let def = tool.definition();
        assert_eq!(def.name, "my_tool");
        assert_eq!(def.category, "mock");
    }
}
