# OpenRouter Provider -- Deep Dive

## Files Referenced

- hermes-agent: Provider resolution chain in `run_agent.py`
- pi-mono: OpenRouter used as primary provider model

## Role

OpenRouter serves as the primary model router in hermes-agent, providing access to 200+ models from multiple providers through a single API key.

## API Mode

OpenRouter uses the standard OpenAI Chat Completions API mode (`chat_completions`), making it compatible with the OpenAI SDK.

## Provider-Specific Headers

When using OpenRouter, these headers are added:

```typescript
{
    "HTTP-Referer": "...",  // Required by OpenRouter
    "X-Title": "...",       // Required by OpenRouter
}
```

These are used for attribution and usage tracking on the OpenRouter platform.

## Model Discovery

OpenRouter provides a `/models` API that returns the list of available models with pricing information:

```
GET https://openrouter.ai/api/v1/models
```

This is used by:
1. pi-mono's `generate-models.ts` script to populate the model catalog
2. hermes-agent's pricing lookup as a fallback when official docs don't have pricing

The response is cached for 1 hour to avoid excessive API calls.

## Pricing Data

OpenRouter models include pricing per million tokens:
- `prompt_tokens_cost_usd_per_million`
- `completion_tokens_cost_usd_per_million`

This data is used by hermes-agent's `estimate_usage_cost()` when `_OFFICIAL_DOCS_PRICING` doesn't have the model.

## Fallback Role

In hermes-agent's provider resolution chain, OpenRouter is the first provider tried:
1. OpenRouter (primary -- 200+ models)
2. Anthropic direct (fallback)
3. OpenAI direct (fallback)
4. Custom endpoints (fallback)

If OpenRouter rate-limits or exhausts all credentials, the chain falls through to direct providers.

## Auxiliary Client Resolution

For side tasks (context compression, vision, web extraction), OpenRouter is also the first resolution choice in hermes-agent's auxiliary client chain.

## Key Patterns for foundation_ai

1. **OpenAI-compatible API** -- works with standard OpenAI SDK
2. **Attribution headers** -- HTTP-Referer and X-Title required
3. **Model discovery API** -- `/models` endpoint for catalog and pricing
4. **Primary router** -- 200+ models through single key, falls back to direct providers
5. **Pricing data source** -- used as fallback when official docs don't have prices
6. **1-hour caching** of model/pricing data balances accuracy with API call overhead
