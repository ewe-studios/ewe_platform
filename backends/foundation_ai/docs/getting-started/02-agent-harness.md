# Getting Started: `foundation_ai` Agent Harness

Guide to setting up and using the **agent system** with different providers:
Llama.cpp (local GGUF), OpenAI, Anthropic (Claude), and OpenRouter. This doc
focuses on the agent lifecycle — building sessions, running turns, steering,
memory, tools, and resume.

Every section shows the **harness** shortcut *and* the **manual** (no-helpers)
equivalent so you understand what the harness is doing and can build from
scratch.

For provider configuration details, see **Doc 00-1** (Getting Started: Providers).

---

## 1. The Agent System — What It Is

An `AgentSession<D, M>` is the single public handle for a conversation. It
orchestrates:

| Component | Role |
|-----------|------|
| `ProviderRouter` | Selects which provider serves each model |
| `ToolCallManager` | Registers and executes tools |
| `MemoryHierarchy` | Working / Observation / Reflection memory tiers |
| `ContextProvider` | Assembles prompts from memory + history |
| `TokenLedger` | Tracks and enforces token budgets |
| `LoopDetector` | Detects repetition and escalates |
| `SteeringQueues` | Inject priority and follow-up messages mid-turn |

The session is generic over two types:
- **`D: DocumentStore`** — stores the message history (conversation log)
- **`M: MemoryStore`** — caches the latest memory record per tier per session

---

## 2. Building an Agent Session

### 2.1. Fastest Path: Harness Presets

```rust
use foundation_ai::harness;
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::SessionId;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

// Anthropic: Claude Opus + Sonnet
let builder = harness::claude_session::<Doc, Mem>(SessionId::new(), &anthropic_key)?;

// OpenAI: GPT-4o + GPT-4o-mini (Chat Completions)
let builder = harness::openai_chat_session::<Doc, Mem>(SessionId::new(), &openai_key)?;

// Llama.cpp (local): GLM 5.2 + Gemma 4 E2B
let builder = harness::glm52_gemma_session::<Doc, Mem>(SessionId::new(), None, None)?;
```

Then customize and build:

```rust
let agent = builder
    .with_system_prompt("You are a helpful assistant.")
    .with_toolshed(my_toolshed)
    .with_config(my_config)
    .with_context_config(my_ctx_cfg)
    .with_memory_config(my_mem_cfg)
    .build()?;
```

### 2.2. Semi-Custom: `RouterMix`

When presets don't match your needs, mix providers yourself:

```rust
use foundation_ai::harness::{RouterMix, CloudPresets, providers::Glm52};
use foundation_ai::types::ModelId;

let preset = RouterMix::new()
    .primary(Glm52::q4_k_m(None)?, ModelId::Name("unsloth/GLM-5.2-GGUF".into(), None))
    .memory(
        CloudPresets::claude_sonnet(&anthropic_key)?,
        ModelId::Name("claude-sonnet-4-6".into(), None),
    )
    .build();

let agent = preset
    .into_agent_builder::<Doc, Mem>(SessionId::new())
    .with_system_prompt("Hybrid local+cloud agent.")
    .build()?;
```

Each role must use a **distinct model id** — the id doubles as the provider's
routing name. The builder panics if a provider cannot describe itself.

### 2.3. Manual: No Helpers, From Scratch

Here's how to build a fully-wired agent without any harness functions. This is
what the harness does internally, spelled out step by step.

#### Step 1: Create and box providers

```rust
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::types::{
    ModelId, ProviderRouter, RoutableProviderBox, RoutingRule,
};
use foundation_auth::{AuthCredential, ConfidentialText};

let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");

let main_provider = AnthropicMessagesProvider::with_config(
    AnthropicConfig::new()
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.clone()))),
);
let memory_provider = AnthropicMessagesProvider::with_config(
    AnthropicConfig::new()
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key))),
);

// Box with explicit identities for routing.
// name = model id string (must be unique per role)
// provider_id = from the provider's descriptor
let main_box = RoutableProviderBox::with_identity(
    main_provider,
    "claude-opus-4-8".into(),
    "anthropic-messages".into(),
);
let memory_box = RoutableProviderBox::with_identity(
    memory_provider,
    "claude-sonnet-4-6".into(),
    "anthropic-messages".into(),
);
```

#### Step 2: Build the router with explicit routing rules

```rust
let router = ProviderRouter::builder()
    .add_provider(Box::new(main_box))
    .add_provider(Box::new(memory_box))
    .rule(RoutingRule {
        model: ModelId::Name("claude-opus-4-8".into(), None),
        provider_name: "claude-opus-4-8".into(),
    })
    .rule(RoutingRule {
        model: ModelId::Name("claude-sonnet-4-6".into(), None),
        provider_name: "claude-sonnet-4-6".into(),
    })
    .build();
```

> **Why explicit rules?** `serves()` is inconsistent across providers:
> - `HuggingFaceGGUFProvider::serves()` → always `false` (no catalog)
> - `AnthropicMessagesProvider::serves()` → always `true` (claims every id)
>
> Explicit `RoutingRule` mapping makes resolution deterministic.

#### Step 3: Build the session

```rust
use foundation_ai::agentic::{AgentSession, AgentConfig, KvMemoryStore};
use foundation_ai::types::ToolShed;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

let agent = AgentSession::<Doc, Mem>::builder(SessionId::new(), router)
    .with_toolshed(ToolShed::default())
    .with_model(ModelId::Name("claude-opus-4-8".into(), None))
    .with_memory_model(ModelId::Name("claude-sonnet-4-6".into(), None))
    .with_system_prompt("You are a helpful assistant.")
    .with_config(AgentConfig::default())
    .build()?;
```

#### Manual: OpenRouter Example (No Helpers)

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{ModelId, ProviderRouter, RoutableProviderBox, RoutingRule};

fn build_openrouter_router(api_key: &str, primary: &str, memory: Option<&str>) -> ProviderRouter {
    let mk = || {
        OpenAIProvider::with_config(
            OpenAIConfig::new()
                .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())))
                .with_base_url("https://openrouter.ai/api/v1"),
        )
    };

    let mut b = ProviderRouter::builder()
        .add_provider(Box::new(RoutableProviderBox::with_identity(
            mk(), primary.into(), "openai-completions".into(),
        )))
        .rule(RoutingRule {
            model: ModelId::Name(primary.into(), None),
            provider_name: primary.into(),
        });

    if let Some(m) = memory {
        b = b
            .add_provider(Box::new(RoutableProviderBox::with_identity(
                mk(), m.into(), "openai-completions".into(),
            )))
            .rule(RoutingRule {
                model: ModelId::Name(m.into(), None),
                provider_name: m.into(),
            });
    }

    b.build()
}
```

---

## 3. Running Turns

### 3.1. Blocking Turn

```rust
use foundation_ai::types::{MessageRole, Messages, TextContent, UserModelContent, SessionRecord, ModelOutput};
use foundation_compact::ids::new_scru128;

let prompt = Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent {
        content: "What is 2 + 2?".into(),
        signature: None,
    }),
    signature: None,
};

let records = agent.run_turn(prompt)?;

for record in &records {
    match record {
        SessionRecord::Generation { content, .. } => {
            if let ModelOutput::Text(tc) = content {
                println!("Assistant: {}", tc.content);
            }
        }
        SessionRecord::ToolCall { name, .. } => println!("Tool call: {name}"),
        SessionRecord::ToolResult { name, .. } => println!("Tool result: {name}"),
        SessionRecord::FailedAction { error, .. } => eprintln!("Error: {error:?}"),
        _ => {}
    }
}
```

### 3.2. Streaming Turn

```rust
use foundation_core::valtron::Stream;

let stream = agent.run_turn_stream(prompt)?;
for item in stream {
    match item {
        Stream::Next(record) => { /* process as it arrives */ }
        Stream::Pending(_) | Stream::Init | Stream::Ignore | Stream::Wait | Stream::Delayed(_) => {}
        Stream::Spread(items) => { /* batch completion */ }
    }
}
```

### 3.3. Multi-Turn

```rust
agent.run_turn(first_prompt)?;
agent.run_turn(follow_up_prompt)?;
```

---

## 4. Steering: Injecting Messages Mid-Turn

### 4.1. Priority Queue (Interrupts)

```rust
agent.steer(Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent {
        content: "Stop. New instructions.".into(),
        signature: None,
    }),
    signature: None,
});
```

### 4.2. Follow-Up Queue (After Current Work)

```rust
agent.follow_up(Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent {
        content: "Also check the config file.".into(),
        signature: None,
    }),
    signature: None,
});
```

---

## 5. Memory System

### 5.1. Memory Tiers

| Tier | Purpose |
|------|---------|
| **Working** | Permanent curated facts |
| **Observation** | Time-scoped structured observations |
| **Reflection** | Condensed reflections over observations |

### 5.2. MemoryStore

```rust
use foundation_ai::agentic::KvMemoryStore;
use foundation_db::MemoryStorage;

let mem_store = KvMemoryStore::<MemoryStorage>::default();  // in-memory
// KvMemoryStore::new(turso_kv_store)  // persistent
```

### 5.3. DocumentStore

```rust
use foundation_db::MemoryDocumentStore;

let doc_store = MemoryDocumentStore::default();  // in-memory
// TursoDocumentStore::new(url, token)?  // persistent
```

### 5.4. Wiring Stores (Manual)

```rust
let agent = AgentSession::<Doc, Mem>::builder(session_id, router)
    .with_doc_store(doc_store)
    .with_memory_store(KvMemoryStore::new(kv_store))
    .with_model(primary_model)
    .build()?;
```

---

## 6. Tools

### 6.1. Default ToolShed

Default: only the `shed` meta-tool (tool registry lookup). No actual
capabilities.

### 6.2. Adding Tools

```rust
use foundation_ai::types::{Tool, ToolShed, Args};
use foundation_jsonschema::scheme;

let toolshed = ToolShed::default()
    .with_read(Some(Tool {
        name: "read_file".into(),
        description: "Read a file".into(),
        arguments: Some(Args::new(
            scheme::object().required("path", scheme::string()).build(),
        )),
        returns: None,
    }))
    .with_shell(Some(Tool {
        name: "shell".into(),
        description: "Run a shell command".into(),
        arguments: Some(Args::new(
            scheme::object().required("command", scheme::string()).build(),
        )),
        returns: None,
    }));
```

### 6.3. Tool Slots

`shed` (always present), `read`, `edit`, `write`, `search`, `search_files`,
`shell`, `memory`, `delegate`.

---

## 7. Configuration

```rust
use foundation_ai::agentic::AgentConfig;

let config = AgentConfig::default()
    .with_max_turns(10)
    .with_loop_threshold(3);
```

Token budgets:

```rust
let snapshot = agent.ledger().snapshot();
println!("Used: {}, remaining: {}", snapshot.total_tokens, snapshot.remaining);
```

---

## 8. Session Lifecycle

### 8.1. Teardown

```rust
agent.end()?;  // flush, drain, persist, reset
```

### 8.2. Resume

```rust
let agent = AgentSession::<Doc, Mem>::resume(
    session_id,
    router,       // rebuild the same ProviderRouter
    AgentConfig::default(),
    None,
)?;
```

Resume protocol: WorkingMemory → Observation → Reflection → recent messages
(10) → semantic recall (deferred) → context assembled → empty queues → fresh
ToolCallManager.

---

## 9. Extension Handles

```rust
agent.message_api();        // message history
agent.ledger();             // token usage
agent.router();             // routing table
agent.steering_queues();    // low-level queue access
agent.memory_hierarchy();   // memory system
agent.tool_manager();       // tool registry
```

---

## 10. Provider-Specific Notes

### Llama.cpp
- Downloads from HuggingFace on first `get_model`
- GPU: `metal` (Apple), `vulkan`, `cuda`
- Quantization: `Q3_K_M`, `Q4_K_M`, `Q5_K_M`, `Q8_0`
- MTP speculative decoding: `harness::with_mtp()` (opt-in, capability-gated)

### OpenAI
- Two providers: `OpenAIProvider` (Chat Completions), `ResponsesProvider` (Responses API)

### Anthropic (Claude)
- `AnthropicMessagesProvider`
- Thinking models: `thinking_level` in `ModelParams`

### OpenRouter
- `OpenAIProvider` with `base_url("https://openrouter.ai/api/v1")`
- Model ids: `provider/model` format (e.g., `anthropic/claude-opus-4-7`)
- Free models: `:free` suffix
- Catalog: `models/providers/openrouter.rs`

---

## Examples (Runnable)

```bash
# Harness shortcuts:
cargo run -p foundation_ai --example hello_claude --features agentic
cargo run -p foundation_ai --example hello_openai --features agentic
cargo run -p foundation_ai --example hello_openrouter --features agentic
cargo run -p foundation_ai --example hello_llamacpp --features "agentic llamacpp"

# Manual (no helpers):
cargo run -p foundation_ai --example manual_claude_router --features agentic
cargo run -p foundation_ai --example manual_openrouter --features agentic
cargo run -p foundation_ai --example custom_router --features agentic
cargo run -p foundation_ai --example agent_with_tools --features agentic
```

---

## See Also

- **Doc 00-1** — Getting Started: Providers (provider setup, persistence)
- **Doc 01** — Agentic loop internals
- **Doc 04** — Tools (tool registry, execution DAG)
- **Doc 08** — AgentSession deep dive (preflight, resume protocol)
- **Doc 12** — Harness presets (RouterMix, RouterPreset)
