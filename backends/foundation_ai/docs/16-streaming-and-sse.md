# Fundamentals 16 — Streaming text and Server-Sent Events (SSE)

How a turn's text reaches a caller, and the one rule that trips people: a
streamed chunk is either a **delta** or a **snapshot**, and the two must not be
treated alike.

Source: `src/agentic/turn.rs`, `src/agentic/agent_loop.rs` (the `join_text_chunks`
rule), and `foundation_netio`'s `SseParser` + wasm `WasmSseIterator`.

---

## 1. Two views of one turn

`AgentSession::run_turn_stream(prompt)` returns a `TurnStream`. That stream is
the raw Valtron `Iterator<Item = Stream<SessionRecord, AgentProgress>>`. It has
two async views:

| View | Returns | `Text` carries | Use it for |
|---|---|---|---|
| `.events()` | `TurnEvent` | the record verbatim | reasoning, tool calls, retractions, progress, the full loop |
| `.into_text_stream()` | `TurnTextEvent` | **new** text to append | a chat UI that appends text |
| `run_turn` / `ask` | collected `Answer` | n/a | the whole turn's text at the end |

`events()` is the richer view; `into_text_stream()` is the convenient view.
They read the same stream, so they end the same way: with one terminal event,
`Done` or `Failed`.

```rust
// Full events: text, tool calls, retracts, progress, then Done/Failed.
for event in turn.events().await {
    match event {
        TurnEvent::Text(t) => { /* verbatim record content */ }
        TurnEvent::ToolCall { name, .. } => { /* … */ }
        TurnEvent::Failed(error) => eprintln!("failed: {error}"),
        TurnEvent::Done(_) => (),
        _ => {}
    }
}

// New text only: append each Text, then read the terminal event.
for event in turn.into_text_stream().await {
    match event {
        TurnTextEvent::Text(t) => print!("{t}"), // always a delta
        TurnTextEvent::Failed(error) => eprintln!("failed: {error}"),
        TurnTextEvent::Done(_) => (),
        _ => {}
    }
}
```

## 2. Delta vs snapshot

Backends disagree on what one chunk means:

- **Local backends** (llama.cpp, Candle) stream each **new piece** — a *delta*.
  `"The"`, `" sea"`, `" breathes"` join to `"The sea breathes"` by concatenation.
- **HTTP backends** (OpenAI, Anthropic, OpenRouter) re-send the **whole text so
  far** — a *snapshot*. `"The"`, `"The sea"`, `"The sea breathes"` all carry the
  full prefix; only the last one is the answer.

`join_text_chunks` (`agentic/agent_loop.rs`) tells the two apart: if every chunk
extends the one before it (`pair[1].starts_with(pair[0])`), they are snapshots
and the last wins; otherwise they are deltas and are concatenated.

```
chunks            -> joined
"The"," sea"," breathes"                    (deltas)     -> "The sea breathes"
"The","The sea","The sea breathes"          (snapshots)  -> "The sea breathes"
```

This rule runs in three places, and they must agree:

1. `Turn::text()` / `Answer` — joins a turn's collected chunks.
2. `coalesce_turn` — joins chunks before they are persisted as one message.
3. `TurnTextStream::from_event` — joins chunks as they stream out.

Loop detection runs on the **coalesced** turn, not the raw chunks, precisely so
a run of near-duplicate snapshots is not mistaken for a loop.

## 3. `into_text_stream` reduces snapshots to deltas

`TurnTextEvent::Text` is **always a delta**. `TurnTextStream` keeps the text
shown so far in a `shown` buffer. For each `TurnEvent::Text(chunk)`:

```rust
let new = if !shown.is_empty() && chunk.starts_with(&shown) {
    // Snapshot: yield only the new suffix.
    let suffix = chunk[shown.len()..].to_string();
    shown = chunk;
    suffix
} else {
    // Delta: yield the whole chunk.
    shown.push_str(&chunk);
    chunk
};
```

A tool call, tool result, or retraction clears `shown` (the text run ended).
Empty chunks are skipped. The stream always ends with `Done` or `Failed`, so a
consumer can tell success from failure even after the last text chunk.

## 4. The "zero response" mistake

Both deployed examples once answered a failed turn with an empty 200. The
failure was dropped at four layers, and the terminal `Failed` never reached the
caller:

1. The providers dropped the partial text and the error on an in-band failure.
   Fix: they emit `Stream::Spread([Done(partial), Pending(ModelState::Error)])`.
2. `lift_model_item` mapped the spread error to `Initializing`. Fix: it maps it
   to `Done(FailedAction)`.
3. The agent loop kept only the first spread item. Fix: it fans the whole spread
   out as `TaskStatus::Spread`.
4. `into_text_stream` dropped `TurnEvent::Failed`. Fix: it yields
   `TurnTextEvent::Failed`.

Rule: **a turn has one terminal event, and it always reaches the caller.** Every
text view (`events`, `into_text_stream`, `Answer`) must forward `Failed` as well
as `Done`. When you add a text view, prove it with a test that fails after
partial text and asserts the partial text *then* the failure.

## 5. The SSE reader side: `WouldBlock` is not EOF

`foundation_ai` consumes SSE through `foundation_netio`'s `SseParser`, which
reads from `std::io::Read`. On wasm, `WasmSseIterator` adapts a JS
`ReadableStream` into that reader. Two contracts matter:

- **Empty queue while the stream is open must be `WouldBlock`, not `Ok(0)`.**
  `Ok(0)` is end-of-stream. A `data:` line split across two JS chunks would
  otherwise parse as a complete line and emit a partial JSON payload
  ("Failed to parse SSE chunk").
- **`WouldBlock` must be distinguishable from a real error.** The wasm reader
  keeps the `io::Error` (`EventSourceError::ReadError(io::Error)`), parks on
  `WouldBlock`, and logs any other error.

A consumer parked on `Stream::Wait` must **park**, never loop: a synchronous
loop that `continue`s on `Wait` spins forever on the single wasm thread. Use
`park_on`, `into_future_stream`, or the async `.events()`/`.into_text_stream()`
views, which already park.

## 6. Choosing the right view

| You want | Use |
|---|---|
| Text to append, as it arrives | `turn.into_text_stream()` |
| Text to append + reasoning + tool activity | `turn.events()` |
| The final text only | `agent.ask(prompt).await` → `Answer` |
| The final text, or the error | `answer.into_result()` |
| The raw Valtron stream | `turn` itself (`Iterator<Item = Stream<…>>`) |

If you show text and the backend is an HTTP one, `events()`'s `TurnEvent::Text`
is a snapshot; appending it raw repeats the prefix. `into_text_stream()` is the
safe default for any UI that appends.
