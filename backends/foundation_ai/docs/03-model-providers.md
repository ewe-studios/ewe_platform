# Fundamentals 03 — Model providers and the provider router

How models are abstracted, routed, and called across OpenAI, Anthropic,
Llama.cpp, Candle, and 12+ other providers.

---

## 1. Two-layer provider architecture

foundation_ai uses a two-layer design:

### Layer 1: `ModelProvider` (concrete, generic)

The trait that actual providers implement. It is **not object-safe** (has
associated types `Config` and `Model`), so you can't have `Arc<dyn ModelProvider>`:

```rust
pub trait ModelProvider {
    type Config: AuthProvider;
    type Model: Model;

    fn create(self, config: Option<Self::Config>) -> ModelProviderResult<Self>
        where Self: Sized;
    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor>;
    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model>;
    fn get_model_by_spec(&self, spec: ModelSpec) -> ModelProviderResult<Self::Model>;
}
```

Key methods:
- **`create()`** — initialize the provider with credentials/config (consumes self)
- **`describe()`** — return provider metadata (name, supported models, etc.)
- **`get_model()`** — create a model interaction type for a given model ID

### Layer 2: `RoutableProvider` (object-safe, for routing)

Because `ModelProvider` is not object-safe, the router uses an erased wrapper:

```rust
pub trait RoutableProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_id(&self) -> ModelProviders;
    fn describe(&self) -> Option<ModelProviderDescriptor>;
    fn serves(&self, model_id: &ModelId) -> bool;
    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec>;
    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec>;
    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel>;
}
```

`RoutableProviderBox<P>` wraps any `P: ModelProvider` into a `RoutableProvider`.

## 2. ModelId — how models are identified

```rust
pub enum ModelId {
    Name(String, Option<ProviderId>),   // "gpt-4", "claude-sonnet-4-6"
    Alias(String, Option<ProviderId>),  // "default", "fast"
    Group(String, Option<ProviderId>),  // "coding", "reasoning"
    Architecture(String, Option<ProviderId>), // "transformer", "mamba"
}
```

The router resolves `ModelId` → concrete provider + model name. Aliases and
groups let the user specify intent rather than exact model names.

## 3. ProviderRouter — model → provider resolution

```rust
let router = ProviderRouter::single(Box::new(openai_provider));

// Or with multiple providers:
let router = ProviderRouter::builder()
    .add_provider(Box::new(openai_provider))
    .add_provider(Box::new(anthropic_provider))
    .rule(RoutingRule {
        model: ModelId::Name("gpt-4".into(), None),
        provider_name: "openai".into(),
    })
    .build();

// Resolve a model:
let provider = router.resolve(&ModelId::Name("gpt-4".into(), None))?;
```

Resolution order:
1. **Explicit rule** — `RoutingRule` override wins
2. **Cached route** — previously resolved model→provider
3. **Declared support** — first provider where `serves(model_id)` is true
4. **Single-provider mode** — index 0
5. **Unresolved** → `RouterError::NoProviderForModel`

## 4. RoutingRule — explicit overrides

```rust
pub struct RoutingRule {
    pub model: ModelId,
    pub provider_name: String,  // match by provider.name()
}
```

Routes a specific model to a named provider, bypassing the normal resolution.

## 5. RoutableProviderBox — wrapping providers

```rust
use foundation_ai::types::{RoutableProviderBox, ModelProviders};

// If provider.describe() works (most built-in providers):
let routed = RoutableProviderBox::new(openai_provider);

// If provider has no descriptor, set explicitly:
let routed = RoutableProviderBox::with_identity(
    my_custom_provider,
    "my-provider",
    ModelProviders::Custom("my-provider".into()),
);
```

**Note:** `RoutableProviderBox::new()` **panics** if the provider cannot describe
itself. Every provider must have a name and identity.

## 6. Model — the interaction type

`ModelProvider::get_model()` returns a `Model` that handles actual generation:

```rust
pub trait Model: Send + Sync {
    fn name(&self) -> &str;
    fn supports(&self, model_id: &ModelId) -> bool;
    fn costing(&self) -> GenerationResult<UsageReport>;
    fn generate(&self, messages: Vec<Messages>) -> GenerationResult<Vec<Messages>>;
    fn generate_stream(&self, messages: Vec<Messages>) -> GenerationResult<ModelStreamBox>;
}
```

The `Model` trait is where `supports()`, `costing()`, `generate()`, and
`generate_stream()` live — not on `ModelProvider` itself.

## 7. Provider implementations

### OpenAI (`openai_provider.rs`)
- Chat completions API (`/v1/chat/completions`)
- Streaming via SSE
- Tool calling via function tools
- Retry with exponential backoff + rate limit detection

### OpenAI Responses (`openai_responses_provider.rs`)
- Responses API (`/v1/responses`) — newer, supports reasoning models
- Different streaming format (JSON events, not SSE)
- Native tool use (not function calling)

### Anthropic Messages (`anthropic_messages_provider.rs`)
- Messages API (`/v1/messages`)
- Streaming via SSE
- Tool use via tool blocks
- Thinking/extended reasoning support

### Llama.cpp (`llamacpp.rs`)
- Local inference via llama.cpp bindings
- Chat template support (Jinja templates)
- GGUF model loading
- GPU acceleration (CUDA, Metal, Vulkan)

### Candle (`candle.rs`)
- Local inference via Candle ML framework
- safetensors model loading
- GPU acceleration (CUDA, Metal)

### Cloud providers
- **Amazon Bedrock** — AWS hosted models
- **Google Vertex** — GCP hosted models
- **Google Gemini CLI** — CLI-based Gemini access
- **Google Antigravity** — internal Google model
- **Kimi Coding** — Moonshot's coding model
- **OpenAI Codex** — code-specific OpenAI model
- **OpenCode** — open coding model
- **OpenRouter** — multi-provider routing service
- **Vercel AI Gateway** — Vercel's model proxy
- **xAI** — Grok models

## 8. Error classification

Each provider maps HTTP errors to `AgenticError`:

| HTTP | Classification |
|---|---|
| 429 | `GenKind::RateLimit` → retry with backoff |
| 500-503 | `GenKind::Network` → retry with backoff |
| 400 | `GenKind::Provider` → fail (bad request) |
| 401 | `GenKind::Auth` → fail (bad key) |
| 524 | `GenKind::Timeout` → retry |

## 9. Streaming architecture

Providers that support streaming return a `ModelStreamBox`:

```rust
type ModelStreamBox = Box<
    dyn StreamIterator<D = Messages, P = ModelState, Item = Stream<Messages, ModelState>> + Send
>;
```

The stream yields:
- `Stream::Init` — stream started
- `Stream::Pending(ModelState::GeneratingTokens(usage))` — token generated
- `Stream::Next(Messages::Assistant { content, ... })` — final message
- `Stream::Next(Messages::Assistant { content: ToolCall, ... })` — tool call
- `Stream::Pending(ModelState::Finished)` — stream complete
- `Stream::Pending(ModelState::Error(msg))` — error

The `AgentStream` lifts this to `Stream<SessionRecord, AgentProgress>` for the
agent loop (Doc 01).
