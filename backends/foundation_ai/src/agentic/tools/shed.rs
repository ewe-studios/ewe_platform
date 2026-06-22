use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agentic::embedding::EmbeddingProvider;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::{ArgType, TextContent, UserModelContent};

use foundation_vectors::store::{VectorEntry, VectorMetadata, VectorStore};
use foundation_vectors::vector::Vector;

// ---------------------------------------------------------------------------
// ToolDiscovery — embed + index tool descriptions into VectorStore
// ---------------------------------------------------------------------------

pub struct ToolDiscovery {
    vector_store: Arc<dyn VectorStore>,
    embedder: Arc<dyn EmbeddingProvider>,
    embedding_model: String,
    summaries: std::sync::RwLock<HashMap<String, ToolSummary>>,
}

impl ToolDiscovery {
    #[must_use]
    pub fn new(
        vector_store: Arc<dyn VectorStore>,
        embedder: Arc<dyn EmbeddingProvider>,
        embedding_model: String,
    ) -> Self {
        Self {
            vector_store,
            embedder,
            embedding_model,
            summaries: std::sync::RwLock::new(HashMap::new()),
        }
    }

    pub fn index(&self, def: &ToolDefinition) -> Result<(), ToolError> {
        let text = format!("{}: {}", def.name, def.description);
        let embedding = self
            .embedder
            .embed(&text, &self.embedding_model)
            .map_err(|e| ToolError::Execution {
                tool: "shed".into(),
                reason: format!("embedding failed: {e}"),
            })?;

        let mut metadata = VectorMetadata::default();
        metadata
            .tags
            .insert("namespace".into(), "tools".into());
        metadata
            .tags
            .insert("tool_name".into(), def.name.clone());

        let entry = VectorEntry {
            id: format!("tool:{}", def.name),
            vector: Vector::new(embedding.data),
            metadata,
        };

        self.vector_store
            .insert(entry)
            .map_err(|e| ToolError::Execution {
                tool: "shed".into(),
                reason: format!("vector insert failed: {e}"),
            })?;

        let schema = serde_json::to_value(&def.arguments.schema).ok();

        let summary = ToolSummary {
            name: def.name.clone(),
            description: def.description.clone(),
            category: def.category.clone(),
            schema,
        };

        self.summaries
            .write()
            .unwrap()
            .insert(def.name.clone(), summary);

        Ok(())
    }

    pub fn search(&self, query: &str, k: usize) -> Result<Vec<ToolSummary>, ToolError> {
        let embedding = self
            .embedder
            .embed(query, &self.embedding_model)
            .map_err(|e| ToolError::Execution {
                tool: "shed".into(),
                reason: format!("query embedding failed: {e}"),
            })?;

        let matches = self
            .vector_store
            .search(&embedding.data, k)
            .map_err(|e| ToolError::Execution {
                tool: "shed".into(),
                reason: format!("vector search failed: {e}"),
            })?;

        let summaries = self.summaries.read().unwrap();
        let results: Vec<ToolSummary> = matches
            .iter()
            .filter_map(|m| {
                let tool_name = m.id.strip_prefix("tool:")?;
                summaries.get(tool_name).cloned()
            })
            .collect();

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// ShedTool — ToolImpl that does vector-backed tool discovery
// ---------------------------------------------------------------------------

pub struct ShedTool {
    discovery: Arc<ToolDiscovery>,
}

impl ShedTool {
    #[must_use]
    pub fn new(discovery: Arc<ToolDiscovery>) -> Self {
        Self { discovery }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ShedQuery {
    pub description: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShedResult {
    pub tools: Vec<ToolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub name: String,
    pub description: String,
    pub category: String,
    pub schema: Option<serde_json::Value>,
}

#[async_trait]
impl ToolImpl for ShedTool {
    fn definition(&self) -> ToolDefinition {
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
                    "default": 5
                }
            },
            "required": ["description"]
        });

        let opts = foundation_jsonschema::ValidationOptions::with_schema(schema);
        let args = crate::types::Args::new(opts);

        ToolDefinition {
            name: "shed".into(),
            description: "Search the tool registry for available tools by natural language description. Use this to discover what tools are available for a task.".into(),
            arguments: args,
            category: "discovery".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let description = match arguments.get("description") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => {
                return Err(ToolError::InvalidArguments {
                    tool: "shed".into(),
                    reason: "missing required argument: description".into(),
                })
            }
        };

        let limit = match arguments.get("limit") {
            Some(ArgType::Usize(n)) => *n,
            Some(ArgType::U64(n)) => *n as usize,
            Some(ArgType::I64(n)) => *n as usize,
            _ => default_limit(),
        };

        let hits = self.discovery.search(&description, limit)?;
        let result = ShedResult { tools: hits };
        let json = serde_json::to_string(&result).map_err(|e| ToolError::Execution {
            tool: "shed".into(),
            reason: format!("serialization failed: {e}"),
        })?;

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: json,
                signature: None,
            }),
            error_detail: None,
        })
    }
}
