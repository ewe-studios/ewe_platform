# Proposal 15 — Simplifying the `foundation_ai` API surface

**Status:** proposal for discussion. Nothing here is implemented. The bug fixes
for the "Known limitations" in Doc 00 are handled separately; this doc is
about making the everyday API smaller and harder to misuse.

Every friction point (§1) and every plan item (§3) comes with code: **Today**
is the current API as it exists in `src/` (signatures copied from the source),
**Proposed** is the call site or signature after the change. Proposed names are
used the same way everywhere in this doc; §6 lists them in one place.

---

## 1. Where the friction is today

Here is the minimum code for "ask Claude a question, print the answer" without
the harness, as the current docs show it:

```rust
type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

let provider = AnthropicMessagesProvider::with_config(
    AnthropicConfig::new()
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key))),
);
let router = ProviderRouter::single(Box::new(RoutableProviderBox::new(provider)));
let agent = AgentSession::<Doc, Mem>::builder(SessionId::new(), router)
    .with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
    .build()?;

let records = agent.run_turn(Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent { content: "Hi".into(), signature: None }),
    signature: None,
})?;
for r in &records {
    if let SessionRecord::Conversation {
        message: Messages::Assistant { content: ModelOutput::Text(t), .. },
    } = r { print!("{}", t.content); }
}
agent.end()?;
```

Roughly 25 lines and 15 imported types, and several of the steps are traps:

| # | Friction | Why it hurts | Fixed by |
|---|---|---|---|
| F1 | `AgentSession<D, M>` generics with turbofish everywhere, plus a `D: Default, M: Default` bound on `build()` | Stores without a `Default` (Cloudflare D1 / R2) can't be used; every signature carries two type parameters | 2, 3 |
| F2 | `with_toolshed(ToolShed)` exists, but tool *implementations* can only be registered on `tool_manager()` after `build()` | The obvious path fails preflight | 1 |
| F3 | Building a user message takes 4 fields and a nested struct | Every example repeats the same 8 lines | 6 |
| F4 | Getting the answer text means matching three nested enums, and streaming consumers must handle `Retracted` | Easy to get wrong; the common case has no helper | 7, 8 |
| F5 | `RoutableProviderBox::new` + `Box::new` + `ProviderRouter::single` for one provider, and `ModelId::Name(s.into(), None)` for a model name | Three wrappers to say "use this provider" | 9, 10 |
| F6 | `AuthCredential::SecretOnly(ConfidentialText::new(key))` | The common case (an API key) is the hardest to type | 11 |
| F7 | `resume()` is a separate constructor that ignores stores, prompt, tools and configs | It can't do its one job | 4 |
| F8 | `AgentConfig` and the builder both set `primary_model`, `fallback_models`, `memory_model` | Two sources of truth; the builder silently wins | 14 |
| F9 | `ArgType` has 18 variants, and backends map booleans/arrays/null differently | Every tool needs defensive matching | 12, 13 |
| F10 | Name collisions: two `LoopDetection` types (one re-exported as `InlineLoopDetection`), `ToolDefinition` reachable from two modules | Confusing imports | 16 |
| F11 | Harness returns `Result<_, String>`, sessions return `ErrorTrace<AgenticError>`, providers return their own errors | `?` needs `Box<dyn Error>` everywhere | 15 |
| F12 | The session's `ContextProvider` isn't reachable, so `with_embedder` can't power `search_context` | A builder option with no observable effect | 5 |
| F13 | The `agentic` feature gates no code | A flag that does nothing | 17 |

### F1 — two type parameters, and a `Default` bound on `build()`

Today (`session.rs`): the builder is generic over both stores, and `build()`
fills any store you didn't set with `Default`:

```rust
pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>
where
    D: Default,
    M: Default,
```

So a store with no `Default` can't be used even when you pass it in:

```rust
let d1 = D1R2DocumentStore::new(
    D1Store::new_kv(token, account, database, "ai_"),
    R2Store::new_blob(token, account, bucket, "ai/"),
);
let agent = AgentSession::<D1R2DocumentStore<D1Store, R2Store>, Mem>::builder(id, router)
    .with_doc_store(d1)
    .build()?; // error[E0277]: `D1R2DocumentStore<D1Store, R2Store>: Default` is not satisfied
```

Stores that *do* implement `Default` (Turso, libsql, JSON-file, Fjall,
`SqlDocumentStore<Q>`, `KvMemoryStore<K>`) compile, but `build()` also calls
`D::default()` / `M::default()` for the memory hierarchy, so part of the
session silently runs on a second, throwaway store (`TursoStorage::default()`
opens `":memory:"`):

```rust
// session.rs, build()
let message_api = MessageApi::new(session_id.clone(), doc_store);          // your store
let coordinator = MemoryCoordinator::new(M::default(), D::default());      // fresh defaults
```

And every function that touches a session carries the generics:

```rust
fn handle<D: DocumentStore + 'static, M: MemoryStore + 'static>(agent: &AgentSession<D, M>) { /* … */ }
let builder = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?;
```

Proposed (items 2, 3):

```rust
let agent = AgentSession::builder(router)            // AgentSessionBuilder<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>
    .with_doc_store(d1)                              // → AgentSessionBuilder<D1R2DocumentStore<D1Store, R2Store>, _>
    .build()?;                                       // no Default bound

fn handle(agent: &AgentSession) { /* … */ }          // in-memory: no generics at all
```

### F2 — the shed and the tools are configured in two places

Today a `ToolShed` holds only *definitions* (`shed: Option<Tool>`,
`tools: Vec<Tool>`); implementations live on the session's
`ToolCallManager`, which only exists after `build()`. But `build()` runs
preflight, which requires every shed entry to already be registered:

```rust
let agent = AgentSession::<Doc, Mem>::builder(id, router)
    .with_toolshed(ToolShed::default().with_tool(GreetTool.definition()))
    .build()?;
// Err: "toolshed tool 'greet' not registered with ToolCallManager"
```

The working path is to skip the shed and register after `build()`:

```rust
let agent = AgentSession::<Doc, Mem>::builder(id, router).build()?;
agent.tool_manager().register(Arc::new(GreetTool));
ToolPreset::files(fs).register_all(agent.tool_manager());
```

Proposed (item 1) — keep `with_toolshed`, and add tool registration to the
builder so both work before preflight:

```rust
let agent = AgentSession::builder(router)
    .with_tool(GreetTool)                                   // one tool
    .with_tools(ToolPreset::files(fs) + ToolPreset::shell()) // a preset…
    .with_tools(vec![Arc::new(MyTool) as Arc<dyn ToolImpl>]) // …or a Vec<Arc<dyn ToolImpl>>
    .build()?;                                              // shed populated, preflight passes

// Setting the shed directly still works and is still checked:
let agent = AgentSession::builder(router)
    .with_toolshed(ToolShed::default().with_tool(GreetTool.definition()))
    .with_tool(GreetTool)
    .build()?;
```

### F3 — building a message

Today:

```rust
let prompt = Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent { content: "Hi".into(), signature: None }),
    signature: None,
};
agent.run_turn(prompt)?;
```

(`agentic::testing::mock_user(&str)` does this, but only behind the `testing`
feature.)

Proposed (item 6):

```rust
agent.run_turn("Hi")?;
agent.steer(Messages::agent("Stop and summarise what you have."));
```

### F4 — reading the answer

Today, collected:

```rust
let mut answer = String::new();
for r in &agent.run_turn(prompt)? {
    if let SessionRecord::Conversation {
        message: Messages::Assistant { content: ModelOutput::Text(t), .. },
    } = r { answer.push_str(&t.content); }
}
```

Today, streaming — the consumer sees valtron's scheduling states and must undo
printed text on `Retracted` itself:

```rust
for item in agent.run_turn_stream(prompt)? {
    match item {
        Stream::Next(SessionRecord::Conversation {
            message: Messages::Assistant { content: ModelOutput::Text(t), .. },
        }) => print!("{}", t.content),
        Stream::Next(SessionRecord::Retracted { .. }) => clear_current_answer(),
        Stream::Next(SessionRecord::Summary { usage, .. }) => eprintln!("{} tokens", usage.total),
        Stream::Spread(items) => { /* StreamSpread::Done(rec) — same matching again */ }
        Stream::Pending(_) | Stream::Init | Stream::Wait | Stream::Delayed(_) | Stream::Ignore => {}
        _ => {}
    }
}
```

Proposed (items 7, 8):

```rust
let answer: String = agent.ask("Hi")?;
let turn = agent.run_turn("Hi")?;      // Turn
println!("{}", turn.text());

for event in agent.run_turn_stream("Hi")?.events() {
    match event {
        TurnEvent::Text(delta) => print!("{delta}"),
        TurnEvent::Retract { .. } => clear_current_answer(),
        TurnEvent::Done(summary) => eprintln!("{} tokens", summary.usage.total),
        _ => {}
    }
}
```

### F5 — one provider, three wrappers; model ids

Today:

```rust
let router = ProviderRouter::single(Box::new(RoutableProviderBox::new(provider)));
let router = ProviderRouter::builder()
    .add_provider(Box::new(RoutableProviderBox::new(anthropic)))
    .add_provider(Box::new(RoutableProviderBox::new(openai)))
    .build();
builder.with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
       .with_fallback_models(vec![ModelId::Name("gpt-4o".into(), None)]);
```

Proposed (items 9, 10):

```rust
AgentSession::builder(provider)                           // impl Into<ProviderRouter>
let router = ProviderRouter::builder().provider(anthropic).provider(openai).build();
builder.with_model("claude-sonnet-4-6").with_fallback_models(["gpt-4o"]);
```

### F6 — API keys

Today (`harness/providers.rs` repeats this five times):

```rust
let config = AnthropicConfig::new()
    .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
let provider = AnthropicMessagesProvider::with_config(config);
```

Proposed (item 11):

```rust
let provider = AnthropicMessagesProvider::api_key(api_key);
let provider = AnthropicMessagesProvider::with_config(AnthropicConfig::from_env()?); // ANTHROPIC_API_KEY
```

### F7 — `resume` can't resume

Today (`session.rs`):

```rust
pub fn resume(
    session_id: SessionId,
    router: ProviderRouter,
    config: AgentConfig,
    policy: Option<ErrorPolicy>,
) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>
// body: ToolShed::default(), D::default(), M::default(), system prompt None,
//       ContextConfig::default(), MemoryConfig::default(), AllowAllAccess,
//       and no preflight() call.
```

There is no parameter for the stores the history lives in, so with a
persistent store type it opens a fresh one and finds nothing:

```rust
type Doc = SqlDocumentStore<TursoStorage>;
type Mem = KvMemoryStore<TursoStorage>;
let agent = AgentSession::<Doc, Mem>::resume(id, router, AgentConfig::default(), None)?;
// TursoStorage::default() = a new ":memory:" database → empty history,
// no system prompt, no tools, no embedder.
```

Proposed (item 4):

```rust
let agent = AgentSession::builder(router)
    .with_session_id(id)                                 // existing id → history read from these stores
    .with_doc_store(SqlDocumentStore::new(turso.clone()))
    .with_memory_store(KvMemoryStore::new(turso))
    .with_system_prompt("You are a coding assistant.")
    .with_tools(ToolPreset::files(fs))
    .build()?;                                           // preflight runs as for a new session
```

### F8 — models set in two places

Today `AgentConfig` has `primary_model`, `fallback_models` and `memory_model`,
and so does the builder. `build()` resolves them like this:

```rust
let mut config = self.config;
if let Some(model) = self.model {
    config.primary_model = model;          // overrides only if with_model was called
}
config.fallback_models = self.fallback_models; // always overwritten (default: empty)
config.memory_model = self.memory_model;       // always overwritten (default: None)
```

So this silently drops the fallback and memory models:

```rust
builder.with_config(AgentConfig {
    fallback_models: vec![ModelId::Name("gpt-4o".into(), None)],
    memory_model: Some(ModelId::Name("claude-sonnet-4-6".into(), None)),
    ..AgentConfig::default()
})
.build()?; // config.fallback_models == [], config.memory_model == None
```

Proposed (item 14): models live only on the builder.

```rust
builder
    .with_model("claude-opus-4-8")
    .with_fallback_models(["gpt-4o"])
    .with_memory_model("claude-sonnet-4-6")
    .with_config(AgentConfig { max_inner_iterations: 40, ..AgentConfig::default() }); // no model fields
```

### F9 — `ArgType` and inconsistent backends

Today `ArgType` has 18 variants (`Text`, `Float32`, `Float64`, `Usize`, `U8`…`U128`,
`Isize`, `I8`…`I128`, `Duration`, `JSON`, `JSONMap`) and no `Bool`. The three
`json_value_to_arg_type` copies disagree:

```rust
// types/base_types.rs (text tool-call formatter: llama.cpp, Candle)
serde_json::Value::Bool(b) => ArgType::Text(if *b { "true" } else { "false" }.to_string()),
serde_json::Value::Null    => ArgType::Text(String::new()),
serde_json::Value::Array(arr) => ArgType::Text(/* elements joined with ", " */),
serde_json::Value::Object(map) => ArgType::JSONMap(/* recursive */),

// backends/backend_utils.rs (OpenAI, Responses) and anthropic_messages_provider.rs
other => ArgType::JSON(other.to_string()),   // bool, null, array AND object
```

So the same tool behaves differently per backend. `EditTool` only accepts the
text form of a boolean:

```rust
// agentic/tools/files.rs
let replace_all = matches!(
    arguments.get("replace_all"),
    Some(ArgType::Text(t)) if t == "true"
); // Anthropic/OpenAI send ArgType::JSON("true") → replace_all is false
```

and each tool module carries its own `text_arg` / `opt_usize` helpers.

Proposed (items 12, 13):

```rust
async fn execute(&self, arguments: HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
    let args = ToolArgs::new("edit", &arguments);
    let path = args.str("path")?;
    let replace_all = args.opt_bool("replace_all")?.unwrap_or(false); // true / "true" / JSON("true")
    // …
}
```

### F10 — name collisions

Today:

```rust
use foundation_ai::agentic::LoopDetection;          // errors::LoopDetection — struct { kind, occurrences }
use foundation_ai::agentic::InlineLoopDetection;    // loop_detection::LoopDetection — enum { NoLoop, ExactLoop, … }
use foundation_ai::agentic::loop_detection::LoopDetection; // the same enum, under its real name

use foundation_ai::types::ToolDefinition;                 // defined here
use foundation_ai::agentic::tool_impl::ToolDefinition;    // re-export
use foundation_ai::agentic::ToolDefinition;               // re-export of the re-export
```

Proposed (item 16):

```rust
use foundation_ai::agentic::LoopDetectedInfo;  // AgenticError::LoopDetected(LoopDetectedInfo)
use foundation_ai::agentic::LoopDetection;     // the detector's verdict enum
use foundation_ai::types::ToolDefinition;      // the only path
```

### F11 — three error types

Today:

```rust
pub fn claude_session<D, M>(session_id: SessionId, api_key: &str)
    -> Result<AgentSessionBuilder<D, M>, String>;                      // harness
pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>; // session

fn start(key: &str) -> Result<AgentSession<Doc, Mem>, ErrorTrace<AgenticError>> {
    let agent = harness::claude_session::<Doc, Mem>(SessionId::new(), key)
        .map_err(|e| ErrorTrace::new(AgenticError::Session(e)))? // no From<String>
        .build()?;
    Ok(agent)
}
```

Proposed (item 15):

```rust
fn start(key: &str) -> Result<AgentSession, ErrorTrace<AgenticError>> {
    RouterPreset::claude(key)?.into_agent_builder().build()
}
```

### F12 — `with_embedder` has no reachable effect

Today `with_embedder` wires the embedder into the session's private
`ContextProvider`, and there is no accessor for it. `SearchContextTool::new`
needs a `ContextProvider<D, M>`, so you build a second one by hand — without
the embedder unless you repeat it, and over a fresh memory store:

```rust
let agent = AgentSession::<Doc, Mem>::builder(id, router)
    .with_embedder(embedder.clone(), "bge-small")
    .build()?;

let ctx = ContextProvider::new(
    agent.session_id().clone(),
    agent.message_api().clone(),
    Arc::new(Mem::default()),           // not the session's memory store
    agent.ledger().clone(),
    None,
    ContextConfig::default(),
)
.with_embedder(embedder, "bge-small");  // repeated by hand
agent.tool_manager().register(Arc::new(SearchContextTool::new(ctx)));
```

Proposed (item 5):

```rust
ToolPreset::search_context(&agent).register_all(agent.tool_manager());
```

### F13 — a feature flag that gates nothing

Today (`Cargo.toml` / `src/lib.rs`):

```toml
default = ["llamacpp", "candle", "agentic"]
agentic = []
```

```rust
pub mod agentic;   // no #[cfg(feature = "agentic")]
pub mod harness;   // none here either; no `feature = "agentic"` anywhere in src/
```

Proposed (item 17): see §3.

## 2. Target shape

The same program, after the proposal:

```rust
let agent = AgentSession::builder(AnthropicMessagesProvider::api_key(api_key))
    .with_model("claude-sonnet-4-6")
    .build()?;

println!("{}", agent.ask("Hi")?);
agent.end()?;
```

And a persistent, tool-using agent:

```rust
let turso = TursoStorage::new("agent.db")?;
turso.init_schema()?;

let agent = AgentSession::builder(router)
    .with_session_id(id)                         // omitted → a new SessionId; reused → resume
    .with_doc_store(SqlDocumentStore::new(turso.clone()))
    .with_memory_store(KvMemoryStore::new(turso))
    .with_model("claude-sonnet-4-6")
    .with_fallback_models(["gpt-4o"])
    .with_system_prompt("You are a coding assistant.")
    .with_tools(ToolPreset::files(fs) + ToolPreset::shell())
    .with_tool(MyTool)
    .build()?;

// Tools that need the built session are registered afterwards, as today.
ToolPreset::search_context(&agent).register_all(agent.tool_manager());

for event in agent.run_turn_stream("Fix the failing test")?.events() {
    match event {
        TurnEvent::Text(delta) => print!("{delta}"),
        TurnEvent::Retract { .. } => clear_current_answer(),
        TurnEvent::ToolCall { name, .. } => eprintln!("→ {name}"),
        TurnEvent::Done(summary) => eprintln!("{} tokens", summary.usage.total),
        _ => {}
    }
}
```

The builder keeps the existing `with_*` method names; the changes are new
methods (`with_session_id`, `with_tool`, `with_tools`), wider argument types
(`impl Into<ModelId>`, `impl Into<Messages>`, `impl Into<ProviderRouter>`) and
store setters that change the builder's type. `with_toolshed` stays.

## 3. Proposed changes

### Tier 1 — remove the traps (small, mostly additive)

#### 1. Builder-owned tools (F2) — keep `with_toolshed`, add `with_tool` / `with_tools`

`with_toolshed(ToolShed)` stays exactly as it is: callers can still set the
shed directly, and preflight still checks that every entry in it has an
implementation. Two methods are added that take implementations, register
them on the session's `ToolCallManager` inside `build()` (before preflight),
and add each tool's `definition()` to the shed.

Today:

```rust
pub struct AgentSessionBuilder<D, M> { toolshed: ToolShed, /* … */ }

pub fn with_toolshed(mut self, toolshed: ToolShed) -> Self { self.toolshed = toolshed; self }

// build(): a fresh, empty manager, then preflight against self.toolshed
let tool_manager = ToolCallManager::new(session_id.clone());
```

Proposed:

```rust
pub struct AgentSessionBuilder<D, M> {
    toolshed: ToolShed,              // unchanged
    tools: Vec<Arc<dyn ToolImpl>>,   // new
    /* … */
}

/// Set the shed directly (unchanged). Entries must be backed by an
/// implementation — from `with_tool`/`with_tools` — or preflight fails.
pub fn with_toolshed(mut self, toolshed: ToolShed) -> Self { self.toolshed = toolshed; self }

/// Register one tool.
pub fn with_tool<T: ToolImpl + 'static>(mut self, tool: T) -> Self {
    self.tools.push(Arc::new(tool));
    self
}

/// Register a list of tools: a `Vec<Arc<dyn ToolImpl>>`, a `ToolPreset`, …
pub fn with_tools(mut self, tools: impl IntoIterator<Item = Arc<dyn ToolImpl>>) -> Self {
    self.tools.extend(tools);
    self
}

// build(), before preflight:
let tool_manager = ToolCallManager::new(session_id.clone());
let mut toolshed = self.toolshed;
for tool in self.tools {
    let def = tool.definition();
    if !toolshed.tools.iter().any(|t| t.name() == def.name()) {
        toolshed.tools.push(def);
    }
    tool_manager.register(tool);
}
// preflight() is unchanged: every shed entry must be registered — now it is.
```

So `ToolPreset` can be passed straight in:

```rust
impl IntoIterator for ToolPreset {
    type Item = Arc<dyn ToolImpl>;
    type IntoIter = std::vec::IntoIter<Arc<dyn ToolImpl>>;
    fn into_iter(self) -> Self::IntoIter { self.tools.into_iter() }
}
```

`agent.tool_manager().register(..)` after `build()` keeps working for tools
that need the session itself (memory, `search_context`).

#### 2. Typestate stores, no `Default` bound (F1)

Today:

```rust
pub struct AgentSession<D, M> { inner: Arc<SessionInner<D, M>> }
pub struct AgentSessionBuilder<D, M> { doc_store: Option<D>, memory_store: Option<M>, /* … */ }

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    pub fn builder(session_id: SessionId, router: ProviderRouter) -> AgentSessionBuilder<D, M>;
}
pub fn with_doc_store(mut self, store: D) -> Self;
pub fn with_memory_store(mut self, store: M) -> Self;
pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>
where D: Default, M: Default;
```

Proposed — the builder starts with the in-memory stores, and the setters
change the type:

```rust
pub struct AgentSession<D = MemoryDocumentStore, M = KvMemoryStore<MemoryStorage>> { /* … */ }
pub struct AgentSessionBuilder<D = MemoryDocumentStore, M = KvMemoryStore<MemoryStorage>> {
    doc_store: D,       // no Option: always filled
    memory_store: M,
    /* … */
}

// One concrete impl, so `AgentSession::builder(..)` infers without a turbofish.
impl AgentSession {
    pub fn builder(router: impl Into<ProviderRouter>) -> AgentSessionBuilder {
        AgentSessionBuilder::new(
            router.into(),
            MemoryDocumentStore::new(),
            KvMemoryStore::new(MemoryStorage::new()),
        )
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSessionBuilder<D, M> {
    pub fn with_doc_store<D2: DocumentStore + 'static>(self, store: D2) -> AgentSessionBuilder<D2, M>;
    pub fn with_memory_store<M2: MemoryStore + 'static>(self, store: M2) -> AgentSessionBuilder<D, M2>;
    pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>; // no Default bound
}
```

Harness bridges lose their generics the same way:

```rust
// today
pub fn into_agent_builder<D, M>(self, session_id: SessionId) -> AgentSessionBuilder<D, M>;
// proposed
pub fn into_agent_builder(self) -> AgentSessionBuilder; // then .with_session_id / .with_doc_store
```

#### 3. One store graph (F1, plus the memory bug)

Today `build()` hands your stores to `MessageApi` and `ContextProvider` and
gives the memory hierarchy its own:

```rust
let message_api = MessageApi::new(session_id.clone(), doc_store);
let context_provider = ContextProvider::new(session_id.clone(), message_api.clone(),
    Arc::clone(&memory_store_arc), ledger.clone(), self.system_prompt, self.context_config);
let coordinator = MemoryCoordinator::new(M::default(), D::default()); // second store graph
```

Proposed — one `Arc` per store, shared by all three:

```rust
let doc_store = Arc::new(self.doc_store);
let memory_store = Arc::new(self.memory_store);

let message_api = MessageApi::from_shared(session_id.clone(), Arc::clone(&doc_store));
let context_provider = ContextProvider::new(session_id.clone(), message_api.clone(),
    Arc::clone(&memory_store), ledger.clone(), self.system_prompt, self.context_config);
let coordinator = MemoryCoordinator::from_shared(memory_store, doc_store);
```

#### 4. Resume is just build (F7)

Today: `AgentSession::<D, M>::resume(session_id, router, config, policy)` (see
F7) and `AgentSession::<D, M>::builder(session_id, router)`.

Proposed:

```rust
pub fn with_session_id(mut self, id: SessionId) -> Self; // default: SessionId::new()

// build(): context assembly already reads the session's records from the doc
// store each turn, so an existing id over persistent stores resumes.

#[deprecated(note = "use AgentSession::builder(router).with_session_id(id)…build()")]
pub fn resume(session_id: SessionId, router: ProviderRouter, config: AgentConfig,
              policy: Option<ErrorPolicy>) -> Result<AgentSession, ErrorTrace<AgenticError>>;

#[deprecated(note = "use AgentSession::builder(router).with_session_id(id)")]
pub fn builder_for(session_id: SessionId, router: ProviderRouter) -> AgentSessionBuilder; // old 2-arg builder
```

Changing `builder`'s arguments is the one breaking edit in Tier 1;
`builder_for` keeps old call sites compiling for a release.

#### 5. Expose recall (F12)

Today: no accessor; see the hand-built `ContextProvider` in F12.

Proposed:

```rust
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    pub fn context_provider(&self) -> &ContextProvider<D, M>;
}

impl ToolPreset {
    /// `search_context` over the session's own stores and embedder.
    pub fn search_context<D, M>(agent: &AgentSession<D, M>) -> Self
    where D: DocumentStore + 'static, M: MemoryStore + 'static
    {
        Self::from_tools(vec![Arc::new(SearchContextTool::new(agent.context_provider().clone()))])
    }
}

// call site
ToolPreset::search_context(&agent).register_all(agent.tool_manager());
```

### Tier 2 — make the common path short (additive)

#### 6. Message constructors (F3)

Today: only the struct literal (F3), or `testing::mock_user` behind a feature.

Proposed:

```rust
impl Messages {
    pub fn user(text: impl Into<String>) -> Self { Self::text_with_role(MessageRole::User, text) }
    pub fn system(text: impl Into<String>) -> Self { Self::text_with_role(MessageRole::System, text) }
    pub fn agent(text: impl Into<String>) -> Self { Self::text_with_role(MessageRole::Agent, text) }

    fn text_with_role(role: MessageRole, text: impl Into<String>) -> Self {
        Messages::User {
            id: new_scru128(),
            role,
            content: UserModelContent::Text(TextContent { content: text.into(), signature: None }),
            signature: None,
        }
    }
}
impl From<&str> for Messages { fn from(s: &str) -> Self { Messages::user(s) } }
impl From<String> for Messages { fn from(s: String) -> Self { Messages::user(s) } }

// session.rs — `Messages` still works, since Messages: Into<Messages>
pub fn run_turn(&self, prompt: impl Into<Messages>) -> Result<Turn, ErrorTrace<AgenticError>>;
pub fn run_turn_stream(&self, prompt: impl Into<Messages>) -> Result<TurnStream<D, M>, ErrorTrace<AgenticError>>;
pub fn steer(&self, msg: impl Into<Messages>);
pub fn follow_up(&self, msg: impl Into<Messages>);
```

#### 7. Turn results (F4)

Today:

```rust
pub fn run_turn(&self, prompt: Messages) -> Result<Vec<SessionRecord>, ErrorTrace<AgenticError>>;
```

Proposed — a wrapper, so existing code that indexes or iterates keeps compiling.
**Decided:** `Turn` wraps `Vec<SessionRecord>` (via `Deref` and `IntoIterator`)
rather than replacing it, so nothing that uses today's return value breaks:

```rust
pub struct Turn { records: Vec<SessionRecord> }

impl std::ops::Deref for Turn { type Target = Vec<SessionRecord>; /* … */ }
impl IntoIterator for Turn { type Item = SessionRecord; /* … */ }
impl<'a> IntoIterator for &'a Turn { type Item = &'a SessionRecord; /* … */ } // `for r in &turn`

impl Turn {
    /// Concatenated `ModelOutput::Text` of the assistant messages (retracted output already dropped).
    pub fn text(&self) -> String;
    /// Each `ModelOutput::ToolCall` the model made: (id, name, arguments).
    pub fn tool_calls(&self) -> impl Iterator<Item = (&str, &str, Option<&HashMap<String, ArgType>>)>;
    /// Each `Messages::ToolResult` produced this turn.
    pub fn tool_results(&self) -> impl Iterator<Item = &Messages>;
    /// The `SessionRecord::Summary` usage, if the turn reached it.
    pub fn usage(&self) -> Option<&TokenSnapshot>;
    pub fn into_records(self) -> Vec<SessionRecord>;
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    pub fn ask(&self, prompt: impl Into<Messages>) -> Result<String, ErrorTrace<AgenticError>> {
        Ok(self.run_turn(prompt)?.text())
    }
}
```

#### 8. Streaming events (F4)

Today `run_turn_stream` returns the raw valtron iterator, whose items are
`Stream<SessionRecord, AgentProgress>` (see the F4 match).

Proposed — a thin wrapper that is still the raw iterator, plus `events()`:

```rust
pub struct TurnStream<D, M>(DrivenStreamIterator<AgentLoop<D, M>>);

impl<D, M> Iterator for TurnStream<D, M> {           // raw items, unchanged
    type Item = Stream<SessionRecord, AgentProgress>;
    fn next(&mut self) -> Option<Self::Item> { self.0.next() }
}

impl<D, M> TurnStream<D, M> {
    pub fn events(self) -> impl Iterator<Item = TurnEvent>;
}

#[non_exhaustive]
pub enum TurnEvent {
    /// Assistant text as the record carried it (a delta for streaming providers).
    Text(String),
    Thinking(String),
    ToolCall { id: String, name: String, arguments: Option<HashMap<String, ArgType>> },
    ToolResult { tool_call_id: String, name: String, content: UserModelContent },
    /// Drop the assistant text shown for this turn; the retry follows.
    Retract { reason: String },
    Progress(AgentProgress),
    Failed(AgenticError),
    Done(TurnSummary),
}

pub struct TurnSummary { pub message_count: u64, pub usage: TokenSnapshot }
```

The mapping `events()` applies:

```rust
match item {
    Stream::Next(rec) => record_to_events(rec),                 // Conversation/Retracted/FailedAction/Summary
    Stream::Spread(items) => items.into_iter()
        .filter_map(|s| match s { StreamSpread::Done(rec) => Some(rec), _ => None })
        .flat_map(record_to_events).collect(),
    Stream::Pending(progress) => vec![TurnEvent::Progress(progress)],
    Stream::Init | Stream::Ignore | Stream::Wait | Stream::Delayed(_) => vec![],
}
```

Memory records (`WorkingMemory`, `Observation`, `Reflection`) are skipped by
`events()`; they stay visible on the raw iterator.

#### 9. String model ids (F5)

Today:

```rust
pub fn with_model(mut self, model: ModelId) -> Self;
pub fn with_fallback_models(mut self, models: Vec<ModelId>) -> Self;
pub fn with_memory_model(mut self, model: ModelId) -> Self;
// call site
.with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
```

Proposed:

```rust
impl From<&str> for ModelId { fn from(s: &str) -> Self { ModelId::Name(s.into(), None) } }
impl From<String> for ModelId { fn from(s: String) -> Self { ModelId::Name(s, None) } }

pub fn with_model(mut self, model: impl Into<ModelId>) -> Self;
pub fn with_fallback_models<I>(mut self, models: I) -> Self
where I: IntoIterator, I::Item: Into<ModelId>;     // Vec<ModelId> still accepted
pub fn with_memory_model(mut self, model: impl Into<ModelId>) -> Self;

// call site
.with_model("claude-sonnet-4-6").with_fallback_models(["gpt-4o"])
```

#### 10. Router conversions (F5)

Today:

```rust
pub fn single(provider: Box<dyn RoutableProvider>) -> Self;                          // ProviderRouter
pub fn add_provider(mut self, provider: Box<dyn RoutableProvider>) -> Self;          // ProviderRouterBuilder
```

Proposed:

```rust
// `ModelProvider` is a local trait and `ProviderRouter` doesn't implement it,
// so this doesn't overlap with `impl<T> From<T> for T`.
impl<P> From<P> for ProviderRouter
where
    P: ModelProvider + Send + Sync + 'static,
    P::Model: Send + Sync,
{
    fn from(p: P) -> Self { ProviderRouter::single(Box::new(RoutableProviderBox::new(p))) }
}

impl ProviderRouterBuilder {
    pub fn provider<P>(self, p: P) -> Self
    where P: ModelProvider + Send + Sync + 'static, P::Model: Send + Sync
    { self.add_provider(Box::new(RoutableProviderBox::new(p))) }
}

// AgentSession::builder takes `impl Into<ProviderRouter>` (item 2):
AgentSession::builder(provider);
AgentSession::builder(ProviderRouter::builder().provider(anthropic).provider(openai).build());
```

#### 11. API-key constructors (F6)

Today: `XConfig::new().with_auth(AuthCredential::SecretOnly(ConfidentialText::new(key)))`
for `AnthropicConfig`, `OpenAIConfig` and `ResponsesConfig` (F6).

Proposed:

```rust
impl AnthropicConfig {
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::new().with_auth(AuthCredential::SecretOnly(ConfidentialText::new(key.into())))
    }
    /// Reads `ANTHROPIC_API_KEY`.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self::api_key(std::env::var("ANTHROPIC_API_KEY")?))
    }
}
// Same pair on OpenAIConfig (OPENAI_API_KEY) and ResponsesConfig (OPENAI_API_KEY).
// OpenRouter goes through OpenAIConfig today (examples/hello_openrouter.rs sets
// with_base_url("https://openrouter.ai/api/v1") by hand), so add:
impl OpenAIConfig {
    /// Reads `OPENROUTER_API_KEY` and points at the OpenRouter base URL.
    pub fn openrouter_from_env() -> Result<Self, std::env::VarError>;
}

impl AnthropicMessagesProvider {
    pub fn api_key(key: impl Into<String>) -> Self { Self::with_config(AnthropicConfig::api_key(key)) }
}
// Same on OpenAIProvider and ResponsesProvider.
```

#### 12. Typed tool arguments (F9)

Today: three `json_value_to_arg_type` functions (F9) and per-module helpers:

```rust
// agentic/tools/files.rs (memory.rs and agent.rs have their own copies)
fn text_arg(args: &HashMap<String, ArgType>, key: &str, tool: &str) -> Result<String, ToolError>;
fn opt_usize(args: &HashMap<String, ArgType>, key: &str) -> Option<usize>;
```

Proposed — one conversion, used by every backend and the text formatter:

```rust
// backends/backend_utils.rs — the only copy
pub fn json_value_to_arg_type(v: &serde_json::Value) -> ArgType {
    match v {
        serde_json::Value::String(s) => ArgType::Text(s.clone()),
        serde_json::Value::Number(n) => n.as_i64().map(ArgType::I64)
            .or_else(|| n.as_f64().map(ArgType::Float64))
            .unwrap_or_else(|| ArgType::Text(n.to_string())),
        serde_json::Value::Object(map) => ArgType::JSONMap(
            map.iter().map(|(k, v)| (k.clone(), json_value_to_arg_type(v))).collect()),
        other => ArgType::JSON(other.to_string()), // bool, null, array: one rule everywhere
    }
}
```

and a reader tools use instead of matching `ArgType`:

```rust
pub struct ToolArgs<'a> { tool: &'a str, args: &'a HashMap<String, ArgType> }

impl<'a> ToolArgs<'a> {
    pub fn new(tool: &'a str, args: &'a HashMap<String, ArgType>) -> Self;
    pub fn str(&self, key: &str) -> Result<&'a str, ToolError>;      // ToolError::InvalidArguments
    pub fn i64(&self, key: &str) -> Result<i64, ToolError>;          // any integer variant, or numeric text
    pub fn f64(&self, key: &str) -> Result<f64, ToolError>;
    pub fn bool(&self, key: &str) -> Result<bool, ToolError>;        // JSON("true") or Text("true")
    pub fn opt_str(&self, key: &str) -> Result<Option<&'a str>, ToolError>;
    pub fn opt_i64(&self, key: &str) -> Result<Option<i64>, ToolError>;
    pub fn opt_bool(&self, key: &str) -> Result<Option<bool>, ToolError>;
    /// All arguments as one struct.
    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<T, ToolError>;
}

// call site
#[derive(Deserialize)]
struct EditArgs { path: String, old_string: String, new_string: String, #[serde(default)] replace_all: bool }
let EditArgs { path, old_string, new_string, replace_all } = ToolArgs::new("edit", &arguments).parse()?;
```

Longer term, `ArgType` collapses to `serde_json::Value` (open question below).

#### 13. Closure tools (F9)

Today every tool is a struct with an `impl ToolImpl` (see `examples/agent_with_tools.rs`):

```rust
struct GreetTool;

#[async_trait]
impl ToolImpl for GreetTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "greet".into(),
            category: "custom".into(),
            description: "Greet someone by name.".into(),
            arguments: Args::new(scheme::object().required("name", scheme::string()).build()),
            returns: None,
        })
    }
    async fn execute(&self, arguments: HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let name = match arguments.get("name") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => return Err(ToolError::InvalidArguments { tool: "greet".into(), reason: "missing 'name'".into() }),
        };
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent { content: format!("Hello, {name}!"), signature: None }),
            error_detail: None,
        })
    }
}
```

Proposed:

```rust
impl ToolCallResult {
    pub fn text(content: impl Into<String>) -> Self; // replaces the per-module `text_result` helpers
}

let greet = FnTool::new(
    "greet",
    "Greet someone by name.",
    Args::new(scheme::object().required("name", scheme::string()).build()),
    |args: ToolArgs<'_>| {
        let name = args.str("name").map(str::to_owned);
        async move { Ok(ToolCallResult::text(format!("Hello, {}!", name?))) }
    },
);
builder.with_tool(greet);   // FnTool: ToolImpl, category defaults to "custom"
```

### Tier 3 — consolidate (breaking; do once, with a migration note)

#### 14. Single source for models (F8)

Today:

```rust
pub struct AgentConfig {
    pub primary_model: ModelId,
    pub fallback_models: Vec<ModelId>,
    pub memory_model: Option<ModelId>,
    pub max_inner_iterations: usize,
    pub max_outer_iterations: usize,
    pub circuit_breaker_threshold: u32,
    pub preflight_compression_threshold: f32,
    pub context_pressure_threshold: f32,
    pub model_params: ModelParams,
}
```

Proposed — the model fields move to the builder (`with_model`,
`with_fallback_models`, `with_memory_model`); the loop gets them from a
separate value the builder fills:

```rust
pub struct AgentConfig {
    pub max_inner_iterations: usize,
    pub max_outer_iterations: usize,
    pub circuit_breaker_threshold: u32,
    pub preflight_compression_threshold: f32,
    pub context_pressure_threshold: f32,
    pub model_params: ModelParams,
}

pub struct ModelSelection {                 // built only by AgentSessionBuilder
    pub primary: ModelId,
    pub fallbacks: Vec<ModelId>,
    pub memory: Option<ModelId>,
}
```

`build()` fails with `AgenticError::Session("no model set")` when
`with_model` was never called, instead of today's empty
`ModelId::Name(String::new(), None)` default.

#### 15. One error type at the public boundary (F11)

Today: harness `Result<_, String>`, session `ErrorTrace<AgenticError>`
(F11).

Proposed — harness functions return the session's error type; provider
construction failures get their own variant:

```rust
pub enum AgenticError {
    /* existing variants … */
    /// A provider or preset couldn't be constructed (download, config, auth).
    Provider(String),
}

// harness/agents.rs
pub fn claude_router(api_key: &str) -> Result<RouterPreset, ErrorTrace<AgenticError>>;
pub fn glm52_gemma_router(main: Option<HuggingFaceGGUFConfig>, memory: Option<HuggingFaceGGUFConfig>)
    -> Result<RouterPreset, ErrorTrace<AgenticError>>;
// harness/providers.rs
pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, ErrorTrace<AgenticError>>;

// caller: one `?` type end to end
fn main() -> Result<(), ErrorTrace<AgenticError>> {
    let agent = RouterPreset::claude(&key)?.into_agent_builder().build()?;
    println!("{}", agent.ask("Hi")?);
    agent.end()
}
```

#### 16. Name cleanup (F10)

Today:

```rust
// agentic/errors.rs
pub struct LoopDetection { pub kind: String, pub occurrences: u32 }
pub enum AgenticError { /* … */ LoopDetected(LoopDetection), /* … */ }
// agentic/mod.rs
pub use errors::{ /* … */ LoopDetection, /* … */ };
pub use loop_detection::{ /* … */ LoopDetection as InlineLoopDetection, /* … */ };
pub use tool_impl::{ /* … */ ToolDefinition, /* … */ };  // re-export of types::ToolDefinition
```

Proposed:

```rust
// agentic/errors.rs
pub struct LoopDetectedInfo { pub kind: String, pub occurrences: u32 }
pub enum AgenticError { /* … */ LoopDetected(LoopDetectedInfo), /* … */ }
// agentic/mod.rs
pub use errors::{ /* … */ LoopDetectedInfo, /* … */ };
pub use loop_detection::{ /* … */ LoopDetection, /* … */ };      // no alias
pub use tool_impl::{ /* … */ ToolImpl, /* … */ };                 // ToolDefinition only via `types`
```

#### 17. Feature flag (F13)

Today: `agentic = []` in the default features, gating nothing (F13).

Proposed (option A — make it real):

```rust
// src/lib.rs
#[cfg(feature = "agentic")]
pub mod agentic;
#[cfg(feature = "agentic")]
pub mod harness;   // harness builds AgentSessionBuilder, so it needs agentic
```

Proposed (option B — delete it):

```toml
default = ["llamacpp", "candle"]
# agentic = []   removed
```

#### 18. Narrow the public surface

Today `agentic/mod.rs` re-exports ~60 items, loop internals included:

```rust
pub use agent_loop::{AgentConfig, AgentLoop, AgentLoopState};
pub use context::{AgentContext, ContextConfig, ContextProvider, KnowledgeHit, SearchMode};
pub use memory_coordinator::MemoryCoordinator;
pub use steering::{CancelCode, SteeringQueues};
pub use tool_impl::{FailMode, ToolCallManager, ToolCallRequest, ToolCallResult, ToolCallStage,
    ToolCallWorkflow, ToolDefinition, ToolError, ToolErrorKind, ToolImpl, ToolRetryConfig, WorkflowResult};
/* … */
```

Proposed:

```rust
// agentic/mod.rs — what an application needs
pub use access::{AllowAllAccess, SessionAccessProvider, TokenBudget};
pub use agent_loop::AgentConfig;
pub use errors::{AgenticError, ErrorPolicy, UserId};
pub use memory_store::{KvMemoryStore, MemoryStore};
pub use progress::AgentProgress;
pub use session::{AgentSession, AgentSessionBuilder, Turn, TurnEvent, TurnStream, TurnSummary};
pub use tool_impl::{FnTool, ToolArgs, ToolCallResult, ToolError, ToolImpl};
pub use crate::harness::ToolPreset;

/// Loop internals, for custom loops and tests. Not covered by semver.
pub mod internals {
    pub use super::agent_loop::{AgentLoop, AgentLoopState};
    pub use super::context::ContextProvider;
    pub use super::memory_coordinator::MemoryCoordinator;
    pub use super::steering::SteeringQueues;
    pub use super::tool_impl::{ToolCallManager, ToolCallWorkflow, ToolCallRequest};
    /* … */
}
```

#### 19. Harness collapse

Today each model mix is a pair of free functions:

```rust
pub fn claude_router(api_key: &str) -> Result<RouterPreset, String>;
pub fn claude_session<D, M>(session_id: SessionId, api_key: &str) -> Result<AgentSessionBuilder<D, M>, String>;
// + glm52_gemma_*, qwen36_gemma_*, gemma_*, openai_chat_*, openai_responses_*, candle_llama_*
let agent = harness::claude_session::<Doc, Mem>(SessionId::new(), &key)?.build()?;
```

Proposed — constructors on `RouterPreset`; the `*_session` functions go away
because `into_agent_builder` (item 2) already is the bridge:

```rust
impl RouterPreset {
    pub fn claude(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>>;
    pub fn openai_chat(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>>;
    pub fn openai_responses(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>>;
    pub fn glm52_gemma(main: Option<HuggingFaceGGUFConfig>, memory: Option<HuggingFaceGGUFConfig>)
        -> Result<Self, ErrorTrace<AgenticError>>;
    pub fn qwen36_gemma(/* same */) -> Result<Self, ErrorTrace<AgenticError>>;
    pub fn gemma(/* same */) -> Result<Self, ErrorTrace<AgenticError>>;
}

let agent = RouterPreset::claude(&key)?
    .into_agent_builder()
    .with_tools(ToolPreset::files(fs))
    .build()?;
```

## 4. Suggested order

1. Tier 1 items 1–4 (they overlap with the bug fixes and unblock persistence).
2. Tier 2 items 6–9 (biggest line-count win for users, all additive).
3. Update the docs and examples to the new shape; keep the old calls
   (`resume`, the two-argument `builder` as `builder_for`) as `#[deprecated]`
   for one release. `with_toolshed` is not deprecated.
4. Tier 2 items 10–13.
5. Tier 3 in one breaking release.

## 5. Open questions

- Should resume be implicit (reusing an id rehydrates) or explicit
  (`.resume(id)`)? Implicit is simpler; explicit is clearer when an id is
  reused by mistake.
- Is the `ArgType` → `serde_json::Value` move worth the breakage, or is the
  `ToolArgs` wrapper enough?
- Do we want `agent.ask` to error on a turn that ends in a `FailedAction`
  after some text, or return the partial text? (`run_turn` already returns
  `Err` on the first `FailedAction`, so `ask` as written above errors.)
- When `with_toolshed` and `with_tools` name the same tool, the shed's entry
  is kept (item 1). Should a mismatch between the shed's definition and the
  tool's own `definition()` be a preflight error?

## 6. Proposed names at a glance

| Where | Name | Item |
|---|---|---|
| `AgentSession` | `builder(impl Into<ProviderRouter>)`, `ask`, `context_provider`, `#[deprecated] builder_for`, `#[deprecated] resume` | 2, 4, 5, 7 |
| `AgentSession` | `run_turn(impl Into<Messages>) -> Turn`, `run_turn_stream(impl Into<Messages>) -> TurnStream` | 6, 7, 8 |
| `AgentSessionBuilder` | `with_toolshed` (kept), `with_tool`, `with_tools`, `with_session_id` | 1, 4 |
| `AgentSessionBuilder` | `with_doc_store` / `with_memory_store` (change the type), `with_model` / `with_fallback_models` / `with_memory_model` (take `Into<ModelId>`) | 2, 9 |
| Turn results | `Turn`, `TurnStream`, `TurnEvent`, `TurnSummary` | 7, 8 |
| Messages / ids | `Messages::{user, system, agent}`, `From<&str>`/`From<String>` for `Messages` and `ModelId` | 6, 9 |
| Providers | `From<P: ModelProvider> for ProviderRouter`, `ProviderRouterBuilder::provider`, `{Anthropic,OpenAI,Responses}Config::{api_key, from_env}`, `{AnthropicMessages,OpenAI,Responses}Provider::api_key` | 10, 11 |
| Tools | `ToolArgs`, `FnTool`, `ToolCallResult::text`, `ToolPreset: IntoIterator`, `ToolPreset::search_context` | 1, 5, 12, 13 |
| Stores | `MessageApi::from_shared`, `MemoryCoordinator::from_shared` | 3 |
| Tier 3 | `ModelSelection`, `AgenticError::Provider`, `LoopDetectedInfo`, `agentic::internals`, `RouterPreset::{claude, openai_chat, …}` | 14–19 |
