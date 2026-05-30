---
feature: "Valtron Counter Demo App"
description: "Minimal Cloudflare Workers example with a static HTML page and a /counter endpoint that streams valtron executor ticks via ReadableStream using StreamAsFutureStream (.await) — counter alternates TaskStatus::Wait and TaskStatus::Pending to exercise both executor JS yield branches"
status: "pending"
priority: "medium"
depends_on: ["11-valtron-async-bridge"]
estimated_effort: "small"
created: 2026-05-29
last_updated: 2026-05-30
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature: Valtron Counter Demo App

## Overview

A minimal Cloudflare Workers example app (`examples/cf-valtron-counter/`) that
demonstrates the valtron executor lifecycle in wasm-bindgen with two endpoints:

- **`GET /`** — serves a static HTML page that fetches `/counter` and updates
  the DOM incrementally as tick values arrive
- **`GET /counter`** — returns a streaming response (`ReadableStream`) where each
  chunk is a valtron executor tick, produced via `StreamAsFutureStream` (`.await`)

The counter task alternates between `TaskStatus::Wait` and
`TaskStatus::Pending(count)`, exercising **both** branches in the executor's
JS yield handling:

- `ProgressIndicator::Wait` → `JSThreadYielder.yield_for(JS_WAIT_CHECK_INTERVAL)` (~4ms)
- `ProgressIndicator::SpinWait(duration)` → `JSThreadYielder.yield_for(duration)`

After reaching a configurable max count (default: 10), the task returns
`TaskStatus::Ready(max)` and the stream closes.

## Architecture

```
Browser                          CF Worker (wasm32)
  │                                      │
  ├───── GET / ──────────────────────────►│
  │                                      │
  │◄──── HTML page ──────────────────────┤  serves static HTML with <script>
  │                                      │
  │  <script>                            │
  │    fetch('/counter')                 │
  │      .body.getReader()               │
  │      .read()  ────┐                  │
  │                   │                  │
  ├───── GET /counter ───────────────────►│
  │                                      │
  │                          initialize_pool(42, None)
  │                                      │
  │                          spawn(CounterTaskIterator)
  │                            .scheduled_stream_iter(4ms)
  │                            .into_future_stream()
  │                                      │
  │                    while let Some(item) = stream.next().await:
  │                      Stream::Pending(0)  → controller.enqueue("0")
  │                      Stream::Wait        → JSThreadYielder setTimeout
  │                      Stream::Pending(1)  → controller.enqueue("1")
  │                      ...
  │                      Stream::Next(10)    → controller.close()
  │                                      │
  │◄──── ReadableStream (SSE chunks) ────┤
  │                   │                  │
  │      ────┘  .read() updates DOM      │
  │                                      │
  │  Page shows: 0 → 1 → 2 → ... → 10    │
  └                                      ┘
```

## Key Components

### CounterTaskIterator

A `TaskIterator` that alternates between `TaskStatus::Wait` and
`TaskStatus::Pending(count)`, then `Ready(max)`:

```rust
use foundation_core::valtron::{TaskIterator, TaskStatus, NoAction};

struct CounterTaskIterator {
    current: u64,
    max: u64,
    emit_wait: bool,
}

impl CounterTaskIterator {
    fn new(max: u64) -> Self {
        Self { current: 0, max, emit_wait: false }
    }
}

impl TaskIterator for CounterTaskIterator {
    type Ready = u64;
    type Pending = u64;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current >= self.max {
            return Some(TaskStatus::Ready(self.current));
        }

        if self.emit_wait {
            self.emit_wait = false;
            Some(TaskStatus::Wait)
        } else {
            let tick = self.current;
            self.current += 1;
            self.emit_wait = true;
            Some(TaskStatus::Pending(tick))
        }
    }
}
```

### Stream Iterator Handler (`/counter`)

Uses `scheduled_stream_iter()` → `.into_future_stream()` → `while let Some(item) = stream.next().await`.
`StreamAsFutureStream` implements `futures_core::Stream`, so each `.await` naturally
yields to the async runtime.

The `ReadableStream`'s `start` callback takes a controller and returns a Promise.
The async loop writes directly to the controller — no `spawn_local`, no channel:

```rust
use futures::StreamExt;
use foundation_core::valtron::{Stream, StreamIteratorExt};
use wasm_bindgen_futures::future_to_promise;

async fn handle_counter() -> web_sys::Response {
    initialize_pool(42, None);

    let stream_iter = spawn::<CounterTaskIterator>()
        .with_task(CounterTaskIterator::new(10))
        .scheduled_stream_iter(Duration::from_millis(4))
        .expect("should schedule task");

    let future_stream = stream_iter.into_future_stream();

    // start callback returns a Promise — the async loop runs inside it,
    // writes chunks to the controller directly, and closes when done.
    let start_callback = Closure::wrap(Box::new(move |controller: web_sys::ReadableStreamDefaultController| {
        let stream = future_stream;
        let promise = future_to_promise(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Stream::Pending(count) => {
                        let chunk = format!("data: {count}\n\n");
                        let _ = controller.enqueue_with_u8_array(&chunk.into_bytes());
                    }
                    Stream::Wait => {
                        // executor yielded to JS event loop via JSThreadYielder
                        // just continue polling via .await
                    }
                    Stream::Next(final_val) => {
                        let chunk = format!("data: done: {final_val}\n\n");
                        let _ = controller.enqueue_with_u8_array(&chunk.into_bytes());
                        controller.close();
                        return Ok(JsValue::NULL);
                    }
                    Stream::Ignore | Stream::Init | Stream::Delayed(_) => {}
                    Stream::Spread(_) => {}
                }
            }
            controller.close();
            Ok(JsValue::NULL)
        });
        JsValue::from(promise)
    }) as Box<dyn FnMut(web_sys::ReadableStreamDefaultController) -> JsValue>);

    let rs_init = web_sys::UnderlyingSourceInit::new();
    rs_init.set_start(&start_callback);

    let rs = web_sys::ReadableStream::new(&rs_init);
    start_callback.forget();

    let init = web_sys::ResponseInit::new();
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/event-stream").unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_readable_stream(Some(rs), &init).unwrap()
}
```

The `.await` on `future_stream.next().await` is where the async magic happens:
- `StreamAsFutureStream::poll_next` calls `iter.next()` on the `NotifyQueueStreamIterator`
- When the queue is empty, `iter.next()` returns `Some(Stream::Wait)` after `park_duration`
- The `futures_core::Stream` returns `Poll::Ready(Some(Stream::Wait))`, the handler ignores it, calls `.next().await` again
- Meanwhile, the valtron executor's `JSThreadYielder` handles all JS yielding internally via `setTimeout`
- No manual `js_yield_ms`, `setTimeout`, or `Promise` code — just `.await`

### HTML Page (`GET /`)

Static HTML with client-side JS that fetches `/counter` and updates the DOM:

```rust
const COUNTER_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Valtron Counter Demo</title></head>
<body>
<h1>Valtron Counter Demo</h1>
<div id="output">Starting...</div>
<script>
(async () => {
  const res = await fetch('/counter');
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  const output = document.getElementById('output');
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    const text = decoder.decode(value);
    const lines = text.split('\\n');
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const msg = line.slice(6);
        output.textContent += msg + '\\n';
      }
    }
  }
})();
</script>
</body>
</html>"#;

fn handle_home() -> web_sys::Response {
    html_response(COUNTER_HTML, 200)
}
```

### wasm-bindgen Entry Point

Two endpoints, `fetch` is `async` since `/counter` is async:

```rust
#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, _env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();

    let path = req.url();
    let path = if let Some(pos) = path.find("//") {
        let rest = &path[pos + 2..];
        if let Some(pos2) = rest.find('/') { rest[pos2..] } else { "/" }
    } else { "/" };

    match path {
        "/" => handle_home(),
        "/counter" => handle_counter().await,
        _ => html_response("Not Found", 404),
    }
}
```

## Project Structure

```
examples/cf-valtron-counter/
├── Cargo.toml              # wasm-pack compatible, minimal deps
├── src/
│   └── lib.rs              # wasm-bindgen entry point:
│                           #   - CounterTaskIterator (alternates Wait/Pending)
│                           #   - fetch() → dispatches / vs /counter
│                           #   - handle_home() serves static HTML
│                           #   - handle_counter() async: into_future_stream() + .await + ReadableStream
├── wrangler.toml           # CF Workers config (no D1/KV needed)
└── package.json            # wasm-pack build scripts
```

## Dependencies

Only needs `foundation_core` with `js-wasmbindgen` feature, plus standard
wasm-bindgen crates. No DB, auth, or HTTP framework required.

```toml
[dependencies]
foundation_core = { workspace = true, features = ["js-wasmbindgen"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
futures = "0.3"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Response", "ResponseInit", "Headers",
    "Request", "ReadableStream", "ReadableStreamDefaultController",
    "UnderlyingSource"
] }
worker = "0.8.3"
console_error_panic_hook = "0.1"
console_log = "1"
log = "0.4"
```

## Tasks

1. [ ] Create `examples/cf-valtron-counter/` directory with `Cargo.toml`, `wrangler.toml`, `package.json`
2. [ ] Implement `src/lib.rs` with `CounterTaskIterator`, two endpoints (`/` and `/counter`), ReadableStream handler using `into_future_stream()` + `.await`
3. [ ] Verify `cargo check --target wasm32-unknown-unknown` passes
4. [ ] Verify `wrangler dev` serves the page and counter updates incrementally in the browser
5. [ ] Add feature entry to `requirements.md` frontmatter and increment feature counts

## Verification

```bash
cd examples/cf-valtron-counter
cargo check --target wasm32-unknown-unknown
wasm-pack build --target web
wrangler dev
# Visit http://localhost:8787
# Expected: Page loads instantly showing "Starting..."
# Then /counter fetch streams: 0, 1, 2, ... 10 appear incrementally
# Console logs: "valtron counter tick: 0" through "valtron counter tick: 9"
```

## Success Criteria

- [ ] `cargo check --target wasm32-unknown-unknown` succeeds
- [ ] `cargo clippy --target wasm32-unknown-unknown -- -D warnings` passes
- [ ] `wrangler dev` serves page at localhost:8787
- [ ] Page loads instantly (static HTML), counter values appear incrementally via SSE
- [ ] Both `TaskStatus::Wait` and `TaskStatus::Pending` paths are exercised (verifiable via executor trace logs)
- [ ] Handler uses `into_future_stream()` + `.await` — no manual `js_yield_ms`, `setTimeout`, or `Promise` code

---

_Created: 2026-05-29_
