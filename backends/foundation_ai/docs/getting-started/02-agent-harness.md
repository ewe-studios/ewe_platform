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

An `AgentSession` is the single public handle for a conversation. It
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

It stores its data in two stores, which are type parameters with in-memory
defaults (`AgentSession` = `AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>`):
- **`D: DocumentStore`** — stores the message history (conversation log)
- **`M: MemoryStore`** — caches the latest memory record per tier per session

---

## 2. Building an Agent Session

### 2.1. Fastest Path: Harness Presets

```rust
use foundation_ai::harness;
use foundation_ai::types::SessionId;

// Anthropic: Claude Opus + Sonnet
let builder = harness::claude_session(SessionId::new(), &anthropic_key)?;

// OpenAI: GPT-4o + GPT-4o-mini (Chat Completions)
let builder = harness::openai_chat_session(SessionId::new(), &openai_key)?;

// Llama.cpp (local): GLM 5.2 + Gemma 4 E2B
let builder = harness::glm52_gemma_session(SessionId::new(), None, None)?;
```

Then customize and build:

```rust
let agent = builder
    .with_system_prompt("You are a helpful assistant.")
    .with_config(my_config)
    .with_context_config(my_ctx_cfg)
    .with_memory_config(my_mem_cfg)
    .build()?;
```

### 2.2. Semi-Custom: `RouterMix`

When presets don't match your needs, mix providers yourself:

```rust
use foundation_ai::harness::{RouterMix, CloudPresets, providers::Glm52};

let preset = RouterMix::new()
    .primary(Glm52::q4_k_m(None)?, "unsloth/GLM-5.2-GGUF")          // model ids: impl Into<ModelId>
    .memory(CloudPresets::claude_sonnet(&anthropic_key)?, "claude-sonnet-4-6")
    .build();

let agent = preset
    .into_agent_builder()                 // in-memory stores, fresh SessionId
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
use foundation_ai::types::{ProviderRouter, RoutableProviderBox, RoutingRule};

// `from_env()` reads ANTHROPIC_API_KEY; `AnthropicConfig::api_key(key)` takes it
// directly (an `AuthCredential::SecretOnly` credential).
let main_provider = AnthropicMessagesProvider::with_config(AnthropicConfig::from_env()?);
let memory_provider = AnthropicMessagesProvider::with_config(AnthropicConfig::from_env()?);

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
        model: "claude-opus-4-8".into(),
        provider_name: "claude-opus-4-8".into(),
    })
    .rule(RoutingRule {
        model: "claude-sonnet-4-6".into(),
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
use foundation_ai::agentic::{AgentSession, AgentConfig};

let agent = AgentSession::builder(router)
    .with_model("claude-opus-4-8")
    .with_memory_model("claude-sonnet-4-6")
    .with_system_prompt("You are a helpful assistant.")
    .with_config(AgentConfig::default())
    .build()?;
```

#### Manual: OpenRouter Example (No Helpers)

```rust
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{ProviderRouter, RoutableProviderBox, RoutingRule};

fn build_openrouter_router(api_key: &str, primary: &str, memory: Option<&str>) -> ProviderRouter {
    // API key + OpenRouter's base URL.
    let mk = || OpenAIProvider::with_config(OpenAIConfig::openrouter(api_key));

    let mut b = ProviderRouter::builder()
        .add_provider(Box::new(RoutableProviderBox::with_identity(
            mk(), primary.into(), "openai-completions".into(),
        )))
        .rule(RoutingRule {
            model: primary.into(),
            provider_name: primary.into(),
        });

    if let Some(m) = memory {
        b = b
            .add_provider(Box::new(RoutableProviderBox::with_identity(
                mk(), m.into(), "openai-completions".into(),
            )))
            .rule(RoutingRule {
                model: m.into(),
                provider_name: m.into(),
            });
    }

    b.build()
}
```

---

## 3. Running Turns

### 3.1. Blocking Turn

Turns take `impl Into<Messages>` — a `&str` is a user message.

```rust
// Just the answer text:
let answer = agent.ask("What is 2 + 2?")?;
println!("Assistant: {}", answer.text());

// The whole turn (a Turn derefs to Vec<SessionRecord>):
let turn = agent.run_turn("What is 2 + 2?")?;
println!("Assistant: {}", turn.text());
for result in turn.tool_results() {
    if let Messages::ToolResult { name, .. } = result {
        println!("Tool result: {name}");
    }
}
// Partial output first, then the error that ended the turn (if any):
if let Some(error) = turn.failure() {
    eprintln!("Error: {error}");
}
```

### 3.2. Streaming Turn

```rust
use foundation_ai::agentic::TurnEvent;

for event in agent.run_turn_stream("What is 2 + 2?")?.events() {
    match event {
        TurnEvent::Text(delta) => print!("{delta}"),
        TurnEvent::Retract { .. } => { /* the loop is retrying: drop the text shown for this turn */ }
        TurnEvent::ToolCall { name, .. } => eprintln!("→ {name}"),
        TurnEvent::Failed(error) => eprintln!("\nError: {error}"),   // terminal
        TurnEvent::Done(summary) => eprintln!("\n[{} tokens]", summary.usage.total), // terminal
        _ => {}
    }
}
```

Iterating the `TurnStream` itself still yields the raw
`Stream<SessionRecord, AgentProgress>` items.

### 3.3. Multi-Turn

```rust
agent.run_turn("First question")?;
agent.run_turn("A follow-up that relies on the first answer")?;
```

---

## 4. Steering: Injecting Messages Mid-Turn

### 4.1. Priority Queue (Interrupts)

```rust
agent.steer("Stop. New instructions.");                  // or Messages::agent(..) from another agent
```

### 4.2. Follow-Up Queue (After Current Work)

```rust
agent.follow_up("Also check the config file.");
```

---

## 5. Memory System

### 5.1. Memory Tiers

| Tier | Purpose |
|------|---------|
| **Working** | Permanent curated facts |
| **Observation** | Time-scoped structured observations |
| **Reflection** | Condensed reflections over observations |

### 5.2. Available Store Backends

`AgentSession<D, M>` keeps its data in a `DocumentStore` (message history) and
a `MemoryStore` (latest memory per tier via a `KeyValueStore`); both default
to the in-memory stores.

**DocumentStore backends:**

| Type | Backend | Feature |
|------|---------|---------|
| `MemoryDocumentStore` | RAM (`Vec<SessionRecord>`) | (always) |
| `SqlDocumentStore<Q>` | Sync SQL (`QueryStore`) | `turso` / `libsql` |
| `AsyncSqlDocumentStore<Q>` | Async SQL | `turso` / `libsql` |
| `D1R2DocumentStore<Q, B>` | D1 + R2 | `d1` + `r2` |

**KeyValueStore backends (wrap into `KvMemoryStore`):**

| Type | Backend | Feature |
|------|---------|---------|
| `MemoryStorage` | RAM (`HashMap`) | (always) |
| `TursoStorage` | Embedded/remote SQLite | `turso` |
| `LibsqlStore` | Local/remote libSQL | `libsql` |
| `D1Store` | Cloudflare D1 | `d1` |
| `JsonFileStorage` | Single JSON file | (always) |
| `R2Store` | Cloudflare R2 objects | `r2` |

See **Doc 00-1** (Getting Started: Providers, §3) for construction details
and feature flag matrix for every backend.

### 5.3. Wiring Stores

```rust
// Quick picks:
//   SqlDocumentStore<TursoStorage>        // embedded SQLite messages
//   D1R2DocumentStore<D1Store, R2Store>   // Cloudflare edge
//   KvMemoryStore<TursoStorage>           // Turso memory cache
//   KvMemoryStore<JsonFileStorage>        // JSON file cache

let agent = AgentSession::builder(router)
    .with_session_id(session_id)
    .with_doc_store(doc_store)                      // changes the builder's type
    .with_memory_store(KvMemoryStore::new(kv_store))
    .with_model(primary_model)
    .build()?;

// From a harness preset:
let agent = harness::claude_router(&key)?
    .into_agent_builder()
    .with_session_id(session_id)
    .with_doc_store(doc_store)
    .with_memory_store(KvMemoryStore::new(kv_store))
    .build()?;
```

The store setters take any store — including ones with no `Default`, like
D1 / R2 — and `build()` has no `Default` bound.

---

## 6. Tools

> For the built-in `read`/`write`/`edit`/`bash` tools, the `ToolPreset`
> helpers, and sub-agent delegation, see the dedicated guide:
> **[Getting Started: Tools & Presets](03-tools-and-presets.md)**. This section
> is the quick version.

### 6.1. Adding Tools

Tools go in a `ToolShed`, passed to `with_toolshed` — the only way to give a
session tools:

```rust
use foundation_ai::agentic::ToolShed;
use foundation_ai::harness::ToolPreset;

let agent = builder
    .with_toolshed(
        ToolShed::new()
            .tool(MyTool)
            .tools(ToolPreset::files(Arc::clone(&fs)))
            .tools(ToolPreset::shell()),
    )
    .build()?;
```

Each tool declares its own shape via `ToolImpl::definition()` (`Tool::SingleCommand` or
`Tool::MultiCommands`); there are no fixed slots.

### 6.2. What the Model Sees

With no tools, the model is offered nothing. Otherwise it is offered only the
built-in `shed` discovery tool; the tools `shed` returns become active and are
declared on the following requests. Calling a tool before `shed` has returned
it is a tool error that tells the model to look it up first.

---

## 7. Configuration

```rust
use foundation_ai::agentic::AgentConfig;

let config = AgentConfig {
    max_inner_iterations: 10,      // tool rounds per turn (default 25)
    max_outer_iterations: 5,       // queue drains per turn (default 10)
    circuit_breaker_threshold: 3,
    ..AgentConfig::default()
};
let agent = builder.with_config(config).build()?;
```

All fields and defaults: Doc 08 §7. Tuning: Doc 09.

Token usage:

```rust
let snapshot = agent.ledger().snapshot();
println!("Used: {}, remaining: {:?}", snapshot.total, snapshot.remaining);
```

---

## 8. Session Lifecycle

### 8.1. Teardown

```rust
agent.end()?;  // flush, drain, persist, reset
```

### 8.2. Resume

History and memory live in the stores, so resuming is building again over the
same stores with the same id:

```rust
let agent = AgentSession::builder(router)      // rebuild the same ProviderRouter
    .with_model("my-model")
    .resume(session_id)                        // Err(SessionNotFound) if the stores don't hold it
    .with_doc_store(doc_store)
    .with_memory_store(mem_store)
    .with_system_prompt("…")                   // configuration isn't persisted: pass it again
    .with_toolshed(tools)
    .build()?;
```

`.with_session_id(id)` instead of `.resume(id)` continues the session if it
exists and starts it otherwise. See Doc 08 §5.

---

## 9. Extension Handles

```rust
agent.message_api();        // message history
agent.ledger();             // token usage
agent.router();             // routing table
agent.steering_queues();    // low-level queue access
agent.memory_hierarchy();   // memory system
agent.tool_manager();       // tool registry
agent.context_provider();   // history + memory recall
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

- **[Getting Started: Providers](01-providers.md)** — provider setup, persistence
- **[Getting Started: Tools & Presets](03-tools-and-presets.md)** — file/shell tools, presets, delegation
- **[Doc 01 — Agentic loop](../01-agentic-loop.md)** — loop internals
- **[Doc 04 — Tools](../04-tools.md)** — tool registry, execution DAG
- **[Doc 08 — AgentSession](../08-how-to-agent-session.md)** — preflight, resume protocol
- **[Doc 12 — Harness presets](../12-how-to-harness-presets.md)** — RouterMix, RouterPreset
- **[Doc 14 — GPU Acceleration](../14-gpu-acceleration.md)** — CUDA, Metal, Vulkan
