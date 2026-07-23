//! Memory tool — a single multi-command `memory` tool (spec-60 F14 / F19).
//!
//! WHY: the agent curates its own Tier-1 working memory. Under the unified tool
//! model (F19), this is ONE `ToolImpl` registered as `memory`, exposing the
//! commands `add` / `remove` / `replace` — dispatched on the `command` argument
//! — rather than three separate tools.
//!
//! WHAT: `MemoryTool<M, D>` over the existing [`MemoryHierarchy`] — no bespoke
//! storage. Each command hydrates the latest `WorkingMemory` record, edits the
//! Tier-1 fact list, and writes it back via `update_working_memory` (version
//! bump), reusing the memory generation task's persistence path.
//!
//! HOW: generic over the session's `M: MemoryStore` + `D: DocumentStore`,
//! monomorphized at registration before boxing into `Arc<dyn ToolImpl>`.

use std::collections::HashMap;
use std::sync::Arc;
use foundation_compact::SystemTime;

use async_trait::async_trait;
use foundation_db::traits::DocumentStore;

use crate::agentic::memory::MemoryHierarchy;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::base_types::Args;
use crate::types::{
    ArgType, MemoryFact, SessionRecord, TextContent, Tool, UserModelContent,
};

const TOOL: &str = "memory";

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn text_arg(args: &HashMap<String, ArgType>, key: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: TOOL.into(),
            reason: format!("missing or invalid '{key}' argument"),
        }),
    }
}

fn text_result(content: String) -> ToolCallResult {
    ToolCallResult {
        content: UserModelContent::Text(TextContent {
            content,
            signature: None,
        }),
        error_detail: None,
    }
}

fn exec_err(reason: impl Into<String>) -> ToolError {
    ToolError::Execution {
        tool: TOOL.into(),
        reason: reason.into(),
    }
}

fn fact_arg() -> foundation_jsonschema::ValidationOptions {
    // A command taking a single required `fact` string.
    foundation_jsonschema::scheme::object()
        .required("fact", foundation_jsonschema::scheme::string().min_len(1))
        .build()
}

fn new_fact(text: String) -> MemoryFact {
    MemoryFact {
        fact: text,
        asserted_at: SystemTime::now(),
        source_message_id: foundation_compact::ids::new_scru128(),
        confidence: 1.0,
    }
}

// ---------------------------------------------------------------------------
// MemoryTool — one multi-command tool
// ---------------------------------------------------------------------------

/// The `memory` tool: `add` / `remove` / `replace` over Tier-1 working memory.
pub struct MemoryTool<M, D> {
    hierarchy: Arc<MemoryHierarchy<M, D>>,
}

impl<M: MemoryStore + 'static, D: DocumentStore + 'static> MemoryTool<M, D> {
    #[must_use]
    pub fn new(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self {
        Self { hierarchy }
    }

    /// Read the latest working-memory facts + version (empty at version 0 when
    /// there is no working memory yet).
    async fn current_working(&self) -> Result<(Vec<MemoryFact>, u64), ToolError> {
        let memory = self
            .hierarchy
            .coordinator()
            .hydrate_async(self.hierarchy.session_id())
            .await
            .map_err(|e| exec_err(format!("hydrate failed: {e}")))?;

        Ok(match memory.working {
            Some(SessionRecord::WorkingMemory { facts, version, .. }) => (facts, version),
            _ => (Vec::new(), 0),
        })
    }

    async fn write(&self, facts: Vec<MemoryFact>, version: u64) -> Result<(), ToolError> {
        self.hierarchy
            .update_working_memory(facts, version)
            .await
            .map_err(|e| exec_err(format!("write failed: {e}")))
    }

    async fn add(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let fact = text_arg(args, "fact")?;
        let (mut facts, version) = self.current_working().await?;
        facts.push(new_fact(fact.clone()));
        self.write(facts, version).await?;
        Ok(text_result(format!("Remembered: {fact}")))
    }

    async fn remove(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let fact = text_arg(args, "fact")?;
        let (facts, version) = self.current_working().await?;
        let before = facts.len();
        let remaining: Vec<MemoryFact> = facts.into_iter().filter(|f| f.fact != fact).collect();
        if remaining.len() == before {
            return Err(exec_err(format!("no fact matching '{fact}' in working memory")));
        }
        self.write(remaining, version).await?;
        Ok(text_result(format!("Forgot: {fact}")))
    }

    async fn replace(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let old = text_arg(args, "old")?;
        let new = text_arg(args, "new")?;
        let (mut facts, version) = self.current_working().await?;
        let mut replaced = false;
        for f in &mut facts {
            if f.fact == old {
                f.fact = new.clone();
                f.asserted_at = SystemTime::now();
                replaced = true;
            }
        }
        if !replaced {
            return Err(exec_err(format!("no fact matching '{old}' in working memory")));
        }
        self.write(facts, version).await?;
        Ok(text_result(format!("Updated memory: {old} → {new}")))
    }
}

#[async_trait]
impl<M: MemoryStore + 'static, D: DocumentStore + 'static> ToolImpl for MemoryTool<M, D> {
    fn definition(&self) -> Tool {
        Tool::MultiCommands(
            TOOL.to_string(),
            vec![
                ToolDefinition {
                    name: "add".into(),
                    category: TOOL.into(),
                    description: "Add a durable fact about the user or task. Args: fact.".into(),
                    arguments: Args::new(fact_arg()),
                    returns: None,
                },
                ToolDefinition {
                    name: "remove".into(),
                    category: TOOL.into(),
                    description: "Remove a fact by its exact text. Args: fact.".into(),
                    arguments: Args::new(fact_arg()),
                    returns: None,
                },
                ToolDefinition {
                    name: "replace".into(),
                    category: TOOL.into(),
                    description: "Replace a fact's text. Args: old (exact), new.".into(),
                    arguments: Args::new(
                        foundation_jsonschema::scheme::object()
                            .required("old", foundation_jsonschema::scheme::string().min_len(1))
                            .required("new", foundation_jsonschema::scheme::string().min_len(1))
                            .build(),
                    ),
                    returns: None,
                },
            ],
        )
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let command = text_arg(&arguments, "command")?;
        match command.as_str() {
            "add" => self.add(&arguments).await,
            "remove" => self.remove(&arguments).await,
            "replace" => self.replace(&arguments).await,
            other => Err(ToolError::InvalidArguments {
                tool: TOOL.into(),
                reason: format!("unknown memory command '{other}' (add|remove|replace)"),
            }),
        }
    }
}

/// Register the `memory` tool onto a [`ToolCallManager`]. `build_toolshed` then
/// surfaces it as one `MultiCommands` tool in `ToolShed.tools`.
///
/// [`ToolCallManager`]: crate::agentic::tool_impl::ToolCallManager
pub fn register_memory_tool<M, D>(
    manager: &crate::agentic::tool_impl::ToolCallManager,
    hierarchy: Arc<MemoryHierarchy<M, D>>,
) where
    M: MemoryStore + 'static,
    D: DocumentStore + 'static,
{
    manager.register(Arc::new(MemoryTool::new(hierarchy)));
}
