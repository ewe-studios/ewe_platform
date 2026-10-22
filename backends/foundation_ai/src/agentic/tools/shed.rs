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
//!
//! The vector store is async (`foundation_vectors::VectorStore`), but
//! `ToolCallManager::register` / `deregister` are sync. So a registry change
//! is queued in `ToolDiscovery` (see [`ToolDiscovery::queue_index`]), and the
//! next async search applies the queue first. A queued change that fails is
//! kept for the next search and reported in [`ShedResult::index_failures`].

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::agentic::embedding::EmbeddingProvider;
use crate::agentic::tool_impl::ToolError;
use crate::types::ToolDefinition;
use crate::types::{Args, Tool};

use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    InMemoryVectorStore, VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig,
    VectorStoreError,
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

/// A registry change that waits for the next async search.
enum PendingChange {
    Index(Box<Tool>),
    Remove,
}

/// Embedding search over tool descriptions.
pub struct ToolDiscovery {
    /// Set at construction ([`new`](Self::new)) or by the first `index` call
    /// ([`in_memory`](Self::in_memory), whose dimension comes from the first
    /// embedding).
    vector_store: OnceLock<Arc<dyn VectorStore>>,
    embedder: Arc<dyn EmbeddingProvider>,
    embedding_model: String,
    summaries: std::sync::RwLock<HashMap<String, ToolSummary>>,
    /// Registry changes not yet applied to the vector store, by tool name.
    /// Only the last change per name is kept, so the queue holds at most one
    /// entry per registered tool.
    pending: Mutex<BTreeMap<String, PendingChange>>,
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
            pending: Mutex::new(BTreeMap::new()),
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
            pending: Mutex::new(BTreeMap::new()),
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
    pub async fn index(&self, def: &ToolDefinition) -> Result<(), ToolError> {
        self.index_tool(&Tool::SingleCommand(def.clone())).await
    }

    /// Index a registered tool (single or multi command) under its name.
    /// # Errors
    /// [`ToolError::Execution`] if embedding or the vector insert fails.
    pub async fn index_tool(&self, tool: &Tool) -> Result<(), ToolError> {
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
            .insert_async(
                NAMESPACE,
                VectorEntry {
                    id: format!("tool:{}", summary.name),
                    vector: Vector::new(embedding),
                    metadata,
                },
            )
            .await
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

    /// Drop a tool from the index (a deregistered tool). A tool that was
    /// never indexed is not an error.
    /// # Errors
    /// [`ToolError::Execution`] if the vector store delete fails.
    pub async fn remove(&self, name: &str) -> Result<(), ToolError> {
        let removed = self
            .summaries
            .write()
            .expect("tool summaries poisoned")
            .remove(name);
        if removed.is_none() {
            return Ok(());
        }
        let Some(store) = self.vector_store.get() else {
            return Ok(());
        };
        match store.delete_async(NAMESPACE, &format!("tool:{name}")).await {
            Ok(()) | Err(VectorStoreError::NotFound { .. }) => Ok(()),
            Err(e) => Err(ToolError::Execution {
                tool: SHED_TOOL_NAME.into(),
                reason: format!("vector delete failed: {e}"),
            }),
        }
    }

    /// Queue `tool` for indexing. Sync, for `ToolCallManager::register`; the
    /// next [`apply_pending`](Self::apply_pending) (which every
    /// `ToolCallManager::search_tools` runs first) indexes it. A later queued
    /// change for the same name replaces this one.
    pub fn queue_index(&self, tool: &Tool) {
        let name = tool.function_spec().name;
        self.pending
            .lock()
            .expect("discovery queue poisoned")
            .insert(name, PendingChange::Index(Box::new(tool.clone())));
    }

    /// Queue the removal of `name` from the index. Sync, for
    /// `ToolCallManager::deregister`; see [`queue_index`](Self::queue_index).
    pub fn queue_remove(&self, name: &str) {
        self.pending
            .lock()
            .expect("discovery queue poisoned")
            .insert(name.to_string(), PendingChange::Remove);
    }

    /// Apply the queued registry changes to the vector store.
    ///
    /// Each change that fails stays queued (unless a newer change for the same
    /// tool arrived meanwhile), so the next call tries it again. A tool that
    /// is not indexed stays findable by name and description.
    ///
    /// Returns one [`IndexFailure`] per change that failed. The queue holds at
    /// most one change per tool name, so it is bounded by the number of
    /// tools.
    pub async fn apply_pending(&self) -> Vec<IndexFailure> {
        let drained = std::mem::take(&mut *self.pending.lock().expect("discovery queue poisoned"));
        let mut failures = Vec::new();
        for (name, change) in drained {
            let result = match &change {
                PendingChange::Index(tool) => self.index_tool(tool).await,
                PendingChange::Remove => self.remove(&name).await,
            };
            if let Err(err) = result {
                tracing::warn!(tool = %name, "updating the `{SHED_TOOL_NAME}` index failed: {err}");
                self.pending
                    .lock()
                    .expect("discovery queue poisoned")
                    .entry(name.clone())
                    .or_insert(change);
                failures.push(IndexFailure {
                    tool: name,
                    reason: err.to_string(),
                });
            }
        }
        failures
    }

    /// The `k` indexed tools closest to `query`.
    /// # Errors
    /// [`ToolError::Execution`] if embedding the query or the search fails.
    pub async fn search(&self, query: &str, k: usize) -> Result<Vec<ToolSummary>, ToolError> {
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
            .search_async(NAMESPACE, &embedding, k)
            .await
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShedResult {
    pub tools: Vec<ToolSummary>,
    /// Tools whose embedding-index update failed during this search. They
    /// are still found by name and description, and the update is tried
    /// again on the next search. Empty (and left out of the JSON) normally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub index_failures: Vec<IndexFailure>,
}

/// A queued embedding-index update that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFailure {
    /// The tool's registry name.
    pub tool: String,
    /// Why the update failed.
    pub reason: String,
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
