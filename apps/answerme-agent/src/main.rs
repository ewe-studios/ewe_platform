use foundation_ai::agentic::tool_impl::ToolCallManager;
use foundation_ai::agentic::{
    AgentConfig, AgentLoop, AgentProgress, ContextConfig, ContextProvider, KvMemoryStore,
    MemoryConfig, MemoryCoordinator, MemoryHierarchy, MessageApi, SteeringQueues, TokenLedger,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ProviderRouter, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

fn main() {
    let session_id = SessionId::new();
    let ledger = TokenLedger::new();

    let kv = KvMemoryStore::new(MemoryStorage::new());
    let memory_store = Arc::new(kv);

    let doc = MemoryDocumentStore::new();
    let message_api = MessageApi::new(session_id.clone(), doc);

    let context_provider = ContextProvider::new(
        session_id.clone(),
        message_api.clone(),
        Arc::clone(&memory_store),
        ledger.clone(),
        Some("You are a helpful assistant.".into()),
        ContextConfig::default(),
    );

    let tool_manager = ToolCallManager::new(session_id.clone());
    let queues = SteeringQueues::new();
    let priority_handle = Arc::clone(&queues.priority);
    let follow_up_handle = Arc::clone(&queues.follow_up);
    let cancel_handle = Arc::clone(&queues.cancel_signal);

    let coordinator = MemoryCoordinator::new(
        KvMemoryStore::new(MemoryStorage::new()),
        MemoryDocumentStore::new(),
    );
    let memory = MemoryHierarchy::new(
        session_id.clone(),
        coordinator,
        ledger.clone(),
        MemoryConfig::default(),
    );

    let router = ProviderRouter::builder().build();

    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .batch_size(256)
        .n_threads(2)
        .build();

    let agent = AgentLoop::new(
        session_id,
        context_provider,
        tool_manager,
        queues,
        memory,
        message_api,
        ledger.clone(),
        router,
        AgentConfig {
            primary_model: ModelId::Name("gemma-4b".into(), None),
            ..Default::default(),
        }
    );
}
