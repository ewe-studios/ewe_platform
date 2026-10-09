# Cooperative Scheduling, Retry, and thread::sleep

## thread::sleep on Native

The three HTTP providers (OpenAI, Anthropic, OpenAI Responses) use synchronous retry loops
with `std::thread::sleep()` for exponential backoff on rate-limit (429) and server errors
(5xx):

```rust
loop {
    let result = self.do_request::<T>(url, body)?;
    match result {
        Ok(value) => return Ok(value),
        Err((status, retry_after, msg)) => {
            if attempt >= max_retries || !is_retryable_status(status) {
                return Err(GenerationError::Backend(msg));
            }
            let delay = retry_after.unwrap_or_else(|| exponential_backoff(attempt));
            attempt += 1;
            thread::sleep(Duration::from_secs(delay));
        }
    }
}
```

This is fine on native — the valtron executor runs providers on dedicated threads, so
blocking one thread doesn't starve others.

## Why thread::sleep is Wrong on Wasm

`wasm32-unknown-unknown` is single-threaded. `std::thread::sleep()` compiles but panics at
runtime because there is no OS thread to suspend. Even if it didn't panic, sleeping the only
thread freezes the entire runtime (browser tab / CF Worker isolate).

## Cooperative Alternative: Stream::Delayed (Valtron)

On wasm, the valtron executor uses cooperative scheduling. Instead of blocking sleep, a task
yields `Stream::Delayed(duration)` to request re-scheduling after a delay:

```rust
// Cooperative retry (wasm-safe)
fn next(&mut self) -> TaskIteratorResult<T> {
    match self.state {
        State::Waiting { attempt, delay } => {
            self.state = State::Retrying { attempt };
            Stream::Delayed(delay)
        }
        State::Retrying { attempt } => {
            // re-attempt the request
        }
    }
}
```

The executor resumes the task after the delay without blocking.

## Current State in F00c

The `thread::sleep` sites remain unchanged. They are in `execute_request()` — a synchronous
function returning `GenerationResult<T>`, not a `TaskIterator`. Converting to cooperative
retry requires refactoring these into state machines, which is a separate effort.

On wasm today, providers build and the `HttpClient` trait works, but the non-streaming
`generate()` path with retries will panic if a retry is needed. The streaming path
(`stream()`) does not use `thread::sleep` and works correctly.

## Retry / Backoff Strategies

The codebase uses exponential backoff with optional `Retry-After` header respect:

```rust
fn exponential_backoff(attempt: u32) -> u64 {
    1u64 << attempt.min(5)  // 1, 2, 4, 8, 16, 32 seconds
}
```

Provider-supplied `Retry-After` takes precedence. The pattern is consistent across all three
providers.

## Cloudflare Workers and ?Send Futures

CF Workers run on V8 isolates — single-threaded, no `thread::spawn`, no `thread::sleep`.
Futures must be `!Send` (they cannot move between threads because there is only one).

The `?Send` constraint appears in the valtron executor's wasm path: task futures are
`Pin<Box<dyn Future<Output = T>>>` (no `Send` bound) vs native's
`Pin<Box<dyn Future<Output = T> + Send>>`.

This means wasm providers can hold `!Send` types (e.g., `web_sys::Response`,
`js_sys::Promise`) inside their futures without wrapper hacks. The `HttpClient` trait's
return types are `Box<dyn Iterator>` (sync) — not futures — so Send doesn't apply to the
provider layer directly.

## How a Wasm Deployment Supplies a Provider

On native, `OpenAiProvider::new()` auto-creates a `SimpleHttpClient` via
`default_http_client()` (cfg-gated off wasm). On wasm, the caller must inject one:

```rust
let fetch_client = FetchHttpClient::new();  // from foundation_netio, wasm-only
let provider = OpenAiProvider::with_http_client(Arc::new(fetch_client));
```

This is intentional: wasm environments vary (browser vs CF Workers vs Deno), and the
correct `FetchHttpClient` configuration depends on the host. There is no universal default.
