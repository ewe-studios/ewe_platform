use std::sync::Arc;

use foundation_ai::agentic::internals::{
    CacheStats, EmbeddingError, EmbeddingVector, NoopColdCache, ShedResult, ToolDiscovery,
    WholeTextChunker,
};
use foundation_ai::agentic::tool_impl::{
    ToolCallManager, ToolCallRequest, ToolCallResult, ToolError, ToolImpl,
};
use foundation_ai::agentic::{CachedEmbeddingProvider, EmbeddingProvider};
use foundation_ai::types::{
    BoxModel, CostStatus, ExecutionHint, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProviderDescriptor, ModelProviders, ModelSpec, ModelStreamBox, ProviderRouter,
    RoutableProvider, StopReason, ToolArguments, ToolDeclarations, ToolDefinition, UsageCosting,
    UsageReport,
};
use foundation_core::valtron::valtron_test;
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

#[valtron_test]
async fn index_and_search_finds_tool() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let def = sample_def("yaml_parser", "Parse YAML documents into structured data", "parsing");
    discovery.index(&def).await.unwrap();

    let results = discovery.search("parse YAML", 5).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "yaml_parser");
    assert_eq!(results[0].category, "parsing");
}

#[valtron_test]
async fn search_returns_empty_when_no_tools_indexed() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let results = discovery.search("anything", 5).await.unwrap();
    assert!(results.is_empty());
}

#[valtron_test]
async fn multiple_tools_indexed_and_found() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);

    for (name, description) in [
        ("tool_a", "First tool"),
        ("tool_b", "Second tool"),
        ("tool_c", "Third tool"),
    ] {
        discovery
            .index(&sample_def(name, description, "general"))
            .await
            .unwrap();
    }

    let results = discovery.search("any tool", 10).await.unwrap();
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

    async fn execute(&self, _arguments: ToolArguments) -> Result<ToolCallResult, ToolError> {
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

fn shed_request(args: ToolArguments) -> ToolCallRequest {
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

#[valtron_test]
async fn shed_returns_matching_tools_as_json_and_activates_them() {
    let mgr = manager_with(&[
        ("grep_tool", "Search for text patterns in files"),
        ("yaml_parser", "Parse YAML documents"),
    ]);
    assert_eq!(mgr.active_definitions().len(), 0);

    let args = ToolArguments::from_iter([(
        "description".to_string(),
        serde_json::Value::String("search for patterns".into()),
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

#[valtron_test]
async fn shed_respects_the_limit() {
    let mgr = manager_with(&[
        ("tool_a", "First tool"),
        ("tool_b", "Second tool"),
        ("tool_c", "Third tool"),
    ]);
    let args = ToolArguments::from_iter([
        (
            "description".to_string(),
            serde_json::Value::String("tool".into()),
        ),
        ("limit".to_string(), serde_json::json!(2)),
    ]);
    let result = mgr.execute_one(&shed_request(args)).await.unwrap();
    assert_eq!(returned_names(&result).len(), 2);
}

#[valtron_test]
async fn shed_missing_description_errors() {
    let mgr = manager_with(&[("tool_a", "First tool")]);
    let err = mgr
        .execute_one(&shed_request(ToolArguments::new()))
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

#[valtron_test]
async fn with_defaults_searches_through_discovery() {
    let discovery = make_discovery(3, vec![1.0, 0.0, 0.0]);
    let session_id = foundation_ai::types::SessionId::new();
    let mgr = ToolCallManager::with_defaults(session_id, discovery);
    mgr.register(Arc::new(NamedTool {
        name: "grep_tool",
        description: "Search for text patterns in files",
    }));

    // The fixed-vector embedder ranks every tool equally, so only embedding
    // search can return grep_tool for a query sharing no word with it.
    let hits = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(hits.index_failures, Vec::new());
    assert_eq!(
        hits.tools.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(),
        vec!["grep_tool"]
    );
    assert!(
        mgr.get("shed").is_none(),
        "shed is built in, not registered"
    );
}

/// An embedder that fails for any text that contains "broken", and embeds
/// everything else as the same unit vector.
struct PickyEmbedder;

impl EmbeddingProvider for PickyEmbedder {
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        if text.contains("broken") {
            return Err(EmbeddingError::ModelNotFound(model_id.to_string()));
        }
        Ok(EmbeddingVector {
            dimensions: 3,
            data: vec![1.0, 0.0, 0.0],
            model_id: model_id.to_string(),
        })
    }

    fn embed_batch(
        &self,
        texts: &[String],
        model_id: &str,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t, model_id)).collect()
    }

    fn register_model(&self, _model_id: &str, _dimensions: u16) {}

    fn cache_stats(&self) -> CacheStats {
        CacheStats::default()
    }

    fn clear_cache(&self) {}
}

fn picky_manager() -> ToolCallManager {
    let discovery = Arc::new(ToolDiscovery::in_memory(
        Arc::new(PickyEmbedder),
        "picky".into(),
    ));
    ToolCallManager::with_defaults(foundation_ai::types::SessionId::new(), discovery)
}

#[valtron_test]
async fn a_failed_index_update_is_reported_and_retried() {
    let mgr = picky_manager();
    mgr.register(Arc::new(NamedTool {
        name: "good_tool",
        description: "Works fine",
    }));
    mgr.register(Arc::new(NamedTool {
        name: "bad_tool",
        description: "A broken description",
    }));

    let first = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(first.index_failures.len(), 1);
    assert_eq!(first.index_failures[0].tool, "bad_tool");
    assert_eq!(
        first.tools.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(),
        vec!["good_tool"],
        "only the indexed tool is found by embedding"
    );

    // The failed update stays queued, so the next search reports it again.
    let second = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(second.index_failures.len(), 1);

    // The tool is still found by name and description.
    let by_name = mgr.search_tools("description", 5).await.unwrap();
    assert!(by_name.tools.iter().any(|h| h.name == "bad_tool"));
}

#[valtron_test]
async fn shed_reports_index_failures_to_the_model() {
    let mgr = picky_manager();
    mgr.register(Arc::new(NamedTool {
        name: "bad_tool",
        description: "A broken description",
    }));
    let args = ToolArguments::from_iter([(
        "description".to_string(),
        serde_json::Value::String("anything".into()),
    )]);
    let result = mgr.execute_one(&shed_request(args)).await.unwrap();
    let foundation_ai::types::UserModelContent::Text(tc) = &result.content else {
        panic!("expected text content");
    };
    let parsed: ShedResult = serde_json::from_str(&tc.content).unwrap();
    assert_eq!(parsed.index_failures.len(), 1);
    assert_eq!(parsed.index_failures[0].tool, "bad_tool");
}

#[valtron_test]
async fn deregister_removes_the_tool_from_the_index() {
    let mgr = picky_manager();
    mgr.register(Arc::new(NamedTool {
        name: "grep_tool",
        description: "Search for text patterns in files",
    }));
    let before = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(before.tools.len(), 1);

    mgr.deregister("grep_tool");
    let after = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(after.index_failures, Vec::new());
    assert_eq!(after.tools.len(), 0);
}

#[valtron_test]
async fn register_then_deregister_before_a_search_indexes_nothing() {
    let mgr = picky_manager();
    mgr.register(Arc::new(NamedTool {
        name: "grep_tool",
        description: "Search for text patterns in files",
    }));
    mgr.deregister("grep_tool");
    let hits = mgr.search_tools("zzz", 5).await.unwrap();
    assert_eq!(hits.index_failures, Vec::new());
    assert_eq!(hits.tools.len(), 0);
}

#[valtron_test]
async fn enable_discovery_returns_an_indexing_error() {
    let mgr = ToolCallManager::new(foundation_ai::types::SessionId::new());
    mgr.register(Arc::new(NamedTool {
        name: "bad_tool",
        description: "A broken description",
    }));
    let discovery = Arc::new(ToolDiscovery::in_memory(
        Arc::new(PickyEmbedder),
        "picky".into(),
    ));
    let err = mgr.enable_discovery(discovery).await.unwrap_err();
    assert!(matches!(err, ToolError::Execution { .. }), "{err:?}");
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
