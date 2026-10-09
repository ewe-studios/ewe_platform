# Credential Failover Behavior

## hermes-agent PooledCredential System

### Credential Pool

Multiple API keys for the same provider managed as a pool:

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

| Error Type | Cooldown Duration |
|------------|------------------|
| Rate limit (429) | 1 hour |
| Other errors | 24 hours |

### Rotation Flow

1. API call fails with rate limit or error
2. Current credential enters cooldown
3. Next credential selected based on strategy
4. Request retried with new credential
5. If all credentials in cooldown, fallback to next provider

## pi-mono

pi-mono uses per-provider environment variables with OAuth support but no explicit pooling:
- Single API key per provider from env vars
- OAuth tokens for github-copilot and anthropic OAuth
- No automatic failover to backup keys

## Comparison

| Aspect | pi-mono | hermes-agent |
|--------|---------|-------------|
| Key management | Single env var | Pool of keys |
| Failover | None | Automatic |
| Strategies | N/A | 4 strategies |
| Cooldown | N/A | 1h (429) / 24h (other) |
| OAuth support | Yes | Yes |
| Rotation | Manual | Automatic |

## Key Patterns for foundation_ai

1. **Credential pooling** is production-hard for multi-key setups
2. **Multiple strategies** suit different use cases (cost optimization vs reliability)
3. **Cooldown periods** prevent hammering rate-limited keys
4. **Automatic rotation** removes manual intervention
5. **OAuth vs API key** distinction tracked per credential
6. **Fallback to next provider** when all credentials exhausted
