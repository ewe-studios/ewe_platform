#![allow(unused_imports)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_ai::agentic::internals::{
    CacheStats, NoopColdCache, SentenceChunker, WholeTextChunker,
};
use foundation_ai::agentic::{CachedEmbeddingProvider, EmbeddingProvider};
use foundation_ai::types::{
    BoxModel, CostStatus, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProviderDescriptor, ModelProviders, ModelSpec, ModelStreamBox, ProviderRouter,
    RoutableProvider, StopReason, UsageCosting, UsageReport,
};
use foundation_core::valtron::{sleep_async, valtron_test};
use futures_lite::future::zip;

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
        unimplemented!("embedding model does not format tools")
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
        unimplemented!("embedding model does not stream")
    }
}

struct EmbeddingMockProvider {
    dims: usize,
    values: Vec<f32>,
}

impl RoutableProvider for EmbeddingMockProvider {
    fn name(&self) -> &'static str {
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

fn make_router(dims: usize, values: Vec<f32>) -> ProviderRouter {
    ProviderRouter::single(Box::new(EmbeddingMockProvider { dims, values }))
}

// ---------------------------------------------------------------------------
// TextChunker tests
// ---------------------------------------------------------------------------

/// The explicit real clock backend for tests.
fn test_clock() -> foundation_core::valtron::Ctx {
    foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    ))
}

#[test]
fn whole_text_chunker_returns_single_chunk() {
    let chunker = WholeTextChunker;
    let chunks = foundation_ai::agentic::internals::TextChunker::chunk(
        &chunker,
        "Hello world. Second sentence.",
    );
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "Hello world. Second sentence.");
}

#[test]
fn sentence_chunker_splits_on_punctuation() {
    let chunker = SentenceChunker;
    let chunks = foundation_ai::agentic::internals::TextChunker::chunk(
        &chunker,
        "First sentence. Second sentence! Third?",
    );
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], "First sentence.");
    assert_eq!(chunks[1], "Second sentence!");
    assert_eq!(chunks[2], "Third?");
}

#[test]
fn sentence_chunker_handles_trailing_text() {
    let chunker = SentenceChunker;
    let chunks = foundation_ai::agentic::internals::TextChunker::chunk(
        &chunker,
        "Sentence one. Fragment without ending",
    );
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "Sentence one.");
    assert_eq!(chunks[1], "Fragment without ending");
}

#[test]
fn sentence_chunker_empty_string_returns_original() {
    let chunker = SentenceChunker;
    let chunks = foundation_ai::agentic::internals::TextChunker::chunk(&chunker, "");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "");
}

// ---------------------------------------------------------------------------
// EmbeddingProvider tests
// ---------------------------------------------------------------------------

#[valtron_test]
async fn embed_returns_correct_dimensions_and_data() {
    let router = make_router(3, vec![1.0, 2.0, 3.0]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    let result = provider.embed("hello", "mock-embed").await.unwrap();
    assert_eq!(result.dimensions, 3);
    assert_eq!(result.data, vec![1.0, 2.0, 3.0]);
    assert_eq!(result.model_id, "mock-embed");
}

#[valtron_test]
async fn embed_caches_results() {
    let router = make_router(2, vec![0.5, 0.5]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    provider.embed("test text", "mock-embed").await.unwrap();
    provider.embed("test text", "mock-embed").await.unwrap();

    let stats = provider.cache_stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.size, 1);
}

#[valtron_test]
async fn embed_batch_returns_one_per_input() {
    let router = make_router(2, vec![1.0, 0.0]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    let texts = vec!["a".into(), "b".into(), "c".into()];
    let results = provider.embed_batch(&texts, "mock-embed").await.unwrap();
    assert_eq!(results.len(), 3);
    for r in &results {
        assert_eq!(r.dimensions, 2);
    }
}

#[valtron_test]
async fn register_model_enforces_dimensions() {
    let router = make_router(3, vec![1.0, 2.0, 3.0]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    provider.register_model("mock-embed", 3);
    assert!(provider.embed("ok input", "mock-embed").await.is_ok());

    let wrong_router = make_router(5, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    let wrong_provider = CachedEmbeddingProvider::new(
        wrong_router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );
    wrong_provider.register_model("mock-embed", 3);
    assert!(wrong_provider
        .embed("bad dims", "mock-embed")
        .await
        .is_err());
}

#[valtron_test]
async fn clear_cache_resets_stats() {
    let router = make_router(2, vec![0.5, 0.5]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    provider.embed("a", "mock-embed").await.unwrap();
    provider.embed("b", "mock-embed").await.unwrap();
    assert_eq!(provider.cache_stats().size, 2);

    provider.clear_cache();
    let stats = provider.cache_stats();
    assert_eq!(stats.size, 0);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
}

#[valtron_test]
async fn lru_eviction_when_cache_full() {
    let router = make_router(2, vec![1.0, 0.0]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        2,
        test_clock(),
    );

    provider.embed("first", "mock-embed").await.unwrap();
    provider.embed("second", "mock-embed").await.unwrap();
    provider.embed("third", "mock-embed").await.unwrap();

    let stats = provider.cache_stats();
    assert_eq!(stats.size, 2);
    assert_eq!(stats.evictions, 1);
}

#[valtron_test]
async fn sentence_chunker_averages_embeddings() {
    let router = make_router(2, vec![2.0, 4.0]);
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(SentenceChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    let result = provider
        .embed("First. Second.", "mock-embed")
        .await
        .unwrap();
    assert_eq!(result.dimensions, 2);
    assert_eq!(result.data, vec![2.0, 4.0]);
}

#[valtron_test]
async fn with_defaults_constructs_provider() {
    let router = make_router(4, vec![1.0, 2.0, 3.0, 4.0]);
    let provider = CachedEmbeddingProvider::with_defaults(router, test_clock());
    let result = provider
        .embed("single sentence", "mock-embed")
        .await
        .unwrap();
    assert_eq!(result.dimensions, 4);
}

// ---------------------------------------------------------------------------
// embedding_search_does_not_block_worker (E5)
// ---------------------------------------------------------------------------

/// A model whose sync `generate` blocks the caller for `wait`, but whose
/// `generate_async` parks cooperatively for the same `wait`. The embedding path
/// must use `generate_async`: a sync `generate` would hold the executor thread
/// and freeze a ticker that shares it.
struct DelayedEmbeddingModel {
    dims: usize,
    values: Vec<f32>,
    wait: Duration,
}

#[async_trait::async_trait]
impl foundation_ai::types::Model for DelayedEmbeddingModel {
    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: "delayed-embed".into(),
            id: ModelId::Name("delayed-embed".into(), None),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn tool_formatter(&self) -> Box<dyn foundation_ai::types::ToolFormatter> {
        unimplemented!("embedding model does not format tools")
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
        // The sync path blocks the calling thread, as a synchronous HTTP call
        // would. The embedding code must not take this path.
        std::thread::sleep(self.wait);
        Ok(vec![embedding_message(self.dims, self.values.clone())])
    }

    async fn generate_async(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> foundation_ai::errors::GenerationResult<Vec<foundation_ai::types::Messages>> {
        // The async path parks on a timer instead of blocking the worker, so a
        // ticker on the same thread keeps ticking while the embed "waits for I/O".
        sleep_async(self.wait).await;
        Ok(vec![embedding_message(self.dims, self.values.clone())])
    }

    fn stream(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> foundation_ai::errors::GenerationResult<ModelStreamBox> {
        unimplemented!("embedding model does not stream")
    }
}

struct DelayedEmbeddingProvider {
    dims: usize,
    values: Vec<f32>,
    wait: Duration,
}

impl RoutableProvider for DelayedEmbeddingProvider {
    fn name(&self) -> &'static str {
        "delayed-embed"
    }

    fn provider_id(&self) -> ModelProviders {
        ModelProviders::Custom("delayed-embed".into())
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
        Some(Box::new(DelayedEmbeddingModel {
            dims: self.dims,
            values: self.values.clone(),
            wait: self.wait,
        }))
    }
}

/// An embedder that answers after 300 ms must not block the worker: a ticker
/// that shares the executor thread keeps ticking while the embed waits.
#[valtron_test]
async fn embedding_search_does_not_block_worker() {
    let router = ProviderRouter::single(Box::new(DelayedEmbeddingProvider {
        dims: 3,
        values: vec![1.0, 2.0, 3.0],
        wait: Duration::from_millis(300),
    }));
    let provider = CachedEmbeddingProvider::new(
        router,
        Box::new(WholeTextChunker),
        Box::new(NoopColdCache),
        100,
        test_clock(),
    );

    // A ticker that shares the executor thread: each tick is a 5 ms cooperative
    // sleep. It runs while the embed parks out its 300 ms wait.
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = {
        let ticks = Arc::clone(&ticks);
        async move {
            for _ in 0..200 {
                ticks.fetch_add(1, Ordering::SeqCst);
                sleep_async(Duration::from_millis(5)).await;
            }
        }
    };

    let (result, ()) = zip(provider.embed("hello", "delayed-embed"), ticker).await;

    let embedding = result.expect("the delayed embedder must succeed");
    assert_eq!(embedding.dimensions, 3);
    assert_eq!(embedding.data, vec![1.0, 2.0, 3.0]);
    assert!(
        ticks.load(Ordering::SeqCst) > 20,
        "the ticker must keep ticking during the async embed, saw {} ticks",
        ticks.load(Ordering::SeqCst)
    );
}
