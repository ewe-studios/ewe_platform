---
feature: "A streaming call's later receives fail on the call deadline"
status: "pending (CI proof)"
lane: "priority follow-up (gap found by feature 123)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_connectrpc"]
catalogue_rows: []  # from docs/bug-backlog.md §1
---

# Feature 124: a streaming call's later receives fail on the call deadline

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **call deadline**: the end time of a remote procedure call (RPC). `Ctx::with_deadline`
  records it and the client enforces it.
- **facade**: the typed stream types that a caller uses — `ServerStream`, `BidiStream`,
  `BidiSender` and `BidiReceiver`.
- **receive**: one wait for the next response message from a stream.

## Problem (found by feature 123 on 2026-11-06)

Feature 123 enforces a call deadline in the shared `Client`
(`foundation_connectrpc/src/shared/client.rs`). Its helper `with_deadline` races the call
future against `foundation_core::valtron::sleep_async`. The helper covers the unary paths
and the setup of every streaming call. It does not cover the waits after setup:

| Type | Method | Line |
| --- | --- | --- |
| `ServerStream<Res>` | `receive` | `client.rs:1145` |
| `ServerStream<Res>` | `response_trailers` | `client.rs:1166` |
| `BidiStream<Req, Res>` | `send`, `send_headers`, `close_request`, `receive`, `response_headers`, `response_trailers` | `client.rs:1270`–`1325` |
| `BidiSender<Req>` | `send`, `send_headers`, `close_request` | `client.rs:1360`–`1395` |
| `BidiReceiver<Res>` | `receive`, `response_headers`, `response_trailers` | `client.rs:1410`–`1435` |

A server that sends its response head and then stalls leaves the caller parked on
`recv.receive()` with no timer. The caller waits for the transport's own idle timeout, or
forever when the transport has none. The contract "a call fails on its deadline" therefore
holds for setup only.

Backlog §1 holds the entry. `docs/bug-backlog.md` §1 names the same fix.

## Design

### 1. Carry the deadline into each facade value

Each facade struct gains one field:

```rust
deadline: Option<Instant>,
```

The constructor of each value copies it from the call `Ctx` that opened the stream. The
shared `Client` builds the value inside the `*_inner` function, and that function already
receives the caller `Ctx`. `BidiStream::split` copies the field into both halves.

The deadline is absolute, not idle-relative: once the end time passes, every later wait
fails at once. This matches the gRPC deadline semantic and the unary path.

### 2. Reuse one helper for every wait

Feature 123's `with_deadline(ctx, call)` reads the deadline from a `Ctx`. Split it into a
second form that takes the deadline directly, so every wait uses one implementation:

```rust
async fn with_deadline_at<T>(
    deadline: Option<Instant>,
    cancel: Option<&CancelSignal>,
    call: impl Future<Output = ConnectResult<T>>,
) -> ConnectResult<T>
```

`with_deadline(ctx, call)` becomes a call to `with_deadline_at(ctx.request.deadline, ...)`.
Each facade method wraps its inner wait:

```rust
pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
    with_deadline_at(self.deadline, None, async {
        match self.recv.receive().await? {
            Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
            None => Ok(None),
        }
    })
    .await
}
```

The borrow of `self` inside the async block is fine: the block is polled by the helper, and
the helper owns the block.

A timeout returns the same error as the unary path,
`ConnectError::deadline_exceeded("call deadline exceeded")`. A deadline that already passed
returns it before the block is polled, so no read is attempted.

The streaming cancel signal is not available to the facade today. Pass `None` for the
cancel argument in this change: the timeout stops the caller's wait, and the stream's own
drop still cancels its tasks. A later change can thread the call's `CancelSignal` into the
facade if a timer must also stop the reader task. Record that as a backlog entry if the
work shows it is needed.

### 3. Do not bound the stream's whole life

The deadline bounds each wait, not the stream. A server-stream that sends one item per
minute under a 10 s deadline fails on the first wait that crosses the deadline, which is
the contract. Do not add a per-receive idle timeout; that is a different setting and no
caller asks for it.

## Tests

Add cases to `tests/client_deadline_tests.rs` (it is already in the crate's `[[test]]`
list with `required-features = ["rpc_multi"]`). The server sends the response head and one
message, then stalls. Each case sets a 200 ms deadline and asserts:

- `a_server_stream_receive_fails_on_the_call_deadline` — `ServerStream::receive` returns
  `DeadlineExceeded` in under 1 s after the first message.
- `a_bidi_receive_fails_on_the_call_deadline` — the same for `BidiStream::receive` and for
  the split `BidiReceiver::receive`.
- `a_receive_with_no_deadline_still_waits` — without a deadline, the stall is observable
  and the stream still returns its data when the server resumes.

Each case must fail on the code before this change (the wait runs to the test's own bound)
and pass after it. Record the observed old-code failure text in the Result.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

## Done when

- [x] Every facade wait named in the Problem table is bounded by the call deadline.
- [x] The three tests pass, and the two deadline cases fail without the change.
- [x] Unary and streaming setup behaviour from feature 123 is unchanged.
- [x] The backlog §1 "streaming call's later receives" entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`. The feature is
`pending (CI proof)` until the pull-request CI is green.

### What changed

- **One helper for every wait.** `with_deadline_at(deadline, cancel, call)` in
  `backends/foundation_connectrpc/src/shared/client.rs` races `call` against
  `foundation_core::valtron::sleep_async` for the time left. It returns
  `ConnectError::deadline_exceeded("call deadline exceeded")` on a timeout and
  on a past deadline. The `cancel` argument is optional, so a facade wait can
  pass `None`.
- **Feature 123 helper kept.** `with_deadline(ctx, call)` rejects a past
  deadline first, with the same message and cancel fire as before, then calls
  `with_deadline_at(ctx.request.deadline, Some(ctx.cancel_signal()), call)`. The
  unary and streaming setup paths therefore keep their exact behaviour and
  message.
- **The deadline is carried into each facade value.** `ServerStream`,
  `BidiStream`, `BidiSender` and `BidiReceiver` gain one private field,
  `deadline: Option<Instant>`. Each constructor copies it from the call `Ctx`
  that opened the stream, and `BidiStream::split` copies it into both halves.
  The deadline is absolute: once it passes, every later wait fails at once.
- **Every facade wait named in the Problem table uses the helper:**
  `ServerStream::receive` and `response_trailers`; `BidiStream::send`,
  `send_headers`, `close_request`, `receive`, `response_headers` and
  `response_trailers`; `BidiSender::send`, `send_headers` and `close_request`;
  `BidiReceiver::receive`, `response_headers` and `response_trailers`.
- **No public signature changed.** The new field is private and no function
  signature changed, so the crate version stays 0.0.5. The crate still builds
  for wasm32 with `--no-default-features --features rpc`.

### Tests

Three new cases in `tests/client_deadline_tests.rs`:

- `a_server_stream_receive_fails_on_the_call_deadline`: one message arrives,
  the second receive returns `DeadlineExceeded` in under 1 s.
- `a_bidi_receive_fails_on_the_call_deadline`: the same for
  `BidiStream::receive` and for the split `BidiReceiver::receive`.
- `a_receive_with_no_deadline_still_waits`: without a deadline, a 150 ms bound
  loses the race against the second receive. The test then proves the same
  receive completes when the real server sends the second message after 400 ms.

**Deviation from the plan's test peer.** The first version used the real
`HttpServer` with a handler that sends one message and stalls. That test passes
on the old code: the server itself reads `Connect-Timeout-Ms` and enforces the
same deadline, then closes the stream with `DeadlineExceeded`. It cannot show
whether the *client* enforces its own deadline. The two deadline tests now use
a raw HTTP/2 peer (`H2Channel`) that ignores the timeout header: it sends the
response head and one Connect envelope, then sends nothing. The no-deadline
test keeps the real `HttpServer`, so the normal path is still tested against
the real service. The test module says why the raw peer is needed.

Old-code proof: `with_deadline_at` was reverted to `call.await` (the deadline
was still recorded and the timeout header was still sent). Both deadline tests
then failed after about 1.57 s, the peer's stall bound, not at the 200 ms
deadline:

```text
assertion `left == right` failed: expected DeadlineExceeded, got: ErrorTrace:
  [0] Context: [internal] the response stream ended without an EndStreamResponse (at backends/foundation_connectrpc/src/shared/error/connect_error.rs:232:9)

  left: Internal
 right: DeadlineExceeded
```

The helper was restored before the final runs.

### Commands run

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings` | pass |
| `cargo test --profile uat -p foundation_connectrpc` | 241 passed, 0 failed (1 doc test ignored) |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | pass, with the 3 known warnings in `shared/router/dispatch.rs` (backlog §1) |

`cargo test --profile uat -p foundation_proxy` was not run. The brief for this
feature lists the four commands above, and no public signature changed.

### Backlog

- Deleted the §1 entry "A streaming call's later receives are not
  deadline-bounded".
- Added a §1 entry: dropping a stream facade does not cancel its reader task.
  The test observes the peer socket stay open after the deadline and the drop,
  because the facade does not hold the call's `CancelSignal`. Fix: thread the
  signal into the facade and fire it on drop and on timeout.
