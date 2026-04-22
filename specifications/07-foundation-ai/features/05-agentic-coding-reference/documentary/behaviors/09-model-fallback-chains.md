# Model Fallback Chains

## hermes-agent Fallback Chain

### Provider Resolution Order

1. **OpenRouter** (primary -- 200+ models)
2. **Anthropic** (native API)
3. **OpenAI** (direct)
4. **OpenAI Codex** (Responses API)
5. **GitHub Copilot**
6. **Nous Portal**
7. **Z.AI / GLM**
8. **Kimi/Moonshot**
9. **MiniMax**
10. **DeepSeek**
11. **Custom endpoints**
12. **AI Gateway**
13. **ACP providers**

### Fallback Format Support

Supports both:
- **Legacy format**: Single dict with one fallback provider
- **New format**: List of fallback providers

### Exhaustion Handling

When a provider is exhausted (all credentials in cooldown):
1. Move to next provider in chain
2. If all providers exhausted, abort

## pi-mono Fallback

pi-mono handles fallback through model cycling:
- `--models` flag accepts comma-separated model list
- On certain errors (rate limits, context overflow), cycles to next model
- No explicit provider-level fallback chain

### Automatic Cycling

When multiple models are specified:
- Agent tries first model
- On retryable errors, cycles to next model
- Provides automatic fallback without manual intervention

## Comparison

| Aspect | pi-mono | hermes-agent |
|--------|---------|-------------|
| Fallback scope | Model-level | Provider-level |
| Configuration | `--models` flag | Provider chain config |
| Trigger | Retryable errors | Provider exhaustion |
| Format | Model list | Legacy + new format |
| Automatic | Yes (model cycling) | Yes (provider chain) |

## Key Patterns for foundation_ai

1. **Provider-level fallback** (hermes-agent) is more comprehensive than model-level
2. **Model cycling** (pi-mono) is simpler but effective for same-provider fallback
3. **Legacy + new format support** for backward compatibility
4. **Exhaustion detection** at both credential and provider levels
5. **Ordered chain** with primary -> fallbacks -> custom endpoints
