# Findings — bugs, learnings and dead ends from spec-60

Everything here was found by doing F04 (drive coverage up), not by looking for
it. That is the headline: **the coverage work paid for itself in defects, not in
percentage points.** Seven real bugs, five of them outside `foundation_ai`
entirely, and none would have been found by adding tests to already-covered
code.

Kept in the spec because the *mechanisms* recur, and because several of them
were mis-diagnosed first — the wrong guesses are recorded deliberately.

---

## 1. HF downloads wrote vendor error bodies to disk *as the model file*

`foundation_deployment_huggingface::repository::repo_download_file` errored only
on HTTP **400**. Every other error status — 401/403 on a gated repo, 404, 429,
5xx — fell through to the write path, so the Hub's error body was saved as the
model file and the function returned `Ok`.

The `.part` → rename dance exists to stop a *truncated* download looking cached,
but a 404 body "downloads" successfully, so it got renamed into place. The cache
lookups only check that a file exists, so the poisoned file was served from cache
on every later run — sticky, surviving retries, surfacing much later as a
bad-magic or unreadable-tensor error pointing at the **loader**.

Found in the wild, not theoretically. `artefacts/models/ewe-platform-nonexistent--does-not-exist-xyz-404/`
held `config.json`, `tokenizer.json` and `model.safetensors`, each 29 bytes
containing `Invalid username or password.` Gated models (Llama, Gemma) are the
realistic production trigger.

**Fix:** `status_num >= 400`; 3xx excluded because the client follows HF's CDN
redirect. Verified by restoring `== 400` and watching the new test fail.

**Learning:** the two live tests covering this asserted only `is_err()`. They
passed — on the *loader* rejecting the garbage, not on the download failing. An
`is_err()` assertion cannot tell you *which* thing failed. They now assert the
cache is left clean.

## 2. Both HF providers discarded listing errors

`repo_list_tree` reports HTTP/JSON failures as `Err` items *inside* the stream.
Both providers filtered them out, so a 500 from the Hub became "no GGUF matching
quantization 'q4_k_m'" (sends you after a quantization problem you do not have)
or, for Candle, an empty `Ok` indistinguishable from a repo with no weights.

## 3. valtron split-collector observers silently dropped delivered items

The one that cost the most. `pop()` and `is_closed()` are two separate reads:

```rust
Err(PopError::Empty) => if self.queue.is_closed() { None } else { Some(Stream::Wait) },
```

A producer that pushes **and** closes in the gap leaves the consumer combining a
stale `Empty` with a fresh `is_closed() == true` → it reports end-of-stream while
the item sits in the queue, delivered and unread.

`PopError::Closed` was already authoritative — `concurrent_queue`'s `Single::pop`
returns it only when the slot is empty AND closed, and pops values out of a
closed queue happily. The `is_closed()` re-check was redundant *and* wrong.

**14 copy-pasted sites** across `valtron/extensions/{tasks,streams}/{sendable,non_sendable}.rs`.

**Why it was expensive:** it presented as a *transport* error —
`Backend error: No response intro received` on HF downloads, ~1 run in 8 at
`--test-threads=8`, never single-threaded. Three plausible hypotheses were killed
first (stale pooled connection, `Connection: close` not honoured, TestHttpServer
idle timeout) before anyone instrumented the queue. Load-dependence is inherent:
the window is between two reads on the consumer thread.

**Learning:** *don't stop at a plausible story.* Three of them were consistent
with the evidence and all three were wrong.

## 4. `TestHttpServer` defaults to a non-blocking read

`read_timeout` defaults to 0, making the socket non-blocking. Two silent
failures follow — no error, no retry, no log:

- **Empty request bodies.** If the body has not landed when the server reads, it
  takes the `WouldBlock` and hands the handler a request whose body is empty.
- **Keep-alive is impossible.** After one response it loops, gets `WouldBlock`,
  and closes — so a pooling client reuses a socket the server already dropped.

This was the long-unexplained flaky
`system_prompt_and_soul_are_combined` / `a_zero_temperature_is_omitted…`. For two
sessions it carried a note blaming "contention under full-suite load". That was
a guess, and it was wrong.

**Fix:** `.blocking_read(Some(..))`. Do **not** work around it by sending
`Connection: close` — the server honours it and closes, reproducing the other
failure from the opposite side.

**What actually cracked it:** changing the recorder to keep the whole request
transcript (method + path + body) instead of one slot. The failure then printed:

```
generate() succeeded
requests seen:
  POST /v1/models/o3-mini body=""
  POST /v1/responses      body=""
```

"The capture was empty" is the one fact that does not help. Record what the
server *did* receive.

## 5. `drain_stream()` waited out the **peer's** keep-alive idle timeout

`HttpClientConnection::drain_stream` reads until EOF or error and cannot know how
much is left. On a live keep-alive connection with nothing left to read it blocks
until the *peer* acts — and a keep-alive server does nothing until its own idle
timeout. Every response carrying a body charged the caller the **server's** idle
timeout before the connection went back to the pool.

Measured, one variable at a time (3 sequential requests):

| server idle | client read timeout | elapsed |
|---|---|---|
| 5s | 10s | 15.4s |
| 5s | **2s** | 15.5s — client timeout irrelevant |
| **1s** | 2s | 3.1s — tracks the server exactly |

nginx defaults to 75s keep-alive → **over a minute per request** in production.

**Fix:** bound the drain with a 20ms read timeout, restore the previous one
after. Correct rather than merely cheap: leftover bytes are already in the local
socket buffer; waiting longer waits for bytes that never come.

| | before | after |
|---|---|---|
| netio main suite | 30.5s | 6.6s |
| foundation_ai HF suite (live Hub) | 48.2s | 3.6s |
| foundation_ai main suite | 83s | 42s |

**Learning:** correctness was fine the whole time. Reuse worked after both
consumed and unconsumed bodies. This was flagged as a "plausible latent
correctness bug" — that was speculation, and measuring disproved it.

## 6. A dead pooled connection fails on the READ, not the write

Writing to a socket the peer closed **succeeds** — their FIN closed only their
sending direction. A retry hung off a failed write therefore never fires; the
symptom is `Timeout` from the response read, not `WriteFailed`. The first
implementation retried on write and was useless until this was understood.

**Fix:** pooled connections are marked `from_pool`; the redirect task redials and
replays once when such a connection yields *no response at all*. Safe precisely
there — no response bytes arrived, so the request never reached the server and
replaying cannot duplicate a side effect. Bounded to one attempt, and the retry
dials fresh so it cannot check out another socket the same peer just closed.

A liveness probe is deliberately **not** used: the peer can close between probe
and write, so a probe is an optimisation, not a guarantee. `MAX_IDLE_TIME` also
went 300s → 30s (under nginx 75s / Apache 5s / CDN 60-120s) to keep the retry
rare rather than routine.

## 7. An external test that could not terminate wedged the whole suite

`openrouter_stream_advances` was bounded by *item count*, but each `Delayed(d)`
sleeps for the transport's backoff — so a stream that reconnects instead of
delivering can spend seconds per item and still be thousands of items from the
cap. It stalled a full coverage run, and because valtron serialises the pool,
~700 other tests queued behind it **with no indication why**.

**Fix:** bound it in the transport (connect/read timeouts), not the loop. A
wall-clock check between items cannot fire when `next()` blocks *inside* the
iterator — which was the first attempt, and it did not work.

---

## Cross-cutting: target-gated code with feature-gated dependencies

Found while making `foundation_ai` build for wasm, and it is a *shape* worth
recognising: code selected by `cfg(target_family = "wasm")` whose dependencies
sit behind an opt-in feature. That combination always compiles the code and
sometimes omits what it needs.

- `foundation_auth`'s SubtleCrypto PBKDF2 is target-gated; its `js-sys` /
  `wasm-bindgen` / `web-sys` deps were behind `wasm-pbkdf2`. `--features wasm`
  compiled that code with all four unlinked.
- `foundation_netio`'s wasm fetch client is target-selected; its deps were behind
  `wasm-fetch`, so a consumer that forgot the feature got
  `unimplemented!("no HTTP client backend available")` at **runtime**.
  `foundation_db` is exactly that consumer — its manifest says netio "has a wasm
  `fetch` client" while pulling netio without the feature.

A workspace scan found only these two instances; nine other hits were verified
false positives. Both fixed by making the dependencies unconditional on the
target, since on wasm they are not a choice.

`foundation_ai` had never been built for wasm at all — five layered blockers,
starting with our own `ahash = "0.8"` pulling `runtime-rng` → `getrandom`, which
hard-errors on wasm32. `arrow-array` already asked for the wasm-safe
`compile-time-rng`; we now unify with it.

---

## Method notes that kept paying off

1. **Verify a test fails without the fix.** Two pool tests passed for the wrong
   reason before they were real: `into_parts()` strands the connection so nothing
   is ever pooled, and an unconsumed body makes `drain_stream()` the thing under
   test rather than the retry. Both were only caught by disabling the fix.
2. **Record the transcript, not the verdict.** See finding 4.
3. **Measure one variable at a time.** Finding 5's diagnosis came from holding
   the client timeout fixed and moving the server's.
4. **A guess written down as a cause outlives the session.** "Contention under
   full-suite load" sat in F04 for two sessions and sent the next reader in the
   wrong direction. Label guesses as guesses.
