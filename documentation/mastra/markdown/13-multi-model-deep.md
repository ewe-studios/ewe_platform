# Mastra -- Multi-Model Execution Deep Dive

## Overview

Mastra's multi-model architecture spans three layers: **Model Router** (200+ provider resolution), **Fallback Chains** (automatic model failover), and **Observability** (usage tracking and tracing). Unlike Hermes's credential pool approach or Pi's model switching, Mastra treats model management as a gateway plugin problem -- providers register once, routes resolve dynamically, and failures cascade through configured fallbacks.

**Key insight:** Mastra doesn't manage credentials per-model. Instead, API keys are configured at the provider level and passed through the gateway. The ModelRouterLanguageModel class resolves `provider/model` strings to concrete SDK clients, then delegates to the appropriate gateway implementation.

## Model Architecture

```mermaid
flowchart TD
    USER[Agent.generate/stream] --> ROUTER[ModelRouterLanguageModel<br/>parse "openai/gpt-4o"]

    ROUTER --> REGISTRY[PROVIDER_REGISTRY (static JSON)<br/>200+ providers]
    REGISTRY --> OFFLINE{MASTRA_OFFLINE<br/>env var?}
    OFFLINE -->|Yes| LOCAL[Local model provider<br/>ollama, llama.cpp]
    OFFLINE -->|No| GATEWAY[Gateway Resolution<br/>OpenAI, Anthropic, Google]

    GATEWAY --> CONFIG[Provider Config<br/>apiKey, baseUrl, options]
    CONFIG --> SDK[SDK Client<br/>OpenAI, Anthropic, etc.]

    SDK --> GENERATE{Success?}
    GENERATE -->|No| FALLBACK[Fallback Chain<br/>next model in list]
    GENERATE -->|Yes| RESULT[Return Response]

    FALLBACK --> SDK
    RESULT --> OBS[Observability<br/>usage tracking + tracing]
```

## Model Router: Provider Resolution

The `ModelRouterLanguageModel` class is the concrete implementation that wraps a gateway client:

```typescript
// packages/core/src/llm/model/router.ts (simplified, line 84+)
import { parseModelRouterId } from './gateway-resolver.js';
import { PROVIDER_REGISTRY } from './provider-registry.ts';

export class ModelRouterLanguageModel implements MastraLanguageModelV2 {
  readonly specificationVersion = 'v2';  // or 'v3' for AI SDK v6
  readonly modelId: string;       // e.g. 'gpt-5'
  readonly provider: string;      // e.g. 'openai'
  readonly gatewayId: string;     // e.g. 'mastra'

  constructor(
    config: ModelRouterModelId | OpenAICompatibleConfig,
    customGateways?: MastraModelGateway[]
  ) {
    // Normalize config: string "openai/gpt-5" → { id: "openai/gpt-5" }
    const normalizedConfig = normalizeModelConfig(config);

    // Collect all available gateways (built-in + custom)
    const allGateways = getEnabledGateways(customGateways);

    // Find the gateway that handles this model
    this.gateway = findGatewayForModel(normalizedConfig.id, allGateways);

    // Parse provider and model from the ID
    const gatewayPrefix = this.gateway.id;
    const parsed = parseModelRouterId(normalizedConfig.id, gatewayPrefix);
    this.modelId = parsed.model;
    this.provider = parsed.provider;

    // Create the underlying SDK LanguageModel via the gateway
    this.model = this.gateway.getModel(normalizedConfig);
  }

  async doGenerate(options: LanguageModelV2CallOptions) {
    return this.model.doGenerate(options);
  }

  async doStream(options: LanguageModelV2CallOptions): Promise<[ModelStream, () => void]> {
    return this.model.doStream(options);
  }
}
```

Key differences from the simplified model-router doc:
- **No `parseModelId()` method** — uses `parseModelRouterId()` from `gateway-resolver.js`
- **No `ProviderRegistry` class** — imports `PROVIDER_REGISTRY` from static JSON
- **No `getModelConfig()` method** — the gateway handles config resolution
- **Constructor accepts** either a `ModelRouterModelId` object or an `OpenAICompatibleConfig`
- **`doStream` returns** a tuple `[ModelStream, () => void]` (stream + cleanup function)

### Gateway Plugin Architecture

Each provider has a gateway implementation:

```typescript
// Provider gateway interface
interface ModelGateway {
  generate(messages, options): Promise<LanguageModelResponse>;
  stream(messages, options): AsyncGenerator<LanguageModelChunk>;
}

// OpenAI gateway
class OpenAIGateway implements ModelGateway {
  #client: OpenAI;

  async generate(messages, options) {
    const response = await this.client.chat.completions.create({
      model: options.model,
      messages,
      max_tokens: options.maxTokens,
      ...options.providerOptions,
    });
    return normalizeOpenAIResponse(response);
  }
}

// Anthropic gateway
class AnthropicGateway implements ModelGateway {
  #client: Anthropic;

  async generate(messages, options) {
    const response = await this.client.messages.create({
      model: options.model,
      messages: convertMessagesForAnthropic(messages),
      max_tokens: options.maxTokens ?? 4096,
      ...options.providerOptions,
    });
    return normalizeAnthropicResponse(response);
  }
}
```

## Fallback Chains

Mastra supports model fallbacks at the Agent level via the `ModelFallbacks` type:

```typescript
// packages/core/src/agent/agent.ts (lines 121-129)
type ModelFallbacks = {
  id: string;                                    // Unique identifier
  model: DynamicArgument<MastraModelConfig>;     // Model config or resolver function
  maxRetries: number;                            // Retries per fallback entry
  enabled: boolean;                              // Toggle on/off
  modelSettings?: DynamicArgument<ModelFallbackSettings>;
  providerOptions?: DynamicArgument<ProviderOptions>;
  headers?: DynamicArgument<Record<string, string>>;
}[];
```

The Agent's `model` field can be set to a `ModelFallbacks` array:
```typescript
model: DynamicArgument<MastraModelConfig | ModelWithRetries[], TRequestContext> | ModelFallbacks
```

Helpers for working with fallbacks:
- `isModelFallbacks()` — type guard to check if a value is a fallback array
- `normalizeModelFallbacks()` — normalize fallback entries
- `toFallbackEntry()` — static method to convert a model config to a fallback entry

When the primary model fails, the Agent iterates through the fallback chain, creating a new `ModelRouterLanguageModel` for each fallback entry and retrying. The `maxRetries` field controls retries per individual fallback model, and `enabled` toggles whether a specific fallback is active.

### Spec Version Handling

The Agent checks the model's `specificationVersion` after resolving the LLM in `generate()` (line 5340):

```typescript
// agent/agent.ts, generate() at line 5340
async generate(messages, options) {
  const llm = await this.getLLM({ requestContext, model: options.model });
  // specVersion is checked AFTER getLLM()
  // v1 models throw: AGENT_GENERATE_V1_MODEL_NOT_SUPPORTED
  // v2/v3 proceed to #execute()
}
```

For v1 legacy models, `generate()` throws `AGENT_GENERATE_V1_MODEL_NOT_SUPPORTED` and users should use `generateLegacy()` instead.

### Error Handling in Fallback Chains

When a model in the fallback chain fails, the Agent's `#execute()` pipeline handles the error through the error processor workflow. Non-retryable errors (auth failures, invalid requests) propagate immediately; retryable errors (rate limits, server errors) trigger the next fallback model.

## LLM Recording for Multi-Model Testing

Mastra's LLM recorder supports testing with multiple providers:

```typescript
// packages/_llm-recorder/src/auto-recording.ts
export const LLM_API_HOSTS = [
  'https://api.openai.com',
  'https://api.anthropic.com',
  'https://generativelanguage.googleapis.com',
  'https://openrouter.ai',
];

// Recording captures which provider was used
const model = body && typeof body === 'object' && 'model' in body
  ? (body as Record<string, unknown>).model
  : undefined;

console.log(`[llm-recorder] Recording: ${url} (model: ${model})`);
```

The recorder stores provider-specific recordings with model metadata, enabling replay testing across different providers.

## Observability: Model Tracing

Every model call generates a trace with hierarchical spans:

```typescript
// observability/mastra/src/model-tracing.ts
// Hierarchy: MODEL_GENERATION -> MODEL_STEP -> MODEL_CHUNK

class ModelSpanTracker {
  #modelSpan?: Span<SpanType.MODEL_GENERATION>;
  #currentStepSpan?: Span<SpanType.MODEL_STEP>;
  #currentChunkSpan?: Span<SpanType.MODEL_CHUNK>;

  startStep(payload?: StepStartPayload) {
    this.#currentStepSpan = this.#modelSpan?.createChildSpan({
      name: `step: ${this.#stepIndex}`,
      type: SpanType.MODEL_STEP,
      input: extractStepInput(payload?.request),  // Summarized, not full request
    });
  }

  #endStepSpan<OUTPUT>(payload: StepFinishPayload<any, OUTPUT>) {
    const usage = extractUsageMetrics(rawUsage, metadata?.providerMetadata);

    this.#currentStepSpan.end({
      output: otherOutput,
      attributes: {
        usage,
        isContinued: stepResult.isContinued,
        finishReason: stepResult.reason,
      },
    });
  }
}
```

### Usage Metrics Extraction

The observability layer extracts standardized usage metrics from provider-specific responses:

```typescript
// observability/mastra/src/usage.ts (simplified)
function extractUsageMetrics(usage: unknown, providerMetadata?: unknown): UsageStats {
  // Normalize provider-specific usage to common format:
  // - promptTokens, completionTokens, totalTokens
  // - cacheReadTokens, cacheWriteTokens (provider-specific)
  // - timeToFirstToken (from completionStartTime)
}
```

## Comparison: Multi-Model Across Projects

| Aspect | Hermes (Python) | Pi (TypeScript) | Mastra (TypeScript) |
|--------|----------------|-----------------|---------------------|
| **Provider Resolution** | Model ID parsing + gateway | Provider adapter registry | `parseModelRouterId()` + gateway plugins |
| **Credential Management** | CredentialPool with threading.Lock | Environment variables / config | Provider-level config in static JSON, env var fallback |
| **Error Classification** | Error classifier in retry_utils.py | API error type checking | Error processor workflow in `#execute()` |
| **Fallback Models** | Async fallback model attempt | Model switching via config | `ModelFallbacks[]` with per-entry `maxRetries`, `enabled` |
| **Auxiliary Models** | AsyncOpenAI client per event loop | Single shared client | Per-provider gateway client |
| **Usage Tracking** | Token normalization in cost tracking | Provider metadata extraction | `extractUsageMetrics()` with cache tokens |
| **Rate Limiting** | NousRateGuard (proactive throttling) | Basic retry with backoff | Retry via error processor + `maxRetries` per fallback |
| **Recording/Replay** | Not implemented | Not implemented | LLM recorder with MSW interception |

### Hermes's Credential Pool

Hermes maintains a `CredentialPool` with `threading.Lock` for thread-safe credential selection. When multiple tools call the same provider concurrently, the pool ensures credentials are distributed without conflicts. Google OAuth uses `threading.Event` for refresh deduplication.

### Pi's Provider Adapters

Pi uses an adapter pattern where each provider (OpenAI, Anthropic, etc.) implements a common interface. Model switching happens by swapping the active adapter. Credentials come from environment variables or configuration.

### Mastra's Gateway Plugins

Mastra treats providers as gateway plugins. `PROVIDER_REGISTRY` (static JSON) holds provider configurations, and `ModelRouterLanguageModel` resolves `provider/model` strings via `parseModelRouterId()` + `findGatewayForModel()`. API keys fall back to environment variables if not explicitly configured.

## Offline Mode and Local Models

Mastra supports local model providers:

```typescript
// Provider registry with offline mode
if (config.offlineMode) {
  // Register local providers: ollama, llama.cpp
  registerLocalProviders();
}

// Local provider doesn't need API key
const config = {
  provider: 'ollama',
  model: 'llama3',
  baseURL: 'http://localhost:11434/v1',  // Ollama's OpenAI-compatible API
};
```

## Background Task Multi-Model

Background tasks can use different models than the main agent:

```typescript
// BackgroundTaskManager can spawn tasks with any model
const task = backgroundTaskManager.createTask({
  name: 'summarize-conversation',
  model: 'openai/gpt-4o-mini',  // Cheaper model for background work
  fn: async () => {
    const summary = await summarize(messages);
    return summary;
  },
});
```

## Key Optimizations

### 1. Provider Config Caching

Provider configurations from the static JSON registry are resolved once and cached. The gateway resolution doesn't re-parse API keys or base URLs on each call.

### 2. Gateway Lazy Initialization

Gateway clients are created lazily -- the OpenAI client isn't instantiated until the first `generate()` call. This avoids connection overhead for unused providers.

### 3. Error Classification Prevents Wasted Retries

By distinguishing retryable (429, timeout, 5xx) from non-retryable (401, 400) errors, Mastra avoids burning through fallback models on errors that won't resolve.

### 4. Usage Normalization

`extractUsageMetrics()` converts provider-specific usage formats (OpenAI's `usage`, Anthropic's `usage`, Google's `metadata`) into a common `UsageStats` object, enabling cross-provider cost tracking.

## Related Documents

- [05-model-router.md](./05-model-router.md) -- ModelRouterLanguageModel and gateway architecture
- [08-multi-model.md](./08-multi-model.md) -- Model fallback chains and background tasks
- [09-data-flow.md](./09-data-flow.md) -- End-to-end model call flow
- [10-comparison.md](./10-comparison.md) -- Pi vs Hermes vs Mastra comparison

## Source Paths

```
packages/core/src/
├── llm/model/
│   ├── router.ts                 ← ModelRouterLanguageModel, provider resolution
│   └── provider-registry.ts      ← Provider registry loader, offline mode
├── agent/agent.ts                ← Agent generate/stream with fallback chain
└── background-tasks/manager.ts    ← Background task execution with custom models

observability/
└── mastra/src/
    ├── model-tracing.ts          ← Hierarchical span tracking per model call
    └── usage.ts                  ← Usage metrics extraction from provider responses

packages/_llm-recorder/src/
└── auto-recording.ts             ← MSW-based recording/replay for multi-provider testing
```
