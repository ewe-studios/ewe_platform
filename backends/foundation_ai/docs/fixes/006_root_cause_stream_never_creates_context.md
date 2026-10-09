# 006 — `LlamaCppStream` never creates its context; agent turns return nothing

## Symptom

`answerme-agent` accepted input, loaded the model, and replied `(no response)`
to everything. `AgentSession::run_turn` returned exactly one record:

```
[Summary { message_count: 0, usage: TokenSnapshot { total: 0, input: 0, output: 0, ... } }]
```

No error was raised anywhere. The whole turn took ~1.4 s — the model loaded,
but no tokens were ever generated. Meanwhile the provider-level Gemma 4 tests
(`harness::integrations::gemma_pull`) passed, so the model, the bindings, and
the Jinja chat-template shim were all demonstrably working.

## Investigation

Three layers were eliminated in order:

1. **Tracing** — worker-thread logs were invisible, which made the failure look
   like the REPL was eating output. That was a separate defect (see
   "Related fix" below). Once fixed, the agent loop became observable.
2. **Steering queues** — instrumenting the drain proved the prompt reached the
   loop: `drained=1` with a matching `Arc` pointer on the first outer
   iteration. The queues were never the problem.
3. **The loop itself** — `inner_assemble` logged
   `messages=1 has_system=true pending_user=1` and called `model.stream(...)`.
   So the loop assembled correctly and *did* reach generation.

The break was below the loop. A direct stream test showed the iterator yields
exactly one item and then ends:

```
stream item shapes: ["Init"]
```

`generate()` on the identical model and interaction returned text. Only
`stream()` was dead — and `stream()` is the API the agent loop uses.

## Root Cause

`LlamaCppStream::next()` creates the inference context lazily, gated on the
**backend** being absent (`backends/foundation_ai/src/backends/llamacpp.rs`):

```rust
// Create backend and context on second call if not exists
if inner.backend.is_none() {
    ...
    inner.backend = Some(backend);
    inner.ctx     = Some(ctx);          // the ONLY place ctx is ever set
    return Some(Stream::Pending(ModelState::GeneratingTokens(None)));
}
```

But `LlamaCppStream::new()` now initialises the backend eagerly, to get real
error reporting at construction time:

```rust
// Initialize backend upfront - this is where we can properly report errors
let backend = LlamaBackend::init_or_get().map_err(...)?;
...
LlamaCppStreamInner { backend: Some(backend), ctx: None, ... }
```

So `inner.backend.is_none()` is **always false**, the block never runs, and
`inner.ctx` stays `None` forever. On the second poll the stream falls through
to:

```rust
let Some(mut ctx) = inner.ctx.clone() else {
    inner.finished = true;
    return None;              // silent termination — no error, no tokens
};
```

The guard tests the wrong field. `ctx` is what the block creates and what the
rest of `next()` requires; `backend` merely happens to be created alongside it.
This is a regression against the design recorded in fix 005, which states the
context "is created lazily inside `next()` (not in the constructor), so it's
always born on the polling thread" — moving *backend* creation into the
constructor silently disabled the lazy *context* path.

The failure is invisible because the `else` branch returns bare `None`
(stream exhausted) rather than an error. The agent loop sees a well-formed,
empty stream, collects zero messages, emits no assistant record, and ends with
a `Summary` — which is exactly the observed behaviour.

## Secondary defect — `stream()` drops the conversation

`LlamaCppStream::new()` builds its prompt from `system_prompt`, `soul`, and the
tool definitions only. `interaction.messages` is never read, and no chat
template is applied — unlike the `generate()` path, which goes through the
Jinja shim. Even with the context bug fixed, streaming would prompt the model
with the system text alone and never show it the user's question.

These are independent bugs; fixing the guard alone is not sufficient.

## Fix

1. Gate the lazy-init block on the field it actually initialises:
   `if inner.ctx.is_none()`.
2. Make the exhausted-context path return a generation error rather than a
   bare `None`, so a missing context can never again present as a normal
   end-of-stream.
3. Build the streaming prompt from `interaction.messages` through the same
   chat-template path `generate()` uses, so both APIs prompt the model
   identically.

## Why it was not caught

`run_turn` — the session's primary API — had **no test at any level**, and no
test anywhere exercised `Model::stream`. Every provider suite calls
`generate()`, which takes a completely different code path. The new
`tests/agentic/integrations/session_turn.rs` covers both APIs side by side on
the same model and interaction, so a divergence between them fails loudly:

- `provider_generate_yields_text` — baseline, passes today
- `provider_stream_yields_text` — reproduces this bug
- `run_turn_returns_an_assistant_reply` — reproduces the end-user symptom
- `run_turn_records_the_user_prompt` — guards the prompt surviving the turn

Note also that `provider_generate_yields_text` passes while the model answers
"Reply with a single short greeting." with `"."` (2 output tokens). The
pre-existing assertions only check `!output.is_empty()`, so degenerate output
still reads as green — worth tightening separately.

## Bugs uncovered underneath (all were unreachable while the stream was dead)

Fixing the guard made the rest of the streaming path execute for the first
time, which exposed three further defects:

### a. Use-after-free on the inference context (SIGSEGV)

`next()` did `inner.ctx.clone()` — commented "Clone is cheap (pointer copy)".
That is exactly the problem: `LlamaModelContext` owns a raw
`llama_context` pointer and frees it in `Drop` (`llama_free`), while its `Clone`
copied the pointer. The clone's drop at the end of each poll ran the C++
`~llama_context` destructor and freed the context the stream still held; the
next poll dereferenced freed memory and segfaulted.

Fixed by borrowing in place (`inner.ctx.as_mut()`) **and** deleting the unsound
`impl Clone for LlamaModelContext` in `infrastructure/llama-cpp/src/context.rs`,
so the footgun cannot be re-armed by any caller. It had no other users.

### b. Sampled tokens were never fed back into the context

After sampling, the stream never added the token to a batch and decoded it —
unlike `generate_text`, which does. The KV cache never advanced past the prompt,
so the stream could not make progress. Fixed by mirroring the decode step.

### c. Every failure reported as success

Nearly every failure path returned `ModelState::Finished` (or a bare `None`) —
indistinguishable from a model that finished normally, so a dead stream read as
an empty but successful turn. They now go through a `stream_error` helper
emitting `ModelState::Error`, which `lift_model_item` converts to a
`FailedAction`, making `run_turn` return `Err`. Two paths (`batch.add_sequence`,
`ctx.decode`) also discarded their error value via `.is_err()`; they now report
it. Genuine completions (max tokens, EOG, MTP `step.done`) still report
`Finished`.

## Model was reloaded from disk on every turn

`AgentLoop::transition_inner_assemble` calls `router.get_model()` on every inner
iteration, and `LlamaBackends::load_model` called `LlamaModel::load_from_file`
unconditionally — no cache anywhere in the chain. Every turn re-read the
multi-GB GGUF and re-ran `repack`, which is the loader spam visible per message
in the REPL. Measured: 2 turns in one session = 2 full disk loads.

Fixed with a process-wide `LOADED_MODELS` cache keyed by model path + spec name
+ the config affecting load/context construction. Measured after: 2 turns = 1
disk load + 1 cache hit (5.6s → 4.0s for the two-turn test).

A cache hit returns `share_weights()`, **not** a clone of the wrapper:
`cumulative_cost` lives in the wrapper, so handing out the identical wrapper
would merge unrelated agents' token/cost totals into one accumulator. Weights
(`Arc<LlamaModel>`) and the MTP draft cache are shared; accounting is per-caller.

### Concurrency: is sharing one model between agents safe?

Yes — verified, not assumed, by
`concurrent_agents_share_one_model_safely`: two sessions on two OS threads, three
turns each, all six turns replied with no panic, fault, or cross-talk.

It is safe because the weights are read-only after load and **no inference state
is shared** — every `generate()` and every stream builds its own
`llama_context`, KV cache, and sampler from the shared weights. The wrapper's
mutex is held only briefly to clone `Arc`s, so inference is not serialised.

Known wart: the cache does check-then-load without holding the lock across the
load, so a cold-start race can load the same model twice (measured 2 loads for 6
concurrent calls) before one wins. Wasteful, not incorrect.

## Related fix

Worker-thread tracing was dead, which is why this took so long to see:
`#[valtron]` initialised the pool *before* the tracing subscriber, and each
worker pins the ambient dispatcher at spawn time. Workers captured the no-op
dispatcher, and a thread-local default outranks the global one, so no later
subscriber could reach them. Fixed by moving `#tracing_init` ahead of
`initialize_pool` in `backends/foundation_macros/src/valtron_entry.rs`.
