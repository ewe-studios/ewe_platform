# Fundamentals 03 — Model providers and the provider router

How backends are abstracted, routed and called.

Sources: `src/types/base_types.rs` (traits), `src/types/routable_provider.rs`
(router), `src/backends/` (implementations), `src/harness/providers.rs`
(presets).

---

## 1. Three layers

```
ModelProvider  (concrete, generic: associated Config + Model types)
     │  wrapped by RoutableProviderBox / PreloadedProvider
     ▼
RoutableProvider  (object-safe; hands out Box<dyn Model>)
     │  registered in
     ▼
ProviderRouter  (ModelId → provider → model)
```

### `ModelProvider`

```rust
pub trait ModelProvider {
    type Config: AuthProvider;
    type Model: Model;

    fn create(self, config: Option<Self::Config>) -> ModelProviderResult<Self> where Self: Sized;
    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor>;
    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model>;
    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model>;
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;
    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>>;
}
```

`create()` consumes the provider and applies config and credentials. It is not
object-safe (associated types), hence the next layer.

### `Model`

```rust
pub trait Model {
    fn spec(&self) -> ModelSpec;
    fn tool_formatter(&self) -> Box<dyn ToolFormatter>;
    fn descriptor(&self) -> Option<ModelProviderDescriptor>;   // pricing, context window
    fn costing(&self) -> GenerationResult<UsageReport>;
    fn generate(&self, interaction: ModelInteraction, specs: Option<ModelParams>)
        -> GenerationResult<Vec<Messages>>;
    fn stream(&self, interaction: ModelInteraction, specs: Option<ModelParams>)
        -> GenerationResult<ModelStreamBox>;
}
```

`BoxModel = Box<dyn Model>` is what the router hands out. The agent loop only
calls `stream`.

### `RoutableProvider`

```rust
pub trait RoutableProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_id(&self) -> ModelProviders;
    fn describe(&self) -> Option<ModelProviderDescriptor>;
    fn serves(&self, model_id: &ModelId) -> bool;          // get_one(id).is_ok()
    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec>;
    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec>;
    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel>;
}
```

Two adapters implement it:

- **`RoutableProviderBox::new(provider)`** — wraps any `ModelProvider` whose
  model is `Send + Sync`. It takes its name and id from `describe()` and
  **panics** if `describe()` fails; use
  `RoutableProviderBox::with_identity(provider, "name", ModelProviders::…)`
  for providers that can't describe themselves.
- **`PreloadedProvider::new(model, model_id)`** — serves one already-loaded
  model (a Candle or llama.cpp model loaded from a local path). Its
  `into_router()` gives you a single-provider router directly.

## 2. `ModelId`

```rust
pub enum ModelId {
    Name(String, Option<Quantization>),          // "claude-sonnet-4-6", "gpt-4o"
    Alias(String, Option<Quantization>),
    Group(String, Option<Quantization>),
    Architecture(String, Option<Quantization>),
}
```

The second field is a GGUF quantization for local models. The router only
looks at `ModelId::name()` — the variant and quantization do not affect
routing; they matter to the provider that receives the id.

## 3. `ProviderRouter`

```rust
// One provider serves everything:
let router = ProviderRouter::single(Box::new(RoutableProviderBox::new(provider)));

// Several providers:
let router = ProviderRouter::builder()
    .add_provider(Box::new(RoutableProviderBox::new(openai)))
    .add_provider(Box::new(RoutableProviderBox::new(anthropic)))
    .rule(RoutingRule {
        model: ModelId::Name("gpt-4o".into(), None),
        provider_name: "OpenAI".into(),     // must equal that provider's name()
    })
    .build();

let model: BoxModel = router.get_model(&ModelId::Name("gpt-4o".into(), None))?;
```

Resolution order (`resolve` / `get_model`):

1. **Explicit rule** whose `model.name()` matches → that provider, or
   `RouterError::RuleMismatch` if no provider has that name.
2. **Cached route** from an earlier resolution of the same name.
3. **Declared support** — the first provider whose `serves(id)` is true; the
   result is cached.
4. **Only one provider registered** → it.
5. Otherwise `RouterError::NoProviderForModel`.

Other methods: `find_provider`, `list_all`, `memory_model(Option<&ModelId>)`
(falls back to the first provider), `is_single`, `provider_count`. The router
is an `Arc` inside, so `clone()` is cheap.

For one-call router setups (Claude Opus + Sonnet, GLM 5.2 + Gemma, …) use the
`harness` module — Doc 12.

## 4. Backends (`src/backends/`)

| Backend | Type | Endpoint / runtime | Notes |
|---|---|---|---|
| OpenAI Chat Completions | `OpenAIProvider` + `OpenAIConfig` | `/v1/chat/completions` | Any OpenAI-compatible server via `with_base_url` (OpenRouter, Ollama, vLLM, LM Studio, llama-server). Embeddings via `/v1/embeddings`. |
| OpenAI Responses | `ResponsesProvider` + `ResponsesConfig` | `/v1/responses` | Reasoning models (o-series). |
| Anthropic Messages | `AnthropicMessagesProvider` + `AnthropicConfig` | `/v1/messages` | Thinking blocks, image tool results. |
| llama.cpp | `LlamaBackends` / `LlamaModels`, `HuggingFaceGGUFProvider` | in-process GGUF | Renders the model's Jinja chat template; GPU via `cuda`/`metal`/`vulkan`; MTP / speculative decoding opt-in (`LlamaBackendConfig::builder().mtp(..)`). Native + emscripten only. |
| Candle | `CandleModels`, `HuggingFaceCandleProvider` | in-process safetensors | Llama and Gemma2 architectures today; CUDA via `candle-cuda`, Metal automatic on Apple. Native only. |

All three HTTP backends stream with Server-Sent Events through
`foundation_netio`, and retry 429 / 5xx internally (`with_max_retries`). The
config builders share `with_base_url`, `with_timeout_secs`,
`with_max_retries`, `with_proxy_url`, `with_streaming` and `with_auth`.

### Model catalogues (`src/models/providers/`)

The files for Amazon Bedrock, Google Vertex, Gemini CLI, Antigravity, Kimi,
OpenAI Codex, OpenCode, OpenRouter, Vercel AI Gateway, xAI, Anthropic and
OpenAI are **generated descriptor tables** — model id, name, API kind,
pricing, context window, max tokens. They are not provider implementations.
`models::model_descriptors()` returns them all; providers use them for
`describe()` and cost reporting. Regenerate with
`cargo run --bin ewe_platform gen_model_descriptors` — do not edit them by
hand.

A model reachable through an OpenAI- or Anthropic-compatible API (OpenRouter,
Vercel AI Gateway, …) is used through the matching HTTP backend with a
different `base_url`.

## 5. Tool formatting per backend

Each backend renders `Tool`s into its own format, starting from
`Tool::function_spec()`. `Model::tool_formatter()` returns the
`ToolFormatter` it uses:

| Backend | `tool_formatter()` | Rendering |
|---|---|---|
| Chat Completions | `OpenAIFormatter` | `function` tools; `strict: true` when the tool has a `returns` schema |
| Responses | `TextBasedFormatter` | The request builder renders native `ResponseTool`s with `strict` (see `docs/fixes/002`) |
| Anthropic | `AnthropicFormatter` | `name` / `description` / `input_schema` (no output schema) |
| llama.cpp, Candle | `TextBasedFormatter` | Tool list and calling instructions in the prompt; parses XML-style calls back out |

## 6. Errors

Backends return `GenerationError` (`Failed`, `Llama`, `Candle`, `Tokenizer`,
`Backend`, `Generic`). There are no HTTP-status variants. At the agent
boundary `AgenticError::from_generation` turns it into a `GenerationFailure`
with a `GenKind`:

| `GenKind` | Detected by |
|---|---|
| `ContextOverflow` | `Messages::is_context_overflow` on the last message (provider error text, or usage above the context window) |
| `RateLimit` | `"rate limit"`, `"429"` or `"too many requests"` in the message |
| `Provider` / `Network` / `Other` | the rest |

Doc 01 §6 shows what the loop does with each.

## 7. Streaming

```rust
pub type ModelStreamBox = Box<
    dyn StreamIterator<D = Messages, P = ModelState, Item = Stream<Messages, ModelState>> + Send,
>;
```

A stream yields `Stream::Pending(ModelState::GeneratingTokens(usage))` while
working and one `Stream::Next(Messages::Assistant { .. })` per chunk — text,
thinking, or a tool call. `ModelState::Error(msg)` reports a failure in band.
The agent loop lifts this to `Stream<SessionRecord, AgentProgress>`
(`progress::lift_model_item`).
