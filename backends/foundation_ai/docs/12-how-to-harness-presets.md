# How-To: The `harness` module — one-call model setup

Zero-to-expert guide for `foundation_ai::harness` — pre-configured providers,
router mixing, and ready-to-customize agent builders for the common model
combinations.

The harness exists so you don't hand-wire `HuggingFaceGGUFProvider`,
`RoutableProviderBox`, `RoutingRule`, `ProviderRouter`, and `AgentSessionBuilder`
just to say "GLM 5.2 for chat with a small Gemma for memory." It encodes
sensible defaults while still returning the fully customizable builder.

See also: Doc 03 (model providers / router internals) and Doc 08 (AgentSession).

---

## Quick Start

The shortest path to a working, multi-model agent — a strong main model plus a
small memory model, wired and ready:

```rust
use foundation_ai::harness;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::SessionId;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

// GLM 5.2 for chat + Gemma 4 E2B for memory, as an AgentSession builder.
let agent = harness::glm52_gemma_session::<Doc, Mem>(SessionId::new(), None, None)?
    .with_system_prompt("You are a helpful assistant.")
    .build()?;
```

`None, None` accept the default GGUF config (Q4_K_M, default cache dir); pass
`Some(HuggingFaceGGUFConfig)` per model to override cache dir, GPU layers, etc.

---

## 1. Three layers, pick your altitude

The harness gives you three entry points, from highest-level to lowest:

| Layer | You get | Use when |
|-------|---------|----------|
| `*_session(id, …)` | a wired `AgentSessionBuilder` | you want an agent, fast |
| `*_router(…)` → `RouterPreset` | router + model ids | you want to inspect/mix further |
| `providers::*` + `RouterMix` | raw providers, full control | fully custom combinations |

They compose: `*_session` calls `*_router().into_agent_builder(id)`, and
`*_router` builds a `RouterMix`.

---

## 2. Combo presets (`agents.rs`)

Each combination has a `*_router()` (returns `RouterPreset`) and a `*_session()`
(returns `AgentSessionBuilder`). Local (GGUF) combos take two optional
`HuggingFaceGGUFConfig`s (main, memory); cloud combos take an `api_key`.

| Function | Main model | Memory model | Backend |
|----------|-----------|--------------|---------|
| `glm52_gemma_*` | GLM 5.2 | Gemma 4 E2B | GGUF (llama.cpp) |
| `qwen36_gemma_*` | Qwen 3.6 35B-A3B | Gemma 4 E2B | GGUF |
| `gemma_*` | Gemma 4 26B-A4B | Gemma 4 E2B | GGUF (big + small) |
| `claude_*` | Claude Opus | Claude Sonnet | Anthropic |
| `openai_chat_*` | GPT-4o | GPT-4o-mini | OpenAI Chat Completions |
| `openai_responses_*` | GPT-4o | GPT-4o-mini | OpenAI Responses API |
| `candle_llama_*` | one safetensors model | — | Candle (Llama arch only) |

```rust
// Local (GGUF) — main + memory:
let preset = harness::gemma_router(None, None)?;              // RouterPreset
let builder = harness::gemma_session::<Doc, Mem>(id, None, None)?;

// Cloud:
let preset = harness::claude_router(&api_key)?;
let builder = harness::openai_responses_session::<Doc, Mem>(id, &api_key)?;
```

> **Candle note:** `candle_llama_*` is gated behind the `candle` feature and
> Candle currently implements only the **Llama** architecture. Other
> architectures load-fail as unsupported.

---

## 3. Provider presets (`providers.rs`)

The building blocks. Each local model is a zero-sized struct with a `MODEL_ID`
const and quantization methods returning a `HuggingFaceGGUFProvider`:

```rust
use foundation_ai::harness::providers::{Glm52, Gemma4E2b};

let glm = Glm52::q4_k_m(None)?;         // Q3_K_M / Q4_K_M / Q5_K_M / Q8_0
let mem = Gemma4E2b::q4_k_m(None)?;
assert_eq!(Glm52::MODEL_ID, "unsloth/GLM-5.2-GGUF");
```

Local presets: `Glm52`, `Qwen36`, `Ornith10`, `Gemma4E4b`, `Gemma4_26b`,
`Gemma4E2b`. (Quantization string consts `Q3_K_M`/`Q4_K_M`/`Q5_K_M`/`Q8_0` are
also exported.)

> `Gemma4E2b` points at `unsloth/gemma-4-E2B-it-GGUF` (not the `ggml-org`
> mirror) because the unsloth repo ships the full K-quant ladder; the mirror
> only carries `Q8_0`/`bf16`, so `q4_k_m` etc. would not resolve a file there.

Cloud presets via `CloudPresets` (each takes an `api_key`):

```rust
use foundation_ai::harness::{CloudPresets, CLAUDE_OPUS, OPENAI_GPT4O};

let claude   = CloudPresets::claude_opus(&api_key)?;   // Anthropic
let sonnet   = CloudPresets::claude_sonnet(&api_key)?;
let gpt4o    = CloudPresets::openai_gpt4o(&api_key)?;   // Chat Completions
let resp     = CloudPresets::openai_responses(&api_key)?; // Responses API
```

Cloud model-id constants (`CLAUDE_OPUS`, `CLAUDE_SONNET`, `OPENAI_GPT4O`,
`OPENAI_GPT4O_MINI`) give you the canonical id strings so a routing rule and the
agent's `primary_model`/`memory_model` line up.

---

## 4. `RouterMix` — fully custom mixing

When the presets aren't the exact combination you want, mix providers yourself.
`RouterMix` assigns each provider a **role** (primary / memory / fallback) and a
distinct routing identity, then builds a `ProviderRouter`:

```rust
use foundation_ai::harness::{RouterMix, providers::{Glm52, Gemma4E2b}};
use foundation_ai::types::ModelId;

let preset = RouterMix::new()
    .primary(Glm52::q4_k_m(None)?, ModelId::Name(Glm52::MODEL_ID.into(), None))
    .memory(Gemma4E2b::q4_k_m(None)?, ModelId::Name(Gemma4E2b::MODEL_ID.into(), None))
    // .fallback(...)  // optional, may be called multiple times
    .build();   // -> RouterPreset
```

### Why explicit rules (the important bit)

`RouterMix` pins each model to its provider with an explicit `RoutingRule` and a
distinct provider identity — it does **not** rely on the router's automatic
`serves()` probe. That's deliberate, because `serves()` is inconsistent across
providers:

- `HuggingFaceGGUFProvider::serves()` always returns **false** (no catalog), so
  the probe would never match a local GGUF model.
- `AnthropicMessagesProvider::serves()` always returns **true** (claims every
  id), so in a mixed router it would greedily capture models owned by another
  provider.

Explicit rules make resolution deterministic regardless of probe behavior. Each
role must use a **distinct model id** — the id string doubles as the provider's
routing name (the presets always use distinct ids: e.g. a 26B main model and an
E2B memory model).

> A provider that cannot describe itself has no identity and cannot be routed —
> `RouterMix` **panics** in that case rather than inventing a default, matching
> `RoutableProviderBox::new()`'s contract.

---

## 5. `RouterPreset` — the bridge to an agent

`*_router()` and `RouterMix::build()` return a `RouterPreset`:

```rust
pub struct RouterPreset {
    pub router: ProviderRouter,
    pub primary_model: ModelId,
    pub memory_model: Option<ModelId>,
    pub fallback_models: Vec<ModelId>,
}
```

Inspect/route with it directly, or bridge into an agent — `into_agent_builder`
applies the primary/memory/fallback models for you:

```rust
let preset = harness::claude_router(&api_key)?;

// Inspect routing:
let main = preset.router.resolve(&preset.primary_model)?;   // &dyn RoutableProvider

// Or hand back a builder you finish customizing:
let agent = preset
    .into_agent_builder::<Doc, Mem>(SessionId::new())
    .with_toolshed(my_toolshed)
    .with_system_prompt("…")
    .build()?;
```

---

## 6. Low-level: single-provider router by hand

If you want none of the mixing machinery, build a single-provider router
directly (this is the plain `ProviderRouter` path from Doc 03):

```rust
use foundation_ai::harness::CloudPresets;
use foundation_ai::types::{ProviderRouter, RoutableProviderBox};

let provider = CloudPresets::claude_sonnet(&api_key)?;
let routable = RoutableProviderBox::new(provider);
let router = ProviderRouter::single(Box::new(routable));
```

---

## 7. Testing

The harness ships offline unit tests plus feature-gated pull tests:

- `tests/harness/router_tests.rs` + `session_bridge_tests.rs` — offline: build
  the presets, assert routing, provider identities, primary/memory/fallback
  wiring, and the agent-builder bridge. Cloud presets construct without any
  network call; GGUF presets construct a HF client + cache dir but download
  nothing until `get_model`, so routing is fully unit-testable.
- `tests/harness/integrations/gemma_pull.rs` — gated behind the
  `integration_tests` feature (no `#[ignore]`; when the feature is on, it
  runs). Pulls Gemma 4 E2B and generates, both via the `Gemma4E2b` preset and
  end-to-end through `gemma_router`.

Run the pull tests:

```bash
cargo test -p foundation_ai --features integration_tests \
  --test foundation_ai_tests -- --nocapture harness::integrations
```

---

## 8. Roadmap: MTP / speculative decoding (opt-in)

Multi-Token Prediction (speculative decoding) for the supporting local models
(GLM 5.2, Qwen 3.6, Gemma 4) is specified as an **opt-in, capability-gated**
addition — off by default, exposed via a `with_mtp(...)` on the supporting
presets. It never applies to models without an MTP head. See
`specifications/51-llama-mtp-speculative/requirements.md`.

---

## 9. ToolPreset — pre-built tool collections

`harness::ToolPreset` bundles tool implementations for quick registration,
mirroring the `register_*` functions in `agentic::tools`. Presets compose and
double as the source of `child_tools` for the F15 agent tool.

```rust
use foundation_ai::harness::ToolPreset;

// Single-tool presets:
ToolPreset::files(fs)       // → read, write, edit
ToolPreset::shell()         // → bash
ToolPreset::memory(h)       // → memory add/remove/replace (MultiCommands)
ToolPreset::shed(d)         // → tool discovery metatool
ToolPreset::agent(...)      // → agent start/check/result/… (F15, MultiCommands)
```

### Compose presets

```rust
let preset = ToolPreset::files(my_fs)
    .merge(ToolPreset::shell())
    .merge(ToolPreset::memory(my_hierarchy));

// Or with the + operator:
let preset = ToolPreset::files(my_fs) + ToolPreset::shell();
```

### Use presets

```rust
// 1. Register on an existing ToolCallManager:
preset.register_all(session.tool_manager());

// 2. Build a fresh ToolCallManager:
let mgr = preset.into_manager(session_id);

// 3. As child tools for the agent tool (F15):
let child_tools = preset.as_child_tools();   // Vec<Arc<dyn ToolImpl>>
let agent_tool = ToolPreset::agent::<Doc, Mem>(
    router, 0, 5, model, "/tmp/delegations", user, child_tools,
);
```

### Composite presets

| Preset | Contents |
|--------|----------|
| `minimal_sub_agent(fs)` | files + shell (safe for sub-agents — no delegation) |
| `standard(fs, hierarchy, discovery)` | files + shell + memory + shed |

### With agent presets

```rust
use foundation_ai::harness::{self, ToolPreset};

// Build the model preset as usual:
let builder = harness::claude_session::<Doc, Mem>(session_id, &api_key)?;

// Build a tool preset and register on the session:
let tools = ToolPreset::standard(fs, hierarchy, discovery);
// … then pass to the session builder's tool manager after build().

// Or for agent delegation:
let child_tools = ToolPreset::minimal_sub_agent(fs).as_child_tools();
let agent = ToolPreset::agent::<Doc, Mem>(
    router, 0, 5, model, "/tmp/delegations", user, child_tools,
);
```

---

## Reference: full public surface

```rust
// Re-exported from foundation_ai::harness
providers::{Glm52, Qwen36, Ornith10, Gemma4E4b, Gemma4_26b, Gemma4E2b}  // + ::MODEL_ID, ::q{3,4,5}_k_m, ::q8_0
CloudPresets  // ::claude_opus, ::claude_sonnet, ::openai_gpt4, ::openai_gpt4o, ::openai_responses
CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI
Q3_K_M, Q4_K_M, Q5_K_M, Q8_0
RouterMix, RouterPreset, ToolPreset

// Combo functions (each has _router and _session):
glm52_gemma_*, qwen36_gemma_*, gemma_*, claude_*, openai_chat_*, openai_responses_*
candle_llama_*   // feature = "candle"
```
