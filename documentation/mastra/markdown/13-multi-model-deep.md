# Mastra -- Multi-Model Execution Deep Dive

## Overview

Mastra's multi-model architecture spans three layers: **Model Router** (200+ provider resolution), **Fallback Chains** (automatic model failover), and **Observability** (usage tracking and tracing). Unlike Hermes's credential pool approach or Pi's model switching, Mastra treats model management as a gateway plugin problem -- providers register once, routes resolve dynamically, and failures cascade through configured fallbacks.

**Key insight:** Mastra doesn't manage credentials per-model. Instead, API keys are configured at the provider level and passed through the gateway. The ModelRouterLanguageModel class resolves `provider/model` strings to concrete SDK clients, then delegates to the appropriate gateway implementation.

## Model Architecture

```mermaid
flowchart TD
    USER[Agent.generate/stream] --> ROUTER[ModelRouterLanguageModel<br/>parse "openai/gpt-4o"]

    ROUTER --> REGISTRY[Provider Registry<br/>200+ providers]
    REGISTRY --> OFFLINE{Offline mode?}
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

The `ModelRouterLanguageModel` class parses model IDs and routes to the correct provider:

```typescript
// packages/core/src/llm/model/router.ts (simplified)
class ModelRouterLanguageModel {
  #providerRegistry: ProviderRegistry;
  #modelId: string;  // e.g., "openai/gpt-4o" or "anthropic/claude-3.5-sonnet"

  async parseModelId(modelId: string): Promise<{ provider: string; model: string }> {
    const [provider, ...modelParts] = modelId.split('/');
    const model = modelParts.join('/');  // Handle models with "/" in name

    if (!this.#providerRegistry.has(provider)) {
      throw new Error(`Unknown provider: ${provider}`);
    }

    return { provider, model };
  }

  async getModelConfig(provider: string, model: string) {
    const config = this.#providerRegistry.getConfig(provider);
    return {
      ...config,
      model,
      apiKey: config.apiKey ?? process.env[`${provider.toUpperCase()}_API_KEY`],
      baseURL: config.baseURL ?? getDefaultBaseURL(provider),
    };
  }
}
```

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

Mastra supports model fallbacks at the Agent level:

```typescript
// packages/core/src/agent/agent.ts (simplified)
class Agent {
  #model: LanguageModel;
  #fallbackModels?: LanguageModel[];

  async generate(input, options) {
    const models = [this.#model, ...(this.#fallbackModels ?? [])];

    let lastError: Error | undefined;
    for (const model of models) {
      try {
        return await model.generate(input, options);
      } catch (error) {
        lastError = error;
        if (!this.isRetryableError(error)) {
          throw error;  // Auth error, bad request -- don't retry
        }
        // Continue to next model in fallback chain
      }
    }

    throw lastError;  // All models exhausted
  }

  isRetryableError(error: Error): boolean {
    // Retry on: rate limits, timeouts, 5xx
    // Don't retry on: auth failures, invalid requests
    return error instanceof RateLimitError
      || error instanceof TimeoutError
      || (error instanceof APIError && error.status >= 500);
  }
}
```

### Error Classification

The error classification determines whether to try the next model:

| Error Type | Retryable? | Why |
|-----------|-----------|-----|
| RateLimitError (429) | Yes | Temporary, next model may have capacity |
| TimeoutError | Yes | Network issue, next model may respond |
| APIError 5xx | Yes | Server-side, transient |
| APIError 4xx | No | Client-side, will fail on all models |
| AuthenticationError | No | Missing/invalid key affects all models |
| InvalidRequestError | No | Bad parameters, won work on any model |

## LLM Recording for Multi-Model Testing

Mastra's LLM recorder supports testing with multiple providers:

```typescript
// packages/_llm-recorder/src/llm-recorder.ts
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
| **Provider Resolution** | Model ID parsing + gateway | Provider adapter registry | ProviderRegistry + gateway plugins |
| **Credential Management** | CredentialPool with threading.Lock | Environment variables / config | Provider-level config, env var fallback |
| **Error Classification** | Error classifier in retry_utils.py | API error type checking | isRetryableError() on Agent |
| **Fallback Models** | Async fallback model attempt | Model switching via config | Fallback chain at Agent level |
| **Auxiliary Models** | AsyncOpenAI client per event loop | Single shared client | Per-provider gateway client |
| **Usage Tracking** | Token normalization in cost tracking | Provider metadata extraction | extractUsageMetrics() with cache tokens |
| **Rate Limiting** | NousRateGuard (proactive throttling) | Basic retry with backoff | Retry with isRetryableError classification |
| **Recording/Replay** | Not implemented | Not implemented | LLM recorder with MSW interception |

### Hermes's Credential Pool

Hermes maintains a `CredentialPool` with `threading.Lock` for thread-safe credential selection. When multiple tools call the same provider concurrently, the pool ensures credentials are distributed without conflicts. Google OAuth uses `threading.Event` for refresh deduplication.

### Pi's Provider Adapters

Pi uses an adapter pattern where each provider (OpenAI, Anthropic, etc.) implements a common interface. Model switching happens by swapping the active adapter. Credentials come from environment variables or configuration.

### Mastra's Gateway Plugins

Mastra treats providers as gateway plugins. The `ProviderRegistry` loads provider configurations, and the `ModelRouterLanguageModel` resolves `provider/model` strings to concrete implementations. API keys fall back to environment variables if not explicitly configured.

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

Provider configurations are resolved once and cached. The `ProviderRegistry` doesn't re-parse API keys or base URLs on each call.

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
└── llm-recorder.ts               ← MSW-based recording/replay for multi-provider testing
```
