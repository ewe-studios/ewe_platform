# hermes-agent Provider System

## File Locations

- Provider resolution: `run_agent.py` (API mode detection at lines ~613-629)
- Credential pool: `credential_pool.py`
- Auxiliary client: `agent/auxiliary_client.py`
- Anthropic adapter: `agent/anthropic_adapter.py`

## API Mode Detection (lines ~613-629)

Three API modes are detected automatically:

| Mode | Auto-Detection | Description |
|------|----------------|-------------|
| `chat_completions` | Default | Standard OpenAI-compatible Chat Completions API |
| `codex_responses` | URLs matching OpenAI direct patterns | OpenAI Responses API (`response.output` items) |
| `anthropic_messages` | `api.anthropic.com` or URLs ending in `/anthropic` | Anthropic Messages API |

The mode determines which SDK client is used and how messages/tools are formatted.

## Client Construction

### Non-Anthropic Modes (OpenAI-compatible)

```python
client = OpenAI(
    api_key=api_key,
    base_url=base_url,
    # Provider-specific headers:
    extra_headers={
        "HTTP-Referer": "...",  # OpenRouter
        "X-Title": "...",       # OpenRouter
        # Copilot-specific headers for GitHub
        # Kimi user-agent for Moonshot
    }
)
```

### Anthropic Mode

```python
client = anthropic.Anthropic(
    api_key=api_key,
    base_url=base_url,
)
```

Messages and tools are converted via `anthropic_adapter.py` before sending.

### Codex Mode

```python
# _CodexCompletionsAdapter translates chat.completions.create() calls
# to Responses API format internally
```

## Credential Pool (`credential_pool.py`)

### PooledCredential Dataclass

```python
@dataclass
class PooledCredential:
    key: str              # API key or OAuth token
    auth_type: str        # "api_key" or "oauth"
    provider: str         # Provider name
    strategy: str         # "fill_first", "round_robin", "random", "least_used"
```

### Failover Strategies

| Strategy | Behavior |
|----------|----------|
| `fill_first` | Use first credential until exhausted, then move to next |
| `round_robin` | Cycle through credentials in order |
| `random` | Pick a random credential each time |
| `least_used` | Pick the credential with fewest uses |

### Cooldown Periods

| Error Type | Cooldown |
|------------|----------|
| Rate limit (429) | 1 hour |
| Other errors | 24 hours |

When a credential is exhausted (rate limited or error), it enters cooldown and is skipped until the cooldown expires.

### Credential Rotation

When rate-limited:
1. Current credential enters cooldown
2. Next credential is selected based on strategy
3. Request is retried with new credential
4. If all credentials are in cooldown, fallback to next provider

## Provider Resolution Chain

The agent resolves providers in this order:

1. **OpenRouter** (primary router -- 200+ models)
2. **Anthropic** (native API via `anthropic_messages` mode)
3. **OpenAI** (direct `api.openai.com`)
4. **OpenAI Codex** (Responses API via `chatgpt.com/backend-api/codex`)
5. **GitHub Copilot**
6. **Nous Portal** (`inference-api.nousresearch.com`)
7. **Z.AI / GLM** (`api.z.ai`)
8. **Kimi/Moonshot** (`api.moonshot.ai`, `api.kimi.com`)
9. **MiniMax** (global and China endpoints)
10. **DeepSeek**
11. **Custom endpoints** (any OpenAI-compatible URL)
12. **AI Gateway** (`ai-gateway.vercel.sh`)
13. **ACP providers** (VS Code / Zed / JetBrains via subprocess)

## Auxiliary Client Resolution

Side tasks (context compression, vision, web extraction) use a separate resolution chain:

1. OpenRouter
2. Nous Portal
3. Custom endpoint
4. Codex OAuth
5. Native Anthropic
6. Direct API-key providers (z.ai, Kimi, MiniMax, etc.)

This ensures side tasks use the best available model even if the primary model is different.

## Supported Providers

| Provider | API Mode | Notes |
|----------|----------|-------|
| OpenRouter | chat_completions | Primary router, 200+ models, referer headers |
| Anthropic | anthropic_messages | Native API, prompt caching, prompt caching markers |
| OpenAI | codex_responses / chat_completions | Direct API or via Responses API |
| GitHub Copilot | chat_completions | OAuth-based, specific headers |
| Nous Portal | chat_completions | `inference-api.nousresearch.com` |
| Z.AI / GLM | chat_completions | `api.z.ai` |
| Kimi/Moonshot | chat_completions | `api.moonshot.ai`, custom user-agent |
| MiniMax | chat_completions | Global and China endpoints |
| DeepSeek | chat_completions | Direct API |
| Custom | chat_completions | Any OpenAI-compatible URL |
| AI Gateway | chat_completions | `ai-gateway.vercel.sh` |
| ACP | varies | VS Code / Zed / JetBrains via subprocess |

## Response Parsing

Three normalizers for three API modes:

| Mode | Normalizer | Source |
|------|------------|--------|
| Codex | `_normalize_codex_response()` | Extracts from `response.output` items |
| Anthropic | `normalize_anthropic_response()` | `anthropic_adapter.py` |
| Chat Completions | Direct `response.choices[0].message` | OpenAI SDK |

## Key Patterns for foundation_ai

1. **Auto-detected API modes** based on URL patterns -- no explicit mode configuration needed
2. **Credential pooling** with multiple strategies and cooldown periods is production-hard
3. **Separate auxiliary client resolution** for side tasks ensures best model for each task
4. **Provider-specific headers** (OpenRouter referer, Copilot headers, Kimi user-agent) injected at client construction
5. **Codex adapter** translates Chat Completions API calls to Responses API internally
6. **Fallback chain** from OpenRouter -> direct providers -> custom endpoints
7. **OAuth vs API key** auth types tracked per credential
