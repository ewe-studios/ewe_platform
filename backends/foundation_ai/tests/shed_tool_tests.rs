use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::{
    CachedEmbeddingProvider, EmbeddingProvider, NoopColdCache, ShedTool, ToolDiscovery,
    WholeTextChunker,
};
use foundation_ai::agentic::tool_impl::{ToolCallManager, ToolDefinition, ToolImpl};
use foundation_ai::types::{
    ArgType, BoxModel, CostStatus, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProviderDescriptor, ModelProviders, ModelSpec, ModelStreamBox, ProviderRouter,
    RoutableProvider, StopReason, ToolShed, UsageCosting, UsageReport,
};
use foundation_vectors::store::{InMemoryVectorStore, VectorStoreConfig};
use foundation_vectors::metric::DistanceMetric;

fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting::zero(CostStatus::Estimated),
    }
}

fn embedding_message(dims: usize, values: Vec<f32>) -> foundation_ai::types::Messages {
    foundation_ai::types::Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("mock-embed".into(), None),
        timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
        usage: zero_usage(),
        content: ModelOutput::Embedding {
            dimensions: dims,
            values,
        },
        stop_reason: StopReason::Stop,
        provider: ModelProviders::Custom("mock".into()),
        error_detail: None,
        signature: None,
        metadata: None,
    }
}

struct FixedEmbeddingModel {
    dims: usize,
    values: Vec<f32>,
}

impl foundation_ai::types::Model for FixedEmbeddingModel {
    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: "mock-embed".into(),
            id: ModelId::Name("mock-embed".into(), None),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn tool_formatter(&self) -> Box<dyn foundation_ai::types::ToolFormatter> {
        unimplemented!()
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        None
    }

    fn costing(&self) -> foundation_ai::errors::GenerationResult<UsageReport> {
        Ok(zero_usage())
    }

    fn generate(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> foundation_ai::errors::GenerationResult<Vec<foundation_ai::types::Messages>> {
        Ok(vec![embedding_message(self.dims, self.values.clone())])
    }

    fn stream(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> foundation_ai::errors::GenerationResult<ModelStreamBox> {
        unimplemented!()
    }
}

struct EmbeddingMockProvider {
    dims: usize,
    values: Vec<f32>,
}

impl RoutableProvider for EmbeddingMockProvider {
    fn name(&self) -> &str {
        "mock-embed"
    }
    fn provider_id(&self) -> ModelProviders {
        ModelProviders::Custom("mock-embed".into())
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
        Some(Box::new(FixedEmbeddingModel {
            dims: self.dims,
            values: self.values.clone(),
        }))
    }
}

fn make_embedder(dims: usize, values: Vec<f32>) -> Arc<dyn EmbeddingProvider> {
    let router = ProviderRouter::single(Box::new(EmbeddingMockProvider { dims, values }));
    Arc::new(CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
    ))
}

fn make_discovery(dims: usize, values: Vec<f32>) -> Arc<ToolDiscovery> {
    let vs_config = VectorStoreConfig::new(dims, DistanceMetric::Cosine);
    let vs: Arc<dyn foundation_vectors::store::VectorStore> =
        Arc::new(InMemoryVectorStore::new(vs_config));
    let embedder = make_embedder(dims, values);
    Arc::new(ToolDiscovery::new(vs, embedder, "mock-embed".into()))
}

fn sample_def(name: &str, desc: &str, category: &str) -> ToolDefinition {
    let schema = serde_json::json!({"type": "object"});
    let opts = foundation_jsonschema::ValidationOptions::with_schema(schema);
    ToolDefinition {
        name: name.into(),
        description: desc.into(),
        arguments: foundation_ai::types::Args::new(opts),
        category: category.into(),
    }
}

// ---------------------------------------------------------------------------
// ToolDiscovery tests
// ---------------------------------------------------------------------------

#[test]
fn index_and_search_finds_tool() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let def = sample_def("yaml_parser", "Parse YAML documents into structured data", "parsing");
    discovery.index(&def).unwrap();

    let results = discovery.search("parse YAML", 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "yaml_parser");
    assert_eq!(results[0].category, "parsing");
}

#[test]
fn search_returns_empty_when_no_tools_indexed() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let results = discovery.search("anything", 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn multiple_tools_indexed_and_found() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);

    discovery
        .index(&sample_def("tool_a", "First tool", "general"))
        .unwrap();
    discovery
        .index(&sample_def("tool_b", "Second tool", "general"))
        .unwrap();
    discovery
        .index(&sample_def("tool_c", "Third tool", "general"))
        .unwrap();

    let results = discovery.search("any tool", 10).unwrap();
    assert_eq!(results.len(), 3);
}

// ---------------------------------------------------------------------------
// ShedTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shed_tool_execute_returns_json() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    discovery
        .index(&sample_def("grep_tool", "Search for text patterns in files", "search"))
        .unwrap();

    let shed = ShedTool::new(discovery);
    let mut args = HashMap::new();
    args.insert("description".into(), ArgType::Text("search for patterns".into()));

    let result = shed.execute(args).await.unwrap();
    let content = match &result.content {
        foundation_ai::types::UserModelContent::Text(tc) => &tc.content,
        _ => panic!("expected text content"),
    };

    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "grep_tool");
}

#[tokio::test]
async fn shed_tool_missing_description_errors() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let shed = ShedTool::new(discovery);
    let args = HashMap::new();

    let err = shed.execute(args).await.unwrap_err();
    match err {
        foundation_ai::agentic::tool_impl::ToolError::InvalidArguments { tool, .. } => {
            assert_eq!(tool, "shed");
        }
        _ => panic!("expected InvalidArguments"),
    }
}

#[test]
fn shed_tool_definition_has_correct_name() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let shed = ShedTool::new(discovery);
    let def = shed.definition();
    assert_eq!(def.name, "shed");
    assert_eq!(def.category, "discovery");
}

// ---------------------------------------------------------------------------
// ToolCallManager::with_defaults
// ---------------------------------------------------------------------------

#[test]
fn with_defaults_registers_shed() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let session_id = foundation_ai::types::SessionId::new();
    let mgr = ToolCallManager::with_defaults(session_id, discovery);

    assert!(mgr.get("shed").is_some());
    assert!(mgr.names().contains(&"shed".to_string()));
}

// ---------------------------------------------------------------------------
// ToolShed::all_tools
// ---------------------------------------------------------------------------

#[test]
fn toolshed_all_tools_includes_shed() {
    let shed = ToolShed::default();
    let tools = shed.all_tools();
    assert!(tools.iter().any(|t| t.name == "shed"));
}

#[test]
fn toolshed_all_tools_empty_when_no_fields_set() {
    let shed = ToolShed {
        shed: None,
        memory: None,
        delegate: None,
        read: None,
        edit: None,
        write: None,
        search: None,
        search_files: None,
        shell: None,
    };
    let tools = shed.all_tools();
    assert!(tools.is_empty());
}
