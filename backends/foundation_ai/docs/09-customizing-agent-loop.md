# How-To: Customizing the Agent Loop

The knobs that change how a turn behaves, what each one really does, and when
to touch it. Doc 01 explains the loop itself.

---

## 1. Iteration limits

```rust
let agent = AgentSession::builder(router)
    .with_config(AgentConfig {
        max_inner_iterations: 10,   // default 25
        max_outer_iterations: 3,    // default 10
        ..AgentConfig::default()
    })
    .with_model(model_id)           // with_model wins over config.primary_model
    .build()?;
```

- **Inner iterations** count tool rounds: generate → run tools → generate
  again. Hitting the cap moves on to output processing; the turn is not an
  error.
- **Outer iterations** count queue drains within one `run_turn`: the prompt,
  then any steering or follow-up messages that arrive while it runs.

Raise inner for long tool chains (read → analyse → edit → test). Lower it to
cap cost on simple agents.

## 2. Fallback models and the circuit breaker

```rust
let agent = builder
    .with_model("claude-sonnet-4-6")
    .with_fallback_models(["gpt-4o"])
    .with_config(AgentConfig { circuit_breaker_threshold: 2, ..AgentConfig::default() })
    .build()?;
```

The breaker moves to the next fallback when the error policy says
`SwitchModel` (rate limits, by default) or loop detection escalates to
`SwitchModelOrTemperature`, once `threshold` consecutive failures have
accumulated. A successful generation resets the count. With no fallbacks
left, a `SwitchModel` error ends the turn.

Every fallback must resolve in the router (Doc 03).

## 3. Budgets, compression and context pressure

All three use the **session token budget** — the `remaining()` of
`SessionAccessProvider::token_budget(user)` at `build()`. With the default
`AllowAllAccess` there is no budget, and none of them do anything.

| Setting | Default | Effect |
|---|---|---|
| Budget exhausted | — | Turn ends with `AgenticError::BudgetExhausted` |
| `can_spend` refused | — | Turn ends with `AgenticError::Budget` (asked before every generation) |
| `preflight_compression_threshold` | 0.85 | Before each request, drop the oldest context messages until the estimate is under `threshold × budget`. `0.0` disables. |
| `context_pressure_threshold` | 0.70 | When the estimate passes `threshold × budget`, append a "be concise" note to the system prompt. `0.0` disables. |
| `max_tokens` clamp | — | Each request's `max_tokens` is capped at the budget's remaining tokens |

These do not look at the model's context window. A provider-side
context-overflow error is handled separately by the error policy
(`RetryWithReducedContext`, which re-assembles — so it only shrinks the
context when a budget makes compression kick in).

To set a budget, supply an access provider (Doc 08 §6) or call
`agent.ledger().set_budget(Some(tokens))` after `build()`.

## 4. Sampling parameters

```rust
let config = AgentConfig {
    model_params: ModelParams {
        temperature: 0.2,
        top_p: 0.95,
        max_tokens: 4096,
        stop_tokens: vec!["</answer>".into()],
        ..ModelParams::default()
    },
    ..AgentConfig::default()
};
```

The same params go to every request in the session. Backends ignore fields
they don't support (Doc 02 §8 lists them). Loop detection's
`SwitchModelOrTemperature` does **not** change the temperature today.

## 5. Context assembly

```rust
let agent = builder
    .with_context_config(ContextConfig {
        recent_message_count: 40,          // default 20
        inject_newer_observations: true,
        ..ContextConfig::default()
    })
    .build()?;
```

`recent_message_count` is the main lever on how much history each request
carries.

## 6. Memory triggers

```rust
let agent = builder
    .with_memory_config(MemoryConfig {
        observation_trigger_tokens: 10_000,
        reflection_trigger_tokens: 20_000,
        ..MemoryConfig::default()
    })
    .build()?;
```

Crossing a threshold only emits `AgentProgress::ProcessingMemory` today;
nothing generates the observation or reflection (Doc 11). To silence the
signal, set both thresholds to `u64::MAX`.

## 7. Loop detection

The loop uses `LoopDetectorConfig::default()`:

| Field | Default |
|---|---|
| `window_size` | 5 |
| `similarity_threshold` | 0.9 (simhash) |
| `tool_call_max_repeats` | 3 |
| `max_redirects` | 3 |
| `try_model_change` | `true` |
| `temperature_delta` | 0.3 (unused by the loop) |
| `detect_vacuous_answers` | `true` |
| `judge_bare_numbers_against_question` | `true` |

`AgentSession` has no way to change it yet. `LoopDetector` is public if you
drive your own loop.

## 8. Error policy

Defaults and the override pattern are in Doc 01 §6. One more example —
treat any provider failure as a reason to fall back instead of ending:

```rust
let policy = ErrorPolicy::customize(|error| match error {
    AgenticError::Generation(ref f) if f.kind == GenKind::Provider => AgentAction::SwitchModel,
    other => ErrorPolicy::new().classify(other),
});
```

## 9. Tool behaviour

- **Retries:** `agent.tool_manager().set_retry_config("name", ToolRetryConfig { .. })`
  (Doc 04 §5).
- **Order:** calls run one at a time in dependency order (Doc 04 §4).
- **Which tools a model sees:** whatever is registered on
  `agent.tool_manager()` when a request is assembled. `deregister(name)`
  removes one mid-session.
