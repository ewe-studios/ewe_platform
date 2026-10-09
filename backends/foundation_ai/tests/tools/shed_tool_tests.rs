use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::tool_impl::{
    ToolCallManager, ToolCallRequest, ToolCallResult, ToolError, ToolImpl,
};
use foundation_ai::agentic::{
    CachedEmbeddingProvider, EmbeddingProvider, NoopColdCache, ShedResult, ToolDiscovery,
    WholeTextChunker,
};
use foundation_ai::types::{
    ArgType, BoxModel, CostStatus, ExecutionHint, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelStreamBox,
    ProviderRouter, RoutableProvider, StopReason, ToolDeclarations, ToolDefinition, UsageCosting,
    UsageReport,
};
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{InMemoryVectorStore, VectorStoreConfig};

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
        returns: None,
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
// The built-in `shed` (run by ToolCallManager)
// ---------------------------------------------------------------------------

/// A tool with a fixed name/description that just answers "ok".
struct NamedTool {
    name: &'static str,
    description: &'static str,
}

#[async_trait::async_trait]
impl ToolImpl for NamedTool {
    fn definition(&self) -> foundation_ai::types::Tool {
        foundation_ai::types::Tool::SingleCommand(sample_def(self.name, self.description, "test"))
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        Ok(ToolCallResult {
            content: foundation_ai::types::UserModelContent::Text(
                foundation_ai::types::TextContent {
                    content: "ok".into(),
                    signature: None,
                },
            ),
            error_detail: None,
        })
    }
}

fn manager_with(tools: &[(&'static str, &'static str)]) -> ToolCallManager {
    let mgr = ToolCallManager::new(foundation_ai::types::SessionId::new());
    for (name, description) in tools {
        mgr.register(Arc::new(NamedTool { name, description }));
    }
    mgr
}

fn shed_request(args: HashMap<String, ArgType>) -> ToolCallRequest {
    ToolCallRequest {
        id: "call-1".into(),
        name: "shed".into(),
        arguments: args,
        depends_on: Vec::new(),
        execution_hint: ExecutionHint::default(),
    }
}

fn returned_names(result: &ToolCallResult) -> Vec<String> {
    let content = match &result.content {
        foundation_ai::types::UserModelContent::Text(tc) => &tc.content,
        _ => panic!("expected text content"),
    };
    let parsed: ShedResult = serde_json::from_str(content).unwrap();
    parsed.tools.into_iter().map(|t| t.name).collect()
}

#[tokio::test]
async fn shed_returns_matching_tools_as_json_and_activates_them() {
    let mgr = manager_with(&[
        ("grep_tool", "Search for text patterns in files"),
        ("yaml_parser", "Parse YAML documents"),
    ]);
    assert_eq!(mgr.active_definitions().len(), 0);

    let args = HashMap::from([(
        "description".to_string(),
        ArgType::Text("search for patterns".into()),
    )]);
    let result = mgr.execute_one(&shed_request(args)).await.unwrap();

    assert_eq!(returned_names(&result), vec!["grep_tool".to_string()]);
    assert!(mgr.is_active("grep_tool"));
    assert!(!mgr.is_active("yaml_parser"));
    let offered = mgr.offered_tools();
    assert_eq!(
        offered.shed.as_ref().map(foundation_ai::types::Tool::name),
        Some("shed")
    );
    assert_eq!(
        offered
            .tools
            .iter()
            .map(foundation_ai::types::Tool::name)
            .collect::<Vec<_>>(),
        vec!["grep_tool"]
    );
}

#[tokio::test]
async fn shed_respects_the_limit() {
    let mgr = manager_with(&[
        ("tool_a", "First tool"),
        ("tool_b", "Second tool"),
        ("tool_c", "Third tool"),
    ]);
    let args = HashMap::from([
        ("description".to_string(), ArgType::Text("tool".into())),
        ("limit".to_string(), ArgType::I64(2)),
    ]);
    let result = mgr.execute_one(&shed_request(args)).await.unwrap();
    assert_eq!(returned_names(&result).len(), 2);
}

#[tokio::test]
async fn shed_missing_description_errors() {
    let mgr = manager_with(&[("tool_a", "First tool")]);
    let err = mgr
        .execute_one(&shed_request(HashMap::new()))
        .await
        .unwrap_err();
    match err {
        ToolError::InvalidArguments { tool, .. } => assert_eq!(tool, "shed"),
        other => panic!("expected InvalidArguments, got {other:?}"),
    }
}

#[test]
fn shed_definition_has_correct_name() {
    let mgr = manager_with(&[]);
    let def = mgr.shed_definition();
    assert_eq!(def.name(), "shed");
    assert_eq!(def.category().unwrap(), "discovery");
}

#[test]
fn check_offered_distinguishes_unknown_inactive_and_active_tools() {
    let mgr = manager_with(&[("tool_a", "First tool")]);
    assert_eq!(mgr.check_offered("shed"), Ok(()));
    assert_eq!(
        mgr.check_offered("tool_a"),
        Err(ToolError::NotActive("tool_a".into()))
    );
    assert_eq!(
        mgr.check_offered("nope"),
        Err(ToolError::UnknownTool("nope".into()))
    );
    mgr.activate(&["tool_a".to_string(), "nope".to_string()]);
    assert_eq!(mgr.check_offered("tool_a"), Ok(()));
    assert!(
        !mgr.is_active("nope"),
        "unregistered names are not activated"
    );
}

#[test]
fn deregister_drops_a_tool_from_the_active_set() {
    let mgr = manager_with(&[("tool_a", "First tool")]);
    mgr.activate(&["tool_a".to_string()]);
    mgr.deregister("tool_a");
    assert!(!mgr.is_active("tool_a"));
    assert_eq!(mgr.active_definitions().len(), 0);
}

// ---------------------------------------------------------------------------
// ToolCallManager::with_defaults
// ---------------------------------------------------------------------------

#[test]
fn with_defaults_searches_through_discovery() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let session_id = foundation_ai::types::SessionId::new();
    let mgr = ToolCallManager::with_defaults(session_id, discovery);
    mgr.register(Arc::new(NamedTool {
        name: "grep_tool",
        description: "Search for text patterns in files",
    }));

    // The fixed-vector embedder ranks every tool equally, so only embedding
    // search can return grep_tool for a query sharing no word with it.
    let hits = mgr.search_tools("zzz", 5).unwrap();
    assert_eq!(
        hits.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(),
        vec!["grep_tool"]
    );
    assert!(
        mgr.get("shed").is_none(),
        "shed is built in, not registered"
    );
}

// ---------------------------------------------------------------------------
// ToolDeclarations::all_tools
// ---------------------------------------------------------------------------

#[test]
fn toolshed_all_tools_includes_shed() {
    let shed = ToolDeclarations::default();
    let tools = shed.all_tools();
    assert!(tools.iter().any(|t| t.name() == "shed"));
}

#[test]
fn toolshed_all_tools_empty_when_no_fields_set() {
    let shed = ToolDeclarations {
        shed: None,
        tools: Vec::new(),
        };
    let tools = shed.all_tools();
    assert!(tools.is_empty());
}
