//! Memory tools — `memory_add` / `memory_remove` / `memory_replace` (spec-60 F14).
//!
//! WHY: the `ToolShed.memory` slot was declared but shipped no `ToolImpl`, so the
//! agent could not curate its own Tier-1 working memory. These implement the
//! three curation verbs the `ToolCallManager::build_toolshed` memory assembler
//! looks for (by the `memory_*` name prefix).
//!
//! WHAT: three `ToolImpl`s over the existing [`MemoryHierarchy`] — no bespoke
//! storage. Each hydrates the latest `WorkingMemory` record, edits the fact
//! list, and writes it back via `update_working_memory` (which bumps the
//! version), so the tools reuse the same persistence path as the memory
//! generation task.
//!
//! HOW: the tools are generic over the session's `M: MemoryStore` + `D:
//! DocumentStore`, monomorphized at registration before boxing into
//! `Arc<dyn ToolImpl>`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use foundation_db::traits::DocumentStore;

use crate::agentic::memory::MemoryHierarchy;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::base_types::Args;
use crate::types::{ArgType, MemoryFact, SessionRecord, TextContent, UserModelContent};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn text_arg(args: &HashMap<String, ArgType>, key: &str, tool: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: tool.into(),
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

fn exec_err(tool: &str, reason: impl Into<String>) -> ToolError {
    ToolError::Execution {
        tool: tool.into(),
        reason: reason.into(),
    }
}

/// Read the latest working-memory facts + version for the session. A session
/// with no working memory yet starts from an empty list at version 0.
async fn current_working<M, D>(
    hierarchy: &MemoryHierarchy<M, D>,
    tool: &str,
) -> Result<(Vec<MemoryFact>, u64), ToolError>
where
    M: MemoryStore,
    D: DocumentStore,
{
    let memory = hierarchy
        .coordinator()
        .hydrate_async(hierarchy.session_id())
        .await
        .map_err(|e| exec_err(tool, format!("hydrate failed: {e}")))?;

    Ok(match memory.working {
        Some(SessionRecord::WorkingMemory { facts, version, .. }) => (facts, version),
        _ => (Vec::new(), 0),
    })
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
// memory_add
// ---------------------------------------------------------------------------

/// Append a curated fact to Tier-1 working memory.
pub struct MemoryAddTool<M, D> {
    hierarchy: Arc<MemoryHierarchy<M, D>>,
}

impl<M: MemoryStore + 'static, D: DocumentStore + 'static> MemoryAddTool<M, D> {
    #[must_use]
    pub fn new(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self {
        Self { hierarchy }
    }
}

#[async_trait]
impl<M: MemoryStore + 'static, D: DocumentStore + 'static> ToolImpl for MemoryAddTool<M, D> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_add".into(),
            description: "Add a durable fact about the user or task to working memory. \
                          Args: fact (required)."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("fact", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "memory".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let fact = text_arg(&arguments, "fact", "memory_add")?;
        let (mut facts, version) = current_working(&self.hierarchy, "memory_add").await?;
        facts.push(new_fact(fact.clone()));
        self.hierarchy
            .update_working_memory(facts, version)
            .await
            .map_err(|e| exec_err("memory_add", format!("write failed: {e}")))?;
        Ok(text_result(format!("Remembered: {fact}")))
    }
}

// ---------------------------------------------------------------------------
// memory_remove
// ---------------------------------------------------------------------------

/// Remove a fact from working memory by exact text match.
pub struct MemoryRemoveTool<M, D> {
    hierarchy: Arc<MemoryHierarchy<M, D>>,
}

impl<M: MemoryStore + 'static, D: DocumentStore + 'static> MemoryRemoveTool<M, D> {
    #[must_use]
    pub fn new(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self {
        Self { hierarchy }
    }
}

#[async_trait]
impl<M: MemoryStore + 'static, D: DocumentStore + 'static> ToolImpl for MemoryRemoveTool<M, D> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_remove".into(),
            description: "Remove a fact from working memory by its exact text. \
                          Args: fact (required)."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("fact", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "memory".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let fact = text_arg(&arguments, "fact", "memory_remove")?;
        let (facts, version) = current_working(&self.hierarchy, "memory_remove").await?;

        let before = facts.len();
        let remaining: Vec<MemoryFact> = facts.into_iter().filter(|f| f.fact != fact).collect();
        if remaining.len() == before {
            return Err(exec_err(
                "memory_remove",
                format!("no fact matching '{fact}' in working memory"),
            ));
        }

        self.hierarchy
            .update_working_memory(remaining, version)
            .await
            .map_err(|e| exec_err("memory_remove", format!("write failed: {e}")))?;
        Ok(text_result(format!("Forgot: {fact}")))
    }
}

// ---------------------------------------------------------------------------
// memory_replace
// ---------------------------------------------------------------------------

/// Replace a fact's text (correcting/updating a memory) by exact match.
pub struct MemoryReplaceTool<M, D> {
    hierarchy: Arc<MemoryHierarchy<M, D>>,
}

impl<M: MemoryStore + 'static, D: DocumentStore + 'static> MemoryReplaceTool<M, D> {
    #[must_use]
    pub fn new(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self {
        Self { hierarchy }
    }
}

#[async_trait]
impl<M: MemoryStore + 'static, D: DocumentStore + 'static> ToolImpl for MemoryReplaceTool<M, D> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_replace".into(),
            description: "Replace an existing working-memory fact with corrected text. \
                          Args: old (required, exact text), new (required)."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("old", foundation_jsonschema::scheme::string().min_len(1))
                    .required("new", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "memory".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let old = text_arg(&arguments, "old", "memory_replace")?;
        let new = text_arg(&arguments, "new", "memory_replace")?;
        let (mut facts, version) = current_working(&self.hierarchy, "memory_replace").await?;

        let mut replaced = false;
        for f in &mut facts {
            if f.fact == old {
                f.fact = new.clone();
                f.asserted_at = SystemTime::now();
                replaced = true;
            }
        }
        if !replaced {
            return Err(exec_err(
                "memory_replace",
                format!("no fact matching '{old}' in working memory"),
            ));
        }

        self.hierarchy
            .update_working_memory(facts, version)
            .await
            .map_err(|e| exec_err("memory_replace", format!("write failed: {e}")))?;
        Ok(text_result(format!("Updated memory: {old} → {new}")))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the three memory-curation tools onto a [`ToolCallManager`]. The
/// manager's `build_toolshed` then assembles the `memory` `ToolShed` slot from
/// their `memory_*` names.
///
/// [`ToolCallManager`]: crate::agentic::tool_impl::ToolCallManager
pub fn register_memory_tools<M, D>(
    manager: &crate::agentic::tool_impl::ToolCallManager,
    hierarchy: Arc<MemoryHierarchy<M, D>>,
) where
    M: MemoryStore + 'static,
    D: DocumentStore + 'static,
{
    manager.register(Arc::new(MemoryAddTool::new(hierarchy.clone())));
    manager.register(Arc::new(MemoryRemoveTool::new(hierarchy.clone())));
    manager.register(Arc::new(MemoryReplaceTool::new(hierarchy)));
}
