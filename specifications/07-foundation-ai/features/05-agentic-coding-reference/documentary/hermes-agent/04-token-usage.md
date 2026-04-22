# hermes-agent Token Usage and Cost Tracking

## File Locations

- Session tracking: `run_agent.py` (lines ~1202-1212)
- Usage normalization: `agent/usage_pricing.py` (`normalize_usage()`)
- Cost estimation: `agent/usage_pricing.py` (`estimate_usage_cost()`)

## Session Tracking

Cumulative per-session counters tracked on the `AIAgent` instance:

| Counter | Purpose |
|---------|---------|
| `session_input_tokens` | Total input tokens (excluding cache) |
| `session_output_tokens` | Total output tokens (including reasoning) |
| `session_cache_read_tokens` | Total cache hit tokens |
| `session_cache_write_tokens` | Total cache write tokens |
| `session_reasoning_tokens` | Total reasoning/thinking tokens |
| `session_api_calls` | Total API call count |
| `session_estimated_cost_usd` | Cumulative estimated cost |
| `session_cost_status` | Cost estimation status |
| `session_cost_source` | Where pricing data came from |

## Usage Normalization (`normalize_usage()`)

Handles three API response shapes:

### Anthropic Format

```python
{
    "input_tokens": 1000,           # total input tokens
    "output_tokens": 500,           # total output tokens
    "cache_read_input_tokens": 200, # cache hits
    "cache_creation_input_tokens": 100,  # tokens written to cache
}
```

Normalization:
```
input = input_tokens - cache_read - cache_write
output = output_tokens
cacheRead = cache_read_input_tokens
cacheWrite = cache_creation_input_tokens
```

### Codex Responses Format

```python
{
    "input_tokens": 1200,           # includes cache
    "output_tokens": 500,
    "input_tokens_details": {
        "cached_tokens": 200        # cache hits
    }
}
```

Normalization (derived by subtracting cached tokens):
```
cacheRead = cached_tokens
input = input_tokens - cached_tokens
output = output_tokens
```

### OpenAI Chat Completions Format

```python
{
    "prompt_tokens": 1200,          # includes cache
    "completion_tokens": 500,
    "prompt_tokens_details": {
        "cached_tokens": 200
    }
}
```

Normalization (same derivation as Codex):
```
cacheRead = cached_tokens
input = prompt_tokens - cached_tokens
output = completion_tokens
```

## CanonicalUsage Dataclass

All providers are normalized into a single `CanonicalUsage` dataclass:

```python
@dataclass
class CanonicalUsage:
    input_tokens: int       # prompt tokens excluding cache
    output_tokens: int      # completion tokens including reasoning
    cache_read_tokens: int  # cache hits from previous requests
    cache_write_tokens: int # tokens written to cache this request
    reasoning_tokens: int   # reasoning/thinking tokens
```

## Cost Estimation (`estimate_usage_cost()`)

The cost estimation pipeline:

1. **Resolve BillingRoute** -- Determine provider/model/base_url routing
2. **Look up pricing** from sources in order:
   a. `_OFFICIAL_DOCS_PRICING` -- hardcoded official prices (Anthropic, OpenAI, Gemini, DeepSeek)
   b. OpenRouter models API (cached for 1 hour)
   c. Custom endpoint `/models` API
   d. Falls back to "unknown" for unrecognized routes
3. **Calculate cost** using per-million-token pricing:
   ```
   cost = (input * input_price + output * output_price +
           cacheRead * cache_read_price + cacheWrite * cache_write_price) / 1000000
   ```

## PricingEntry

```python
@dataclass
class PricingEntry:
    input: float      # $ per million input tokens
    output: float     # $ per million output tokens
    cache_read: float # $ per million cache read tokens
    cache_write: float # $ per million cache write tokens
```

## BillingRoute

```python
@dataclass
class BillingRoute:
    provider: str     # e.g., "anthropic", "openai"
    model: str        # e.g., "claude-sonnet-4-20250514"
    base_url: str     # API endpoint
```

## CostResult

```python
@dataclass
class CostResult:
    cost_usd: float           # estimated cost
    status: str               # "ok", "unknown", "error"
    source: str               # "official_docs", "openrouter", "custom", "unknown"
```

## Pricing Data Sources

| Source | Models Covered | Refresh |
|--------|---------------|---------|
| `_OFFICIAL_DOCS_PRICING` | Anthropic, OpenAI, Gemini, DeepSeek | Hardcoded |
| OpenRouter models API | 200+ models | Cached 1 hour |
| Custom endpoint `/models` | Custom providers | On demand |
| Unknown fallback | Unrecognized models | N/A |

## Key Patterns for foundation_ai

1. **CanonicalUsage** normalizes all providers into a single dataclass
2. **Session cumulative counters** provide running totals, not just per-request
3. **Three API shape normalization** (Anthropic, Codex, OpenAI) with derived cache values
4. **Multi-source pricing** with fallback chain: official docs -> OpenRouter -> custom -> unknown
5. **OpenRouter caching** (1 hour) balances accuracy with API call overhead
6. **CostResult** carries status and source metadata for debugging
7. **BillingRoute** separates provider/model/base_url routing from pricing lookup
