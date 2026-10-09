# Proposal 15 — Simplifying the `foundation_ai` API surface

**Status:** proposal for discussion. Nothing here is implemented. The bug fixes
for the "Known limitations" in Doc 00 are handled separately; this doc is
about making the everyday API smaller and harder to misuse.

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

| # | Friction | Why it hurts |
|---|---|---|
| F1 | `AgentSession<D, M>` generics with turbofish everywhere, plus a `D: Default, M: Default` bound on `build()` | Persistent stores can't be used at all; every signature carries two type parameters |
| F2 | `with_toolshed(ToolShed)` exists, but tools must be registered on `tool_manager()` after `build()` | The obvious path fails preflight |
| F3 | Building a user message takes 4 fields and a nested struct | Every example repeats the same 8 lines |
| F4 | Getting the answer text means matching three nested enums, and streaming consumers must handle `Retracted` | Easy to get wrong; the common case has no helper |
| F5 | `RoutableProviderBox::new` + `Box::new` + `ProviderRouter::single` for one provider | Three wrappers to say "use this provider" |
| F6 | `AuthCredential::SecretOnly(ConfidentialText::new(key))` | The common case (an API key) is the hardest to type |
| F7 | `resume()` is a separate constructor that ignores stores, prompt, tools and configs | It can't do its one job |
| F8 | `AgentConfig` and the builder both set `primary_model`, `fallback_models`, `memory_model` | Two sources of truth; the builder silently wins |
| F9 | `ArgType` has 18 variants, and backends map booleans/arrays/null differently | Every tool needs defensive matching |
| F10 | Name collisions: two `LoopDetection` types (one re-exported as `InlineLoopDetection`), `ToolDefinition` reachable from two modules | Confusing imports |
| F11 | Harness returns `Result<_, String>`, sessions return `ErrorTrace<AgenticError>`, providers return their own errors | `?` needs `Box<dyn Error>` everywhere |
| F12 | The session's `ContextProvider` isn't reachable, so `with_embedder` can't power `search_context` | A builder option with no observable effect |
| F13 | The `agentic` feature gates no code | A flag that does nothing |

## 2. Target shape

The same program, after the proposal:

```rust
let agent = AgentSession::builder(Anthropic::api_key(api_key))
    .model("claude-sonnet-4-6")
    .build()?;

println!("{}", agent.ask("Hi")?);
agent.end()?;
```

And a persistent, tool-using agent:

```rust
let agent = AgentSession::builder(router)
    .session_id(id)                         // omitted → a new SessionId; reused → resume
    .doc_store(SqlDocumentStore::new(turso.clone()))
    .memory_store(KvMemoryStore::new(turso))
    .model("claude-sonnet-4-6")
    .fallback("gpt-4o")
    .system_prompt("You are a coding assistant.")
    .tools(ToolPreset::files(fs) + ToolPreset::shell())
    .tool(MyTool)
    .build()?;

for event in agent.turn("Fix the failing test")?.events() {
    match event {
        TurnEvent::Text(delta) => print!("{delta}"),
        TurnEvent::Retract => clear_current_answer(),
        TurnEvent::ToolCall { name, .. } => eprintln!("→ {name}"),
        TurnEvent::Done(summary) => eprintln!("{} tokens", summary.usage.total),
        _ => {}
    }
}
```

## 3. Proposed changes

### Tier 1 — remove the traps (small, mostly additive)

1. **Builder-owned tools (F2).** Add `.tool(impl ToolImpl)` and
   `.tools(ToolPreset)` to `AgentSessionBuilder`; register them before
   preflight. Deprecate `with_toolshed`; the shed is derived from the
   registry anyway.
2. **Typestate stores, no `Default` bound (F1).** Start the builder with the
   in-memory stores as type defaults and let `doc_store(d)` /
   `memory_store(m)` change the type:
   `fn doc_store<D2: DocumentStore>(self, d: D2) -> AgentSessionBuilder<D2, M>`.
   Add default type parameters
   `AgentSession<D = MemoryDocumentStore, M = KvMemoryStore<MemoryStorage>>`
   so in-memory users never write generics.
3. **One store graph (F1, plus the memory bug).** Keep the stores in `Arc`s
   and give `MessageApi`, `ContextProvider` and `MemoryCoordinator` the same
   handles.
4. **Resume is just build (F7).** `.session_id(existing)` on the builder
   rehydrates from the supplied stores. Deprecate `AgentSession::resume`.
5. **Expose recall (F12).** `ToolPreset::search_context(&agent)`, built from
   the session's own `ContextProvider`.

### Tier 2 — make the common path short (additive)

6. **Message constructors (F3).** `Messages::user(text)`,
   `Messages::system(text)`, `Messages::agent(text)`;
   `impl From<&str> / From<String> for Messages`; `run_turn` /
   `run_turn_stream` take `impl Into<Messages>`.
7. **Turn results (F4).** `run_turn` returns a `Turn` (a thin wrapper around
   `Vec<SessionRecord>` that derefs to it) with `text()`, `tool_calls()`,
   `tool_results()`, `usage()`. Add `agent.ask(text) -> Result<String>` for the
   one-shot case.
8. **Streaming events (F4).** `run_turn_stream(..)?.events()` yields a small
   `TurnEvent` enum: `Text`, `Thinking`, `ToolCall`, `ToolResult`, `Retract`,
   `Progress(AgentProgress)`, `Failed`, `Done(Summary)`. Valtron scheduling
   items (`Init`, `Wait`, `Delayed`, `Ignore`) and `Spread` are flattened
   away. The raw stream stays available.
9. **String model ids (F5).** `impl From<&str> for ModelId` (→ `Name(s, None)`)
   and `impl<T: Into<ModelId>>` on `with_model` / `fallback` / `memory_model`.
10. **Router conversions (F5).** `impl<P: ModelProvider + …> From<P> for
    ProviderRouter` (single provider), and
    `ProviderRouterBuilder::provider(p)` that boxes for you.
    `AgentSession::builder` takes `impl Into<ProviderRouter>`.
11. **API-key constructors (F6).** `AnthropicConfig::api_key(key)`,
    `OpenAIConfig::api_key(key)`, `ResponsesConfig::api_key(key)`, plus
    `from_env()` (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`).
12. **Typed tool arguments (F9).** Normalise every backend through one
    `json_value_to_arg_type`, and add a `ToolArgs` wrapper with
    `str(key)`, `i64(key)`, `f64(key)`, `bool(key)`, `parse::<T: Deserialize>()`
    so tools stop matching on `ArgType`. Longer term, collapse `ArgType` to
    `serde_json::Value`.
13. **Closure tools.** `FnTool::new(name, description, Args, |args| async { … })`
    for tools that don't need their own struct.

### Tier 3 — consolidate (breaking; do once, with a migration note)

14. **Single source for models (F8).** Remove `primary_model`,
    `fallback_models` and `memory_model` from `AgentConfig`; they live on the
    builder only.
15. **One error type at the public boundary (F11).** Harness functions return
    `ErrorTrace<AgenticError>` (or a crate-level `Error` that wraps it, the
    router and the provider errors) instead of `String`.
16. **Name cleanup (F10).** Rename `errors::LoopDetection` →
    `LoopDetectedInfo`; keep `loop_detection::LoopDetection` as the one
    `LoopDetection`. Export `ToolDefinition` from `types` only.
17. **Feature flag (F13).** Either gate `agentic/` and `harness/` behind
    `agentic`, or delete the flag.
18. **Narrow the public surface.** Today `agentic` re-exports ~60 items.
    Users need `AgentSession`, its builder, `ToolImpl`, `ToolPreset`,
    `TurnEvent`/`Turn`, `AgentConfig`, `ErrorPolicy` and the stores. Move the
    loop internals (`AgentLoop`, `AgentLoopState`, `ContextProvider`,
    `MemoryCoordinator`, `SteeringQueues`, …) under `agentic::internals` or
    make them `pub(crate)`.
19. **Harness collapse.** Replace the `*_router` / `*_session` pairs with
    `RouterPreset` constructors (`RouterPreset::claude(key)`,
    `RouterPreset::glm52_gemma(..)`) — the builder bridge already exists as
    `into_agent_builder`.

## 4. Suggested order

1. Tier 1 items 1–4 (they overlap with the bug fixes and unblock persistence).
2. Tier 2 items 6–9 (biggest line-count win for users, all additive).
3. Update the docs and examples to the new shape; keep the old calls as
   `#[deprecated]` for one release.
4. Tier 2 items 10–13.
5. Tier 3 in one breaking release.

## 5. Open questions

- Should `Turn` replace `Vec<SessionRecord>` outright, or wrap it with
  `Deref`? (Proposal: wrap — no breakage.)
- Should resume be implicit (reusing an id rehydrates) or explicit
  (`.resume(id)`)? Implicit is simpler; explicit is clearer when an id is
  reused by mistake.
- Is the `ArgType` → `serde_json::Value` move worth the breakage, or is the
  `ToolArgs` wrapper enough?
- Do we want `agent.ask` to error on a turn that ends in a `FailedAction`
  after some text, or return the partial text?
