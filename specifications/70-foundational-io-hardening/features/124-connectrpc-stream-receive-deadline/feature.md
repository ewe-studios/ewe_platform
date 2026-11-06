---
feature: "A streaming call's later receives fail on the call deadline"
status: "pending"
lane: "priority follow-up (gap found by feature 123)"
model: "sonnet"
branch: "spec70/f124-stream-receive-deadline"
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

- [ ] Every facade wait named in the Problem table is bounded by the call deadline.
- [ ] The three tests pass, and the two deadline cases fail without the change.
- [ ] Unary and streaming setup behaviour from feature 123 is unchanged.
- [ ] The backlog §1 "streaming call's later receives" entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. The gap was found while feature 123 checked which
waits its `with_deadline` helper covers.
