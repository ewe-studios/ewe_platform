# How-To: Using the AgentSession API

Creating, running, steering and ending an agent session. For the one-call
harness path see `getting-started/02-agent-harness.md`; this doc is the
manual path and the reference.

Source: `src/agentic/session.rs`.

---

## Quick start

```rust
use foundation_ai::agentic::AgentSession;
use foundation_ai::backends::anthropic_messages_provider::{AnthropicConfig, AnthropicMessagesProvider};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ModelOutput, ProviderRouter,
    RoutableProviderBox, SessionRecord, TextContent, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. A provider, wrapped for routing.
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let provider = AnthropicMessagesProvider::with_config(
        AnthropicConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key))),
    );
    let router = ProviderRouter::single(Box::new(RoutableProviderBox::new(provider)));

    // 2. A session (in-memory stores and a fresh SessionId by default).
    let agent = AgentSession::builder(router)
        .with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
        .with_system_prompt("Answer concisely.")
        .build()?;

    // 3. A turn.
    let records = agent.run_turn(Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Explain Rust lifetimes in two sentences.".into(),
            signature: None,
        }),
        signature: None,
    })?;

    for record in &records {
        if let SessionRecord::Conversation {
            message: Messages::Assistant { content: ModelOutput::Text(t), .. },
        } = record
        {
            print!("{}", t.content);
        }
    }

    // 4. Teardown.
    agent.end()?;
    Ok(())
}
```

`getting-started/01-providers.md` shows the provider setup (auth,
`base_url`, local models) for every backend.

---

## 1. The router

`AgentSession` takes a `ProviderRouter`. One provider:
`ProviderRouter::single(Box::new(RoutableProviderBox::new(p)))`. Several
providers, rules and resolution order: Doc 03 §3.

Always set the model with `with_model(...)`. The default `primary_model` is an
empty name, which only resolves in a single-provider router.

## 2. The builder

`AgentSession::builder(router)` starts a builder over the in-memory stores
(`MemoryDocumentStore` + `KvMemoryStore<MemoryStorage>`) and a fresh
`SessionId`; chain `with_*` → `build()`. The stores are part of the builder's
type: `with_doc_store` / `with_memory_store` swap one and change the type, so
any store works — including ones with no `Default` (Cloudflare D1 / R2) —
and in-memory sessions need no type parameters at all:

```rust
fn handle(agent: &AgentSession) { /* … */ }   // AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>

let agent = AgentSession::builder(router)
    .with_doc_store(SqlDocumentStore::new(turso.clone()))   // → AgentSessionBuilder<SqlDocumentStore<_>, _>
    .with_memory_store(KvMemoryStore::new(turso))
    .build()?;                                              // no Default bound
```

| Method | Default | Notes |
|---|---|---|
| `with_session_id(SessionId)` | `SessionId::new()` | Create-or-continue (§5 Resume) |
| `resume(SessionId)` | — | Like `with_session_id`, but `build()` fails with `SessionNotFound` if the stores hold nothing for it |
| `with_doc_store(D2)` | `MemoryDocumentStore` | Message history (and the memory audit log); changes the builder's type |
| `with_memory_store(M2)` | `KvMemoryStore<MemoryStorage>` | Memory tiers; changes the builder's type |
| `with_toolshed(ToolShed)` | empty | The session's tools — the only tool entry point (§3) |
| `with_model(ModelId)` | empty name | Primary model |
| `with_fallback_models(Vec<ModelId>)` | `[]` | Circuit-breaker fallbacks |
| `with_memory_model(ModelId)` | `None` | Stored in `AgentConfig::memory_model`; nothing generates memory yet (Doc 11) |
| `with_system_prompt(..)` | none | |
| `with_config(AgentConfig)` | `AgentConfig::default()` | `with_model` / `with_fallback_models` / `with_memory_model` override the matching fields at `build()` |
| `with_context_config(ContextConfig)` | 20 recent messages | |
| `with_memory_config(MemoryConfig)` | 30k / 40k token triggers | |
| `with_error_policy(ErrorPolicy)` | `ErrorPolicy::new()` | Doc 01 §6 |
| `with_embedder(Arc<dyn EmbeddingProvider>, model)` | none | Semantic recall (Doc 07) and embedding search for `shed` |
| `with_access(Arc<dyn SessionAccessProvider>)` | `AllowAllAccess` | §6 |
| `with_user(UserId)` | `UserId("local")` | |

All components share the one document store and one memory store: the
message log, context assembly and the memory hierarchy see the same data.

`build()` constructs the toolshed's tools (§3), then runs preflight: the
access provider must allow the session and the primary model. The user's
`token_budget` becomes the ledger's budget.

The old two-argument `AgentSession::<D, M>::builder_for(session_id, router)`
(default-constructed stores) and `builder_with_stores(..)` remain, deprecated.

## 3. Tools

Tools go in a `ToolShed`, given to the builder — the session builder has no
per-tool methods:

```rust
use foundation_ai::agentic::ToolShed;
use foundation_ai::harness::ToolPreset;

let agent = AgentSession::builder(router)
    .with_toolshed(
        ToolShed::new()
            .tool(MyTool)
            .tools(ToolPreset::files(Arc::clone(&fs)) + ToolPreset::shell())
            .tools(ToolPreset::session_memory()),   // built from the session's memory
    )
    .build()?;
```

The model is offered only the built-in `shed` meta-tool at first; the tools
`shed` returns become active and are declared on later requests. A name added
twice fails `build()` with `ToolShedError::DuplicateTool`. Details: Doc 04 §2–3.

Writing tools: Doc 10. Presets and sub-agents:
`getting-started/03-tools-and-presets.md`.

## 4. Running turns

### Streaming

```rust
use foundation_core::valtron::Stream;
use foundation_ai::agentic::AgentProgress;

for item in agent.run_turn_stream(prompt)? {
    match item {
        Stream::Pending(AgentProgress::Generating { model, tokens_so_far }) => {
            eprintln!("[{model}: {} tokens]", tokens_so_far.unwrap_or(0));
        }
        Stream::Pending(AgentProgress::ToolCallRequested { name }) => eprintln!("[tool {name}]"),
        Stream::Pending(_) => {}
        Stream::Next(SessionRecord::Conversation {
            message: Messages::Assistant { content: ModelOutput::Text(t), .. },
        }) => print!("{}", t.content),
        Stream::Next(SessionRecord::Retracted { reason, .. }) => {
            // The loop is retrying this answer: clear what you printed for it.
            eprintln!("\n[retrying: {reason}]");
        }
        Stream::Next(SessionRecord::FailedAction { error, .. }) => {
            eprintln!("error: {error:?}");
        }
        Stream::Next(SessionRecord::Summary { usage, .. }) => {
            eprintln!("\n[{} tokens total]", usage.total);
        }
        _ => {}
    }
}
```

`AgentProgress` is `#[non_exhaustive]`, so keep the `_` arms.

### Collected

```rust
let records = agent.run_turn(prompt)?;   // Err on the first FailedAction
```

`run_turn` already drops retracted assistant output.

### Multi-turn

Call `run_turn` again on the same session. Each turn's messages are persisted
to the `MessageApi`, and the next turn's context includes the last
`ContextConfig::recent_message_count` records.

## 5. Steering and lifecycle

```rust
agent.steer(msg);       // interrupt: discards an in-flight generation / cancels tools
agent.follow_up(msg);   // run after the current work
agent.abort();          // end the running turn at the next boundary
agent.end()?;           // flush the log, persist queued messages, reset the cancel signal
```

`AgentSession` clones are cheap (`Arc`), so steering from another thread is a
clone away. Details: Doc 05.

### Resume

A session's history and memory live in its stores, keyed by `SessionId`, and
every turn's context is read from them. So resuming is building again with the
same id over the same stores — with the same prompt, tools and embedder, which
are configuration, not data:

```rust
// Explicit: "resume exactly this session" — Err(SessionNotFound(id)) when the
// stores hold no records for it, so a typo or a wrong store can't silently
// start an empty session.
let agent = AgentSession::builder(router)
    .resume(session_id)
    .with_doc_store(SqlDocumentStore::new(turso.clone()))
    .with_memory_store(KvMemoryStore::new(turso))
    .with_model(model_id)
    .with_system_prompt("…")
    .with_toolshed(ToolShed::new().tool(MyTool))
    .build()?;                       // preflight runs as for a new session

// Implicit: "this id" — continue if it exists, otherwise start it.
let agent = AgentSession::builder(router)
    .with_session_id(session_id)
    /* …same stores and settings… */
    .build()?;
```

`AgentSession::resume(id, router, config, policy)` and
`resume_with_stores(..)` are deprecated: they can't take a system prompt,
tools or an embedder, and `resume` opens default-constructed stores.

### Extension handles

`agent.session_id()`, `message_api()`, `ledger()`, `steering_queues()`,
`router()`, `tool_manager()`, `memory_hierarchy()`, `context_provider()`.

## 6. Access control

```rust
pub trait SessionAccessProvider: Send + Sync {
    fn can_access_session(&self, user: &UserId, session: &SessionId) -> Result<bool, AuthError>;
    fn can_use_model(&self, user: &UserId, model: &str) -> Result<bool, AuthError>;
    fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError>;
    // Defaulted to "allow" / no-op:
    fn can_use_tool(&self, user: &UserId, tool: &str) -> Result<bool, AuthError> { Ok(true) }
    fn can_spend(&self, user: &UserId, tokens: u64) -> Result<bool, AuthError> { Ok(true) }
    fn record_usage(&self, user: &UserId, tokens: u64) -> Result<(), AuthError> { Ok(()) }
}
```

At `build()` the session checks `can_access_session` and `can_use_model` (for
the primary model only) and applies `token_budget(user).remaining()` as the
ledger budget. During a turn the loop:

- asks `can_spend(user, max_tokens)` before each generation — a refusal ends
  the turn with `AgenticError::Budget`;
- asks `can_use_tool(user, name)` before each tool call — a refusal becomes an
  error result the model sees, and the tool does not run;
- reports each generation's `total_tokens` to `record_usage`.

A turn also stops with `AgenticError::BudgetExhausted` once the ledger budget
is spent.

## 7. Configuration reference

### `AgentConfig`

| Field | Default |
|---|---|
| `primary_model` | empty `ModelId::Name` |
| `fallback_models` | `[]` |
| `memory_model` | `None` |
| `max_inner_iterations` | 25 |
| `max_outer_iterations` | 10 |
| `circuit_breaker_threshold` | 3 |
| `preflight_compression_threshold` | 0.85 (of the token budget) |
| `context_pressure_threshold` | 0.70 (of the token budget) |
| `model_params` | `ModelParams::default()` |

### `ContextConfig`

| Field | Default | Meaning |
|---|---|---|
| `recent_message_count` | 20 | Recent records in each context |
| `recall_budget_tokens` | 4096 | Reserved for semantic recall; not read by any code yet |
| `inject_newer_observations` | `true` | Include the observation tier when it is newer than the latest reflection |

### `MemoryConfig`

| Field | Default |
|---|---|
| `observation_trigger_tokens` | 30 000 |
| `reflection_trigger_tokens` | 40 000 |
| `memory_model` | `None` |
| `parse_strategy` | `MemoryParseStrategy::StructuredText` |

Tuning guidance: Doc 09.
