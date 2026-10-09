# Getting Started: `foundation_ai` Providers

Progressive guide to setting up `foundation_ai` for inference with different
providers: **Llama.cpp** (local GGUF), **Candle** (local safetensors),
**OpenAI**, **Anthropic** (Claude), and **OpenRouter**. We walk from in-memory
(no persistence) through persisted sessions with real stores and tooling.

Every section shows the **harness** shortcut *and* the **manual** (no-helpers)
equivalent so you understand what the harness is doing and can build from
scratch.

---

## 0. Feature Flags

`foundation_ai` is feature-gated. Pick the flags that match your target:

| Feature | What it enables | When |
|---------|----------------|------|
| `llamacpp` | `llama.cpp` GGUF inference + GPU backends | Local inference |
| `candle` | Candle (safetensors) inference | Local ML framework |
| `candle-cuda` / `cuda` / `metal` / `vulkan` | GPU acceleration | Local with GPU |
| `testing` | `MockModelProvider`, `MockTool` | Unit tests |

The agent layer (loop, session, tools, memory) is always compiled; the
`llamacpp` and `candle` features are on by default. For GPU
setup, see the **[GPU Acceleration](../14-gpu-acceleration.md)** deep dive.

```toml
[dependencies]
foundation_ai = { version = "0.0.1", default-features = false }
foundation_auth = "0.0.1"
foundation_db = "0.0.1"
serde_json = "1"
# Add "llamacpp" for local GGUF models, or rely on cloud providers only.
```

---

## 1. Level 1 — In-Memory, No Persistence (Fastest Start)

The shortest path to a working agent that holds everything in RAM. No model
files are downloaded; this uses cloud providers (Anthropic, OpenAI, or
OpenRouter).

### 1a. Using the Harness (Recommended)

```rust
use foundation_ai::harness;
use foundation_ai::types::SessionId;

// Claude Opus main + Sonnet memory, via the Anthropic Messages API.
let builder = harness::claude_session(
    SessionId::new(),
    &std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY"),
)?;

let agent = builder
    .with_system_prompt("You are a helpful assistant.")
    .build()?;
```

Harness presets available:

| Function | Main model | Memory model | Provider |
|----------|-----------|-------------|----------|
| `claude_session(id, key)` | Claude Opus | Claude Sonnet | Anthropic |
| `openai_chat_session(id, key)` | GPT-4o | GPT-4o-mini | OpenAI Chat |
| `openai_responses_session(id, key)` | GPT-4o | GPT-4o-mini | OpenAI Responses |
| `glm52_gemma_session(id, None, None)` | GLM 5.2 | Gemma 4 E2B | Llama.cpp (GGUF) |
| `qwen36_gemma_session(id, None, None)` | Qwen 3.6 | Gemma 4 E2B | Llama.cpp (GGUF) |
| `gemma_session(id, None, None)` | Gemma 4 26B | Gemma 4 E2B | Llama.cpp (GGUF) |
| `candle_llama_session(id, repo, None)` | one safetensors | — | Candle |

### 1b. Manual: No Helpers, From Scratch

Here's what the harness does internally. Build the providers, box them into
`RoutableProviderBox`, register routing rules, and build the `ProviderRouter`:

```rust
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::types::{ProviderRouter, RoutableProviderBox, RoutingRule};
use foundation_ai::agentic::{AgentSession, AgentConfig};

// 1. Create providers. `AnthropicConfig::from_env()` reads ANTHROPIC_API_KEY;
//    `api_key(key)` is `new().with_auth(AuthCredential::SecretOnly(..))`.
let main_provider = AnthropicMessagesProvider::with_config(AnthropicConfig::from_env()?);
let memory_provider = AnthropicMessagesProvider::with_config(AnthropicConfig::from_env()?);

// 2. Box them with explicit identities (name = model id string).
//    A provider must be able to describe itself — the descriptor gives
//    us the provider_id. Routing uses the name, not the serves() probe.
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

// 3. Build the router with explicit rules.
//    Why explicit rules? serves() is inconsistent:
//    - HuggingFaceGGUFProvider::serves() → always false (no catalog)
//    - AnthropicMessagesProvider::serves() → always true (claims everything)
//    Explicit RoutingRule mapping → deterministic resolution.
let router = ProviderRouter::builder()
    .add_provider(Box::new(main_box))
    .add_provider(Box::new(memory_box))
    .rule(RoutingRule {
        model: "claude-opus-4-8".into(),
        provider_name: "claude-opus-4-8".into(),
    })
    .rule(RoutingRule {
        model: "claude-sonnet-4-6".into(),
        provider_name: "claude-sonnet-4-6".into(),
    })
    .build();

// 4. Build the session (in-memory stores and a fresh SessionId by default).
let agent = AgentSession::builder(router)
    .with_model("claude-opus-4-8")
    .with_memory_model("claude-sonnet-4-6")
    .with_config(AgentConfig::default())
    .with_system_prompt("You are a helpful assistant.")
    .build()?;
```

For a **single provider** (no memory model, no routing rules needed), skip the
router — a provider converts into one:

```rust
let agent = AgentSession::builder(AnthropicMessagesProvider::api_key(api_key))
    .with_model("claude-sonnet-4-6")
    .build()?;
// or: let router: ProviderRouter = AnthropicMessagesProvider::api_key(api_key).into();
```

### 1c. Running a Turn

```rust
use foundation_ai::agentic::TurnEvent;

// Just the text (or what was produced before a failure):
let answer = agent.ask("Hello! Say hi back.")?;
println!("{}", answer.text());

// Blocking: collects the turn's records (a Turn derefs to Vec<SessionRecord>).
let turn = agent.run_turn("Hello! Say hi back.")?;
println!("{}", turn.text());

// Streaming: events as they happen.
for event in agent.run_turn_stream("Hello! Say hi back.")?.events() {
    if let TurnEvent::Text(delta) = event {
        print!("{delta}");
    }
}
```

---

## 2. Level 2 — Provider-Specific Setup

Each provider has its own configuration shape. The harness hides this; here's
what happens under the hood.

### 2.1. Anthropic (Claude)

```rust
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
let config = AnthropicConfig::api_key(api_key);   // or AnthropicConfig::from_env()? (ANTHROPIC_API_KEY)

let provider = AnthropicMessagesProvider::with_config(config);
// shorthand: AnthropicMessagesProvider::api_key(api_key)
```

`AnthropicConfig` supports `with_auth()`, base URL override, and timeouts.

### 2.2. OpenAI (Chat Completions)

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};

let config = OpenAIConfig::api_key(api_key);       // or OpenAIConfig::from_env()? (OPENAI_API_KEY)

let provider = OpenAIProvider::with_config(config); // shorthand: OpenAIProvider::api_key(api_key)
```

### 2.3. OpenAI (Responses API)

```rust
use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};

let config = ResponsesConfig::api_key(api_key);    // or ResponsesConfig::from_env()? (OPENAI_API_KEY)

let provider = ResponsesProvider::with_config(config); // shorthand: ResponsesProvider::api_key(api_key)
```

### 2.4. OpenRouter

OpenRouter speaks the OpenAI Completions API wire format. Use the **OpenAI
provider** with a custom `base_url`:

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};

let config = OpenAIConfig::openrouter(api_key);   // API key + OpenRouter base URL
// or OpenAIConfig::openrouter_from_env()?          // OPENROUTER_API_KEY

let provider = OpenAIProvider::with_config(config);

// Any OpenRouter model id (see models/providers/openrouter.rs for the full catalog):
let model = provider.get_model("anthropic/claude-opus-4-7".into())?;
// Free models:
let model = provider.get_model("google/gemma-4-26b-a4b-it:free".into())?;
```

**Manual router with primary + memory:**

```rust
use foundation_ai::types::{ProviderRouter, RoutableProviderBox, RoutingRule};

fn openrouter_router(
    api_key: &str,
    primary_model: &str,
    memory_model: Option<&str>,
) -> Result<ProviderRouter, String> {
    let make_provider = || OpenAIProvider::with_config(OpenAIConfig::openrouter(api_key));

    let main_box = RoutableProviderBox::with_identity(
        make_provider(),
        primary_model.into(),
        "openai-completions".into(),
    );

    let mut builder = ProviderRouter::builder()
        .add_provider(Box::new(main_box))
        .rule(RoutingRule {
            model: primary_model.into(),
            provider_name: primary_model.into(),
        });

    if let Some(mem) = memory_model {
        let mem_box = RoutableProviderBox::with_identity(
            make_provider(),
            mem.into(),
            "openai-completions".into(),
        );
        builder = builder
            .add_provider(Box::new(mem_box))
            .rule(RoutingRule {
                model: mem.into(),
                provider_name: mem.into(),
            });
    }

    Ok(builder.build())
}
```

### 2.5. Llama.cpp (Local GGUF)

```rust
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::harness::providers::Glm52;

// Preset (Glm52, Qwen36, Gemma4E2b, Gemma4_26b, Ornith10):
let provider = Glm52::q4_k_m(None)?;  // Q3_K_M / Q4_K_M / Q5_K_M / Q8_0

// Custom config:
let config = HuggingFaceGGUFConfig::builder()
    .default_quantization("Q4_K_M")
    .cache_dir("/path/to/model/cache")
    .n_gpu_layers(35)
    .build();
let provider = HuggingFaceGGUFProvider::new(config)?;
```

GPU features: `metal` (Apple), `vulkan`, `cuda`/`cuda_static`.

### 2.6. Candle (Local Safetensors)

```rust
use foundation_ai::backends::candle::CandleArchitecture;
use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};

let config = HuggingFaceCandleConfig::builder()
    .architecture(CandleArchitecture::Llama)
    .build();

let provider = HuggingFaceCandleProvider::new(config)?;
```

---

## 3. Level 3 — Persisted Sessions

`AgentSession<D, M>` keeps its data in two stores (in-memory by default).
`foundation_db` ships with several — pick what fits your deployment.

### 3.1. DocumentStore Backends (Message History)

| Type | Backend | Feature | Target |
|------|---------|---------|--------|
| `MemoryDocumentStore` | `Vec<SessionRecord>` in RAM | (always) | dev / test |
| `SqlDocumentStore<Q>` | Any `QueryStore` (sync SQL) | `turso` / `libsql` | embedded |
| `AsyncSqlDocumentStore<Q>` | Any `AsyncQueryStore` | `turso` / `libsql` | native server |
| `D1R2DocumentStore<Q, B>` | D1 (query) + R2 (blobs) | `d1` + `r2` | Cloudflare Workers |

#### In-Memory (Default)

```rust
use foundation_db::MemoryDocumentStore;
let store = MemoryDocumentStore::default();
```

#### Turso (Embedded SQLite with libSQL sync API)

Requires `turso` feature on `foundation_db`.

```rust
use foundation_db::{TursoStorage, SqlDocumentStore};

// Local embedded SQLite:
let turso = TursoStorage::builder()
    .url("file:my_app.db")  // or a remote libSQL URL
    .build()?;
let store = SqlDocumentStore::new(turso);
```

#### libSQL (Local or Remote)

Requires `libsql` feature on `foundation_db`.

```rust
use foundation_db::{LibsqlStore, SqlDocumentStore};

// Local file:
let store = SqlDocumentStore::new(LibsqlStore::new("file:my_app.db")?);

// Remote (Turso Cloud):
let store = SqlDocumentStore::new(
    LibsqlStore::new_with_auth("https://my-db.turso.io", auth_token)?,
);
```

#### Cloudflare D1 + R2 (Edge / Workers)

Requires `d1` + `r2` features. Splits small records into D1, large blobs into R2.

```rust
use foundation_db::{D1R2DocumentStore, D1Store, R2Store};

let doc_store = D1R2DocumentStore::new(
    D1Store::new(d1_client),   // small records (D1 edge SQLite)
    R2Store::new(r2_bucket),   // large blobs (Cloudflare R2)
);
```

### 3.2. KeyValueStore Backends (Memory Cache)

The `MemoryStore` trait wraps a `KeyValueStore`. `KvMemoryStore<K>` adapts
any KV store.

| Type | Backend | Feature | Target |
|------|---------|---------|--------|
| `MemoryStorage` | `HashMap<String, Vec<u8>>` | (always) | dev / test |
| `TursoStorage` | SQLite via libSQL | `turso` | native |
| `LibsqlStore` | Local/remote libSQL | `libsql` | native |
| `D1Store` | Cloudflare D1 edge SQLite | `d1` | native |
| `JsonFileStorage` | Single JSON file on disk | (always) | native, simple |
| `R2Store` | Cloudflare R2 object store | `r2` | native |

#### In-Memory (Default)

```rust
use foundation_db::MemoryStorage;
use foundation_ai::agentic::KvMemoryStore;

let mem_store = KvMemoryStore::<MemoryStorage>::default();
```

#### JsonFileStorage (Simple Disk Persistence)

No dependencies — writes everything to a single JSON file.

```rust
use foundation_db::JsonFileStorage;
use foundation_ai::agentic::KvMemoryStore;

let kv = JsonFileStorage::new("data/stores.json")?;
let mem_store = KvMemoryStore::new(kv);
```

#### Turso / libSQL

```rust
use foundation_db::{TursoStorage, LibsqlStore};
use foundation_ai::agentic::KvMemoryStore;

let kv = TursoStorage::builder().url("file:my_app.db").build()?;
let mem_store = KvMemoryStore::new(kv);

// Or libSQL remote:
let kv = LibsqlStore::new_with_auth("https://my-db.turso.io", token)?;
let mem_store = KvMemoryStore::new(kv);
```

#### Cloudflare D1 (Edge)

```rust
use foundation_db::D1Store;
use foundation_ai::agentic::KvMemoryStore;

let kv = D1Store::new(d1_client);
let mem_store = KvMemoryStore::new(kv);
```

### 3.3. Wiring Stores Into a Session

```rust
// Pick your concrete stores:
//   SqlDocumentStore<TursoStorage>          // Turso messages
//   D1R2DocumentStore<D1Store, R2Store>     // Cloudflare edge
//   KvMemoryStore<TursoStorage>             // Turso memory cache
//   KvMemoryStore<JsonFileStorage>          // JSON file cache

// The store setters change the builder's type and need no Default bound, so
// they work with every persistent backend above.
let agent = AgentSession::builder(router)
    .with_session_id(session_id)
    .with_doc_store(doc_store)         // your concrete DocumentStore
    .with_memory_store(mem_store)      // KvMemoryStore wrapping a KeyValueStore
    .with_model(primary_model_id)
    .with_system_prompt("You are a helpful assistant.")
    .build()?;
```

### 3.4. Feature Flag Matrix

| Store | Feature on `foundation_db` | Notes |
|-------|---------------------------|-------|
| `MemoryDocumentStore` | (always) | Ephemeral, zero setup |
| `MemoryStorage` | (always) | Ephemeral HashMap |
| `JsonFileStorage` | (always) | Single JSON file, native only |
| `TursoStorage` | `turso` | Embedded SQLite + sync |
| `LibsqlStore` | `libsql` | Local/remote libSQL |
| `D1Store` | `d1` | Cloudflare D1 (HTTP) |
| `R2Store` | `r2` | Cloudflare R2 blobs |

The `default` feature on `foundation_db` enables `turso`, `d1`, and `r2`.

### 3.5. Session Resume

A session's history and memory live in its stores, keyed by the
`SessionId`, so resuming is building again with the same id over the same
stores:

```rust
let agent = AgentSession::builder(router)   // rebuild ProviderRouter the same way
    .with_model("my-model")
    .resume(session_id)                     // same SessionId from before; must exist
    .with_doc_store(doc_store)
    .with_memory_store(mem_store)
    .with_system_prompt("You are a helpful assistant.")
    .build()?;                              // Err(SessionNotFound(id)) if the stores don't hold it
```

Use `.with_session_id(session_id)` instead to continue the session when it
exists and start it otherwise. The system prompt and tools are not persisted —
pass them again. See Doc 08 §5.

---

## 4. Level 4 — Adding Tools

Tools are `ToolImpl`s in a `ToolShed`, passed to the builder — the session
builder has no per-tool methods:

```rust
use foundation_ai::agentic::ToolShed;
use foundation_ai::harness::ToolPreset;

let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(
        ToolShed::new()
            .tool(MyTool)                              // your own ToolImpl
            .tools(ToolPreset::files(Arc::clone(&fs))) // read / write / edit over a VFS
            .tools(ToolPreset::shell()),               // bash
    )
    .build()?;
```

The model is offered the `shed` discovery tool first; the tools it returns
become callable on the following requests.

There are no fixed tool slots. See
[Tools & Presets](03-tools-and-presets.md) and
[Doc 10](../10-building-custom-tools.md) for writing tools.

---

## 5. Level 5 — Steering, Memory, and Configuration

### 5.1. Steering Queues

```rust
// High-priority: interrupts current work
agent.steer(priority_message);

// Follow-up: processed after current work completes
agent.follow_up(follow_up_message);
```

### 5.2. Token Budgets

```rust
let snapshot = agent.ledger().snapshot();
println!("Tokens used: {}, remaining: {:?}",
    snapshot.total, snapshot.remaining);
```

### 5.3. Agent Configuration

```rust
use foundation_ai::agentic::AgentConfig;

let config = AgentConfig {
    max_inner_iterations: 10,
    max_outer_iterations: 5,
    ..AgentConfig::default()
};
```

### 5.4. Session Lifecycle

```rust
agent.end()?;  // flush buffers, drain queues, persist
```

---

## 6. Quick Reference

```
Harness path (fastest):
  harness::claude_session(id, key) → .build()

Manual path (full control):
  Provider + RoutableProviderBox → ProviderRouter::builder()
    → AgentSession::builder(router) → .build()

OpenRouter:
  OpenAIConfig::new().with_base_url("https://openrouter.ai/api/v1")

Persistence:
  .with_doc_store(doc_store).with_memory_store(kv_memory_store)

Resume:
  AgentSession::builder(router).resume(id).with_doc_store(..).with_memory_store(..)

Tools:
  .with_toolshed(ToolShed::new().tool(tool).tools(ToolPreset::files(fs)))
```

---

## Examples (Runnable)

```bash
# Harness shortcuts:
cargo run -p foundation_ai --example hello_claude
cargo run -p foundation_ai --example hello_openai
cargo run -p foundation_ai --example hello_llamacpp

# Manual (no helpers):
cargo run -p foundation_ai --example manual_claude_router
cargo run -p foundation_ai --example manual_openrouter
cargo run -p foundation_ai --example agent_with_tools
```

---

## See Also

- **Doc 00-2** — Getting Started: Agent Harness (session lifecycle, resume, steering)
- **Doc 03** — Model providers (router internals, RoutableProvider, RoutingRule)
- **Doc 12** — Harness presets deep dive (RouterMix, RouterPreset)
- **Doc 09** — Customizing the agent loop
- **Doc 04** — Tools (tool registry, execution DAG)
