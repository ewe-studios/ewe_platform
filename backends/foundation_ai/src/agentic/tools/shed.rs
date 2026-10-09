//! The `shed` meta-tool: tool discovery for an agent session.
//!
//! WHY: Declaring every registered tool on every request spends context on
//! schemas the model never uses. Instead the model is offered one tool,
//! `shed`, and asks it for what it needs; every tool `shed` returns becomes
//! *active* and is declared on the following requests.
//!
//! WHAT: [`ToolDiscovery`] — embedding search over tool descriptions (an
//! embedder plus a `VectorStore`); the `shed` argument/result types
//! ([`ShedQuery`], [`ShedResult`], [`ToolSummary`]); and [`shed_definition`],
//! the `Tool` the model sees.
//!
//! HOW: `ToolCallManager` runs `shed` itself (it owns the registry and the
//! active set): it searches with `ToolDiscovery` when the session has an
//! embedder, otherwise by name/description match, then activates the hits.
//! See `ToolCallManager::search_tools` and `ToolCallManager::activate`.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::agentic::embedding::EmbeddingProvider;
use crate::agentic::tool_impl::{ToolDefinition, ToolError};
use crate::types::{Args, Tool};

use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    InMemoryVectorStore, VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig,
};
use foundation_vectors::vector::Vector;

/// The registry name of the discovery meta-tool. Reserved: a session's
/// `ToolShed` rejects a tool registered under it.
pub const SHED_TOOL_NAME: &str = "shed";

/// How many tools `shed` returns when the call doesn't say.
pub const DEFAULT_SHED_LIMIT: usize = 5;

const NAMESPACE: &str = "tools";

/// The `shed` tool as the model sees it.
#[must_use]
pub fn shed_definition() -> Tool {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "description": {
                "type": "string",
                "description": "Natural language description of the tool you need"
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of tools to return",
                "default": DEFAULT_SHED_LIMIT
            }
        },
        "required": ["description"]
    });

    Tool::SingleCommand(ToolDefinition {
        name: SHED_TOOL_NAME.into(),
        description: "Find the tools available for a task. Describe what you need; \
                      every tool returned can be called from then on. Call a tool only \
                      after `shed` has returned it."
            .into(),
        arguments: Args::new(foundation_jsonschema::ValidationOptions::with_schema(
            schema,
        )),
        category: "discovery".into(),
        returns: None,
    })
}

// ---------------------------------------------------------------------------
// ToolDiscovery — embed + index tool descriptions into a VectorStore
// ---------------------------------------------------------------------------

/// Embedding search over tool descriptions.
pub struct ToolDiscovery {
    /// Set at construction ([`new`](Self::new)) or by the first `index` call
    /// ([`in_memory`](Self::in_memory), whose dimension comes from the first
    /// embedding).
    vector_store: OnceLock<Arc<dyn VectorStore>>,
    embedder: Arc<dyn EmbeddingProvider>,
    embedding_model: String,
    summaries: std::sync::RwLock<HashMap<String, ToolSummary>>,
}

impl ToolDiscovery {
    /// Discovery over an existing vector store.
    #[must_use]
    pub fn new(
        vector_store: Arc<dyn VectorStore>,
        embedder: Arc<dyn EmbeddingProvider>,
        embedding_model: String,
    ) -> Self {
        Self {
            vector_store: OnceLock::from(vector_store),
            embedder,
            embedding_model,
            summaries: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Discovery over an in-memory (cosine) vector store, created on the first
    /// `index` call with the embedder's dimension. What an agent session uses
    /// when it is given an embedder.
    #[must_use]
    pub fn in_memory(embedder: Arc<dyn EmbeddingProvider>, embedding_model: String) -> Self {
        Self {
            vector_store: OnceLock::new(),
            embedder,
            embedding_model,
            summaries: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, ToolError> {
        self.embedder
            .embed(text, &self.embedding_model)
            .map(|v| v.data)
            .map_err(|e| ToolError::Execution {
                tool: SHED_TOOL_NAME.into(),
                reason: format!("embedding failed: {e}"),
            })
    }

    /// Index one command definition.
    /// # Errors
    /// [`ToolError::Execution`] if embedding or the vector insert fails.
    pub fn index(&self, def: &ToolDefinition) -> Result<(), ToolError> {
        self.index_tool(&Tool::SingleCommand(def.clone()))
    }

    /// Index a registered tool (single or multi command) under its name.
    /// # Errors
    /// [`ToolError::Execution`] if embedding or the vector insert fails.
    pub fn index_tool(&self, tool: &Tool) -> Result<(), ToolError> {
        let summary = ToolSummary::of(tool);
        let embedding = self.embed(&format!("{}: {}", summary.name, summary.description))?;

        let store = self.vector_store.get_or_init(|| {
            Arc::new(InMemoryVectorStore::new(VectorStoreConfig::new(
                embedding.len(),
                DistanceMetric::Cosine,
            )))
        });

        let mut metadata = VectorMetadata::default();
        metadata.tags.insert("namespace".into(), NAMESPACE.into());
        metadata
            .tags
            .insert("tool_name".into(), summary.name.clone());

        store
            .insert(
                NAMESPACE,
                VectorEntry {
                    id: format!("tool:{}", summary.name),
                    vector: Vector::new(embedding),
                    metadata,
                },
            )
            .map_err(|e| ToolError::Execution {
                tool: SHED_TOOL_NAME.into(),
                reason: format!("vector insert failed: {e}"),
            })?;

        self.summaries
            .write()
            .expect("tool summaries poisoned")
            .insert(summary.name.clone(), summary);
        Ok(())
    }

    /// Drop a tool from the index (a deregistered tool).
    /// # Errors
    /// [`ToolError::Execution`] if the vector store delete fails.
    pub fn remove(&self, name: &str) -> Result<(), ToolError> {
        let removed = self
            .summaries
            .write()
            .expect("tool summaries poisoned")
            .remove(name);
        if removed.is_none() {
            return Ok(());
        }
        match self.vector_store.get() {
            Some(store) => store
                .delete(NAMESPACE, &format!("tool:{name}"))
                .map_err(|e| ToolError::Execution {
                    tool: SHED_TOOL_NAME.into(),
                    reason: format!("vector delete failed: {e}"),
                }),
            None => Ok(()),
        }
    }

    /// The `k` indexed tools closest to `query`.
    /// # Errors
    /// [`ToolError::Execution`] if embedding the query or the search fails.
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<ToolSummary>, ToolError> {
        let Some(store) = self.vector_store.get() else {
            // Nothing indexed yet.
            return Ok(Vec::new());
        };
        let embedding = self.embed(query).map_err(|e| match e {
            ToolError::Execution { tool, reason } => ToolError::Execution {
                tool,
                reason: format!("query {reason}"),
            },
            other => other,
        })?;

        let matches = store
            .search(NAMESPACE, &embedding, k)
            .map_err(|e| ToolError::Execution {
                tool: SHED_TOOL_NAME.into(),
                reason: format!("vector search failed: {e}"),
            })?;

        let summaries = self.summaries.read().expect("tool summaries poisoned");
        Ok(matches
            .iter()
            .filter_map(|m| summaries.get(m.id.strip_prefix("tool:")?).cloned())
            .collect())
    }
}

// ---------------------------------------------------------------------------
// shed arguments and result
// ---------------------------------------------------------------------------

/// `shed`'s arguments.
#[derive(Serialize, Deserialize)]
pub struct ShedQuery {
    pub description: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    DEFAULT_SHED_LIMIT
}

/// What `shed` returns to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShedResult {
    pub tools: Vec<ToolSummary>,
}

/// One tool in a `shed` result: enough for the model to call it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub name: String,
    pub description: String,
    pub category: String,
    /// The tool's argument JSON Schema.
    pub schema: Option<serde_json::Value>,
}

impl ToolSummary {
    /// Summarise a registered tool from its declaration.
    #[must_use]
    pub fn of(tool: &Tool) -> Self {
        let spec = tool.function_spec();
        Self {
            name: spec.name,
            description: spec.description,
            category: tool.category().unwrap_or_default().to_string(),
            schema: Some(spec.parameters),
        }
    }
}
