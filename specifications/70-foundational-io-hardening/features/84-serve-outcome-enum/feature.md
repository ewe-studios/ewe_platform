---
feature: "A ServeFuture resolves to a closed outcome enum, so a future cannot return another future"
status: "done"
lane: "after 82 (both touch foundation_http); no dependency on 83"
model: "sonnet"
branch: "spec70/f84-serve-outcome-enum"
crates: ["foundation_http", "foundation_proxy", "foundation_keychain", "foundation_connectrpc"]
catalogue_rows: []  # from the owner review on 2026-11-05; not a §8 catalogue row
---

# Feature 84: `ServeOutcome`, a closed result for `ServeFuture`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## What the owner asked

The owner saw this type:

```rust
pub type ServeFuture = Pin<Box<dyn Future<Output = ConnectionResult> + Send + 'static>>;
```

`ConnectionResult` has an `Async(ServeFuture)` variant, so the type says a future may
resolve to another future, and so on without a limit. The owner asked whether that chaining
is the design, or a gap that needs a smaller enum.

## Answer after the search: it is not the design, and the type should not allow it

1. **The server treats a nested result as a programming error.**
   `ConnectionHandler::apply_async_result` (`foundation_http/src/native/server/connection.rs`,
   "Awaiting: a handler returned Async from an Async future; closing") logs an error and
   closes the connection. It never polls the inner future.
2. **A test pins that behaviour.** `nested_async_closes_with_error` and `NestedAsyncHandler`
   in `foundation_http/tests/async_serve_tests.rs` assert that a nested result closes the
   connection. The test documents the accident, not a feature.
3. **No caller chains on purpose.** The wrappers box a future so they can observe the result:
   `Observing` (`tests/static_file_tests.rs`), `closing` (`foundation_proxy/src/handler.rs`),
   and the `send_*` helpers (`foundation_http/src/native/writer.rs`) all await one future and
   return its result. None returns a second future.
4. **Chaining adds no capability.**
   - A handler that needs a second asynchronous step writes it inside one `async` block:
     `async { first().await; second().await }`. Returning a new future is `await` with more
     machinery.
   - Chaining adds a livelock: a future that resolves immediately to `Async` lets a handler
     keep the connection task runnable forever without progress. There is no progress bound
     and no per-connection budget.
   - Every wrapper must then handle `Async` again, so the return type stops saying what a
     finished handler can do.

The current code therefore already forbids chaining by policy, but the type does not. A
mistake is found at run time (an error log and a closed connection), not at compile time.
This feature moves the rule into the type.

## Design

1. In `foundation_http/src/shared/serve/mod.rs`:

   ```rust
   /// How a handler's async continuation ends. It has no `Async` variant: a
   /// future resolves once, to one of the three connection actions.
   pub enum ServeOutcome {
       /// The handler wrote a response; the connection carries the next request.
       Keep,
       /// The handler owns the connection permanently (WebSocket, SSE, long poll).
       Take,
       /// Close the connection, optionally with an error for the log.
       Close(Option<ErrorTrace<ServeError>>),
   }

   pub type ServeFuture =
       Pin<Box<dyn Future<Output = ServeOutcome> + Send + 'static>>;
   ```

2. Conversions, so call sites stay short:
   - `impl From<ServeOutcome> for ConnectionResult`.
   - `impl ConnectionResult { pub fn split(self) -> Result<ServeFuture, ServeOutcome> }`:
     `Ok(future)` for `Async`, `Err(outcome)` for the three ready variants. Wrappers use it
     to observe both cases without a match on four arms.
3. Change every future that currently ends in `ConnectionResult` to end in `ServeOutcome`:
   - `write_response` (`native/writer.rs`) → `ServeOutcome`; its success is `Keep`, and its
     error is `Close(None)` (it logs the write error, as today).
   - `StaticFileJob::run` (`native/handlers/static_file.rs`) → `ServeOutcome`.
   - `send_continue_100` (`native/writer.rs`): its future ends in `Keep` or `Close`.
   - The async blocks in `foundation_keychain/src/server/native.rs`,
     `foundation_connectrpc/src/native/server.rs` and
     `foundation_proxy/src/handler.rs` return `ServeOutcome`.
   - `foundation_proxy::handler::closing` becomes: await the future, record or rewrite the
     outcome, return `ServeOutcome::Close(None)`.
4. `ConnectionHandler`:
   - `apply_async_result` and `apply_continue_result` take `ServeOutcome`. Delete the
     `ConnectionResult::Async(_)` arm (now unreachable).
   - `Processing` and `await_response` keep matching `ConnectionResult`; the `Async` arm still
     enters `Awaiting`, and the future now yields a `ServeOutcome`.
   - `protocol_detect::finish_after_write` awaits the future and ignores the outcome; it needs
     no logic change.
5. Tests:
   - Delete `NestedAsyncHandler` and `nested_async_closes_with_error`; the type makes the case
     impossible. Keep a short comment in `async_serve_tests.rs` that names this feature, so a
     reader knows why the test is gone.
   - `async_serve_tests::write_or_close` and every test wrapper return `ServeOutcome`.
   - `Observing` (`static_file_tests.rs`) records the `ServeOutcome`.
   - `integration_tests.rs` websocket handler awaits its `send_text` future and returns a
     `ServeOutcome`.
   - Add `async_serve_tests::async_result_take_and_close` if no test covers `Take` and `Close`
     as future results; `Take` must end the connection task, and `Close(Some(e))` must log and
     close.
6. Version bump: `foundation_http`'s public type changes. If feature 82 already bumped it on
   this branch, keep that number; otherwise bump 0.0.4 → 0.0.5. Update every workspace
   requirement and every `bin/platform/templates/*/Cargo.toml.template` pin.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_http -p foundation_proxy -p foundation_keychain \
  -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_http --features multi
cargo test --profile uat -p foundation_proxy
cargo test --profile uat -p foundation_keychain
cargo test --profile uat -p foundation_connectrpc
cargo check --workspace --all-targets --profile uat
python3 scripts/ci/ci.py run foundation_http --profile uat   # in the Linux CI image
```

## Done when

- [x] `ServeFuture` resolves to `ServeOutcome`; `ServeOutcome` has no `Async` variant.
- [x] The nested-result error arm is gone, and its test is deleted with a comment that names
      this feature.
- [x] Every async test passes, including a `Take` and a `Close(Some(_))` result from a future.
- [x] Versions and consumer requirements are updated.
- [ ] CI green.
