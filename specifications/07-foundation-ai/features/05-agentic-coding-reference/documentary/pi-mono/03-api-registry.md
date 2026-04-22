# pi-mono API Registry

## File Locations

- Registry: `packages/ai/src/api-registry.ts`
- Provider registration: `packages/ai/src/providers/register-builtins.ts`
- Provider implementations: `packages/ai/src/providers/*.ts`
- Env-to-provider mapping: `packages/ai/src/env-api-keys.ts`

## Registry Pattern

Providers register via `registerApiProvider()` with three fields:
- `api` -- The API type name (e.g., `"anthropic-messages"`, `"openai-completions"`)
- `stream` -- Detailed streaming function with provider-specific options
- `streamSimple` -- Simplified function with unified options (e.g., `reasoning` for thinking level)

### Registration Flow

```typescript
registerApiProvider({
  api: "anthropic-messages",
  stream: streamAnthropic,
  streamSimple: streamAnthropicSimple,
});
```

Providers are lazily loaded -- they are dynamically imported on first use via `register-builtins.ts`. This means:
- No provider code is loaded until its API type is actually needed
- Each provider imports its own SDK dependencies (which may be large)
- The registry only holds the `stream` and `streamSimple` function references

### Lazy Loading Map

`register-builtins.ts` contains a map of API types to their module paths:

| API Type | Module | SDK Dependency |
|----------|--------|----------------|
| `anthropic-messages` | `providers/anthropic.ts` | `@anthropic-ai/sdk` |
| `openai-completions` | `providers/openai-completions.ts` | `openai` |
| `openai-responses` | `providers/openai-responses.ts` | `openai` |
| `openai-codex-responses` | `providers/openai-codex-responses.ts` | `openai` |
| `azure-openai-responses` | `providers/azure-openai-responses.ts` | `openai` |
| `bedrock-converse-stream` | `providers/amazon-bedrock.ts` | AWS SDK (Node.js only) |
| `mistral-conversations` | `providers/mistral.ts` | Mistral SDK |
| `google-generative-ai` | `providers/google.ts` | `@google/generative-ai` |
| `google-gemini-cli` | `providers/google-gemini-cli.ts` | Google CLI SDK |
| `google-vertex` | `providers/google-vertex.ts` | Google Vertex SDK |

## Dual Interface: stream vs streamSimple

### stream() -- Detailed Interface

Exposes provider-specific options. Example for Anthropic:

```typescript
stream(model, messages, {
  maxTokens,
  tools,
  toolChoice,
  thinking,       // Anthropic-specific: { type: "enabled", budget_tokens }
  cacheControl,   // Anthropic-specific: { type: "ephemeral" }
  metadata,       // Anthropic-specific: user_id for rate limit buckets
  // ... other Anthropic-specific options
});
```

### streamSimple() -- Unified Interface

Maps unified options to provider-specific options:

```typescript
streamSimple(model, messages, {
  maxTokens,
  tools,
  toolChoice,
  reasoning: "high",  // Unified -- maps to thinking for Anthropic,
                      // reasoning_effort for OpenAI, etc.
});
```

The `reasoning` option maps differently per provider:
- Anthropic: `thinking: { type: "enabled", budget_tokens: <level> }`
- OpenAI: `reasoning_effort: "<level>"`
- Google: No direct mapping (uses thought markers in system prompt)

## Known API Types

```typescript
type Api =
  | "openai-completions"       // Chat Completions API (many providers)
  | "openai-responses"         // OpenAI Responses API
  | "azure-openai-responses"   // Azure variant
  | "openai-codex-responses"   // Codex variant
  | "anthropic-messages"       // Anthropic Messages API
  | "bedrock-converse-stream"  // AWS Bedrock Converse
  | "mistral-conversations"    // Mistral API
  | "google-generative-ai"     // Google Gemini API
  | "google-gemini-cli"        // Google Gemini CLI API
  | "google-vertex"            // Google Vertex AI
```

## Known Providers

```typescript
type Provider =
  | "anthropic" | "openai" | "google" | "google-gemini-cli"
  | "google-antigravity" | "google-vertex" | "amazon-bedrock"
  | "azure-openai-responses" | "openai-codex" | "github-copilot"
  | "xai" | "groq" | "cerebras" | "openrouter" | "vercel-ai-gateway"
  | "zai" | "mistral" | "minimax" | "minimax-cn" | "huggingface"
  | "fireworks" | "opencode" | "opencode-go" | "kimi-coding"
```

## API Key Resolution

Each provider maps to specific environment variables via `env-api-keys.ts`:

| Provider | Env Var |
|----------|---------|
| `openai` | `OPENAI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `google` | `GOOGLE_API_KEY` |
| `xai` | `XAI_API_KEY` |
| `groq` | `GROQ_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `github-copilot` | OAuth token (not env var) |
| `amazon-bedrock` | AWS credentials / ADC |
| `google-vertex` | ADC (Application Default Credentials) |

OAuth providers (github-copilot, anthropic OAuth) use different token patterns -- they don't use simple API keys but rather OAuth access tokens with specific header formats.

## Provider-Specific Client Construction

Each provider builds its own SDK client in its `stream()` function:

### Anthropic
```typescript
const client = new Anthropic({ apiKey, baseURL: model.baseUrl });
// Supports "stealth mode" that mimics Claude Code tool naming for OAuth tokens
```

### OpenAI Completions
```typescript
const client = new OpenAI({ apiKey, baseURL: model.baseUrl });
// Extensive compat detection via detectCompat() for non-standard providers
```

### Bedrock (Node.js only)
```typescript
// Uses AWS SDK Converse Stream API
// Checks for AWS credentials / ADC instead of API key
```

## Compatibility Detection (OpenAI Completions)

The `detectCompat()` function in `openai-completions.ts` identifies non-standard providers:

| Provider | Compat Flag | Behavior |
|----------|-------------|----------|
| Cerebras | `cerebras` | No cache control, simplified response |
| xAI | `xai` | Specific header requirements |
| z.ai | `zai` | Thinking format variations |
| OpenRouter | `openrouter` | Extra headers, specific error format |
| Qwen | `qwen` | reasoning_content field handling |

This allows a single `openai-completions` provider to work with dozens of OpenAI-compatible endpoints by detecting quirks per provider.

## Key Patterns for foundation_ai

1. **Lazy loading** prevents loading all provider SDKs at startup -- only the needed one is imported
2. **Dual interface** (detailed + simple) lets advanced users access provider-specific features while simple users get a unified API
3. **Compatibility detection** for OpenAI-compatible endpoints avoids needing a separate provider per endpoint
4. **OAuth vs API key** distinction is handled at the provider level, not the registry level
5. **Node.js-only providers** (Bedrock) are gated at the module level
