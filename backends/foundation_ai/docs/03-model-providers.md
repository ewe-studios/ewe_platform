# Fundamentals 03 — Model providers and the provider router

How models are abstracted, routed, and called across OpenAI, Anthropic,
Llama.cpp, Candle, and 12+ other providers.

---

## 1. ModelProvider trait

Every model backend implements this:

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &str;
    fn supports(&self, model: &ModelId) -> bool;
    fn costing(&self, model: &ModelId) -> Option<ModelUsageCosting>;

    async fn generate(
        &self, interaction: ModelInteraction,
    ) -> GenerationResult<Vec<Messages>>;

    async fn generate_stream(
        &self, interaction: ModelInteraction,
    ) -> GenerationResult<ModelStreamBox>;
}
```

**`ModelInteraction`** — the full generation request:
- `system_prompt` — optional system instructions
- `soul` — optional personality/role definition
- `tools_shed` — available tools for the model
- `messages` — conversation history
- `chat_template` — optional template override (for local models)
- `tool_choice` — force/none/auto tool selection

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

## 3. RoutableProvider — the routing interface

```rust
pub trait RoutableProvider: ModelProvider {
    fn serves(&self, model: &ModelId) -> bool;
    fn fallbacks(&self) -> Vec<ModelId>;
}
```

Each provider declares which models it serves and what to try if it fails.
The router uses this to build a fallback chain.

## 4. ProviderRouter — model → provider resolution

```rust
let router = ProviderRouter::new()
    .register(openai_provider)
    .register(anthropic_provider)
    .register(llamacpp_provider);

let provider = router.resolve(&ModelId::Name("claude-sonnet-4-6".into(), None))?;
```

Resolution order:
1. Exact match (model name + provider)
2. Name-only match (any provider with this model)
3. Alias resolution (alias → concrete model)
4. Group resolution (group → best available model)
5. Fallback chain (if primary fails)

## 5. Provider implementations

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

### HuggingFace providers
- **GGUF** — downloads GGUF models from HF Hub, runs via llama.cpp
- **Candle** — downloads safetensors from HF Hub, runs via Candle

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

## 6. Error classification

Each provider maps HTTP errors to `AgenticError`:

| HTTP | Classification |
|---|---|
| 429 | `GenKind::RateLimit` → retry with backoff |
| 500-503 | `GenKind::Network` → retry with backoff |
| 400 | `GenKind::Provider` → fail (bad request) |
| 401 | `GenKind::Auth` → fail (bad key) |
| 524 | `GenKind::Timeout` → retry |

## 7. Streaming architecture

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
