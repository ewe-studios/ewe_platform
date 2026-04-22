# Iteration Budget Behavior

## pi-mono Approach

pi-mono does not have an explicit iteration budget. The agent loop runs until:
- No more tool calls are returned
- The user interrupts via steering
- An error occurs

The outer/inner loop structure handles continuation implicitly.

## hermes-agent Approach

### Dual Budget System

Two independent limits control the conversation:

| Budget | Default | Purpose |
|--------|---------|---------|
| `max_iterations` | Configurable | Hard cap on API call count |
| `iteration_budget.remaining` | 90 | Thread-safe turn counter |

Both conditions must be true to continue:
```python
while api_call_count < self.max_iterations and self.iteration_budget.remaining > 0:
```

### Subagent Budgets

Subagents get their own budget (default 50), separate from the parent agent's budget.

### Refunded Turns

Certain tool turns don't consume budget:
- `execute_code` turns are "refunded" -- they don't decrement the iteration budget
- This allows code execution-heavy tasks to run more turns

### Budget Warning Injection

Budget warnings are injected into tool-result JSON:
```json
{
  "result": "...",
  "metadata": {
    "budget_warning": "10 iterations remaining"
  }
}
```

This avoids breaking the Anthropic cache prefix pattern.

## Key Patterns for foundation_ai

1. **Dual budget** provides two levers: hard cap + soft counter
2. **Refunded turns** for specific tool types enable more turns for expensive operations
3. **Subagent isolation** -- each subagent has its own budget
4. **Budget warnings in tool results** preserves cache prefix
5. **Thread-safe counter** for budget tracking in concurrent environments
