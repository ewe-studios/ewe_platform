# Learnings

## foundation_http is our primary HTTP server layer (not raw simple_http)

**Why:** The initial spec drafted feature 06 (Native Proxy) using raw `foundation_netio::simple_http` iterators (`HttpRequestReader`, `Http11ResponseIterator`, `HttpConnectionPool`) for hand-rolling the accept loop, HTTP parsing, and route dispatch. This was unnecessary — `foundation_http` already provides a full valtron-integrated HTTP server with routing, middleware, keep-alive, and static asset serving.

**How to apply:**
- **Proxy server**: Use `foundation_http::native::server::HttpServer` — provides `HttpApp<Arc<dyn Serve>>` → `.server(addr)` → `.serve(&shutdown)`. Handles TCP accept loop, non-blocking, valtron task submission per connection, HTTP/1 parsing, keep-alive multiplexing.
- **Route dispatch**: `HttpApp::route_any::<Handler>(path)` — path + method matching built in.
- **Static reloader.js**: `StaticAssetHandler::new(bytes, "text/javascript")` — already exists in `foundation_http::native::handlers::static_asset`.
- **Context/state injection**: `ContextBag` — type-erased `Arc<T>` store, handlers retrieve shared resources (reload channel, build state) via `bag.get::<T>()` in `Serve::create()`.
- **Response helpers**: `respond::json`, `respond::text`, `respond::html`, `respond::redirect` — no manual response building.
- **Per-connection keep-alive**: `ConnectionHandler` is already a valtron `TaskIterator` — `TaskStatus::Delayed` when idle, proper timeout escalation.

**Impact on spec:** Feature 06 (Native Proxy) is `small` effort — we're registering handlers on `HttpApp`, not building a proxy from scratch.

---

## Blocking operations: fire-and-forget with QueueReadiness

**Why:** The initial spec drafted `CargoBuilderTask` running `std::process::Command::output()` inline on the valtron executor thread. `cargo build` can take 30s+ — blocking an executor thread starves all other tasks.

**How to apply:**
- **ProjectBuilderTask**: Pluggable `ProjectBuilder` trait — each builder decides via `should_build(&FileChange) -> bool` whether to run. Multiple builders per task (cargo, wasm-pack, esbuild). Background job receives `Arc<dyn ProjectBuilder>` — calls `builder.build()`, pushes result to queue.
- **BinaryRunnerTask**: Does NOT need background offload — `child.kill()` (<1ms), `Command::spawn()` (<10ms), `child.try_wait()` (non-blocking) are all fast. Uses `QueueReadiness` for build-complete notifications from ProjectBuilderTask.
- **QueueReadiness<T>** — new valtron type: `impl<T: Send> EventReadiness` that returns `!queue.is_empty()`. One type, no race between flipping a bool and pushing to a queue. Added to `foundation_core::valtron::task.rs`.

**BackgroundJobRegistry:**
- Fixed thread pool (scales via `split_thread_count`: 2/3 for valtron tasks, 1/3 for blocking jobs)
- `run_background_job(job)` — dispatches to pool on `multi` mode, inline on wasm/single
- Workers use `ConcurrentQueue` + `catch_unwind` for panic safety

---

## FileWatcherTask already exists — reuse it, don't rewrite

**Why:** Feature 03 drafted a hand-rolled `FileWatcherTask` that spins with `TaskStatus::Wait(self.poll_interval)`. But `foundation_nativeapis::valtron::FileWatcherTask` already exists and uses `TaskStatus::Depends(CompositeReadiness(watcher, stop_signal))` — parks on inotify epoll (Linux) / kevent (macOS), zero CPU spinning.

**How to apply:**
- **No watcher implementation needed** in `foundation_toolings`. Just `use foundation_nativeapis::valtron::FileWatcherTask`.
- **Subscribe** via `task.subscribe()` → returns `mpp::Receiver<WatchEvent>`.
- **Spawn** via `engine.schedule(Box::new(task))` — it's already a `TaskIterator`.
- **Map** `WatchEvent.path → FileChange` in subscriber tasks (ProjectBuilderTask, SSE pump).
- **Subscribers** avoid spinning by returning `TaskStatus::Depends(QueueReadiness)` — the queue itself is the signal, no separate bool to flip.

**Impact:** Feature 03 effort `medium` → `small`. We're wiring subscriptions, not building watchers.

---

## TunnelProxy for non-HTTP traffic on the same port

**Why:** The devserver needs to handle both HTTP connections (proxy + SSE) and raw non-HTTP connections (TCP tunnel to upstream) on the same port. `foundation_http`'s `ConnectionHandler` detects non-HTTP when `read_next_request()` returns `Some(Err(e))` with a non-transient error — currently it sends 400 and closes. We extend it with a user-provided tunnel handler.

**Design:**
```rust
pub trait TunnelProxyTrait: Send + Copy + 'static {
    fn handle(
        &self,
        conn: SharedByteBufferStream<RawStream>,
        streams: HTTPStreams<RawStream>,
        client_ip: &str,
    ) -> ConnectionResult;
}
```

- Registered once on `HttpApp`: `app.tunnel_proxy(TunnelProxy { dest })`
- Invoked inside `ConnectionHandler::handle_idle()` when HTTP parse fails
- Runs on the **same valtron thread** that owns the connection (no thread handoff)
- Same ownership model as `ConnectionResult::Take` — tunnel takes connection, runs to completion, TaskIterator ends
- `Send + Copy` so it can be cloned into each `ConnectionHandler` (struct implementing trait, not trait object)

**Devserver usage:**
```rust
#[derive(Copy, Clone)]
struct TunnelProxy { dest: String }

impl TunnelProxyTrait for TunnelProxy {
    fn handle(&self, conn, _streams, _client_ip) -> ConnectionResult {
        let mut upstream = Connection::connect(&self.dest).expect("connect");
        copy_bidirectional(&mut conn, &mut upstream);
        ConnectionResult::Take
    }
}
```

**Impact:** Single port handles HTTP + raw TCP. No separate tunnel listener needed. Reusable by any `foundation_http` user who needs mixed-protocol serving.

---

## foundation_netio is our raw connection layer (no tokio)

**Why:** The old devserver used `tokio::net::TcpListener` + `tokio::io::copy_bidirectional` for the tunnel path. In `foundation_toolings`, all raw TCP uses `foundation_netio::netcap::connection::{Listener, Connection}` — `std::net`-based with `Read + Write`. No async runtime anywhere.

**How to apply:**
- **TunnelProxy**: `Connection::connect(dest)` + `copy_bidirectional` from `foundation_netio`
- **Raw listener** (if ever needed standalone): `Listener::bind(addr)` → `accept()` → `Connection`
- **No tokio imports** in `foundation_toolings` — `std::io::{Read, Write}`, `std::net`, `std::process` only

---

## foundation_netio already covers networking + SSE

**Why:** The initial spec drafted hand-rolling HTTP parsing, raw socket SSE writers, and a poll-layer integration for the proxy. This was unnecessary — `foundation_netio` already provides all of this.

**How to apply:**
- **Proxy networking**: Use `foundation_netio::netcap::connection::{Listener, Connection}` — provides `TcpListener` accept loop + `TcpStream` with `Read + Write`, no async needed.
- **HTTP parsing/responses**: Use `foundation_netio::simple_http::shared::*` — `SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleHeaders`, `Status`, `Proto`. Already handles HTTP/1 parsing.
- **SSE server**: Use `foundation_netio::event_source::*` — `SseEvent` (builder), `EventWriter<W>` (writes to any `Write`), `SseResponse` (builds correct HTTP headers: `text/event-stream`, `no-cache`, `keep-alive`).
- **No need for** `foundation_nativeapis` poll-layer in the proxy — `Connection` already wraps `std::net::TcpStream` with `Read + Write` and timeout support.

**Impact on spec:** Features 06 (Native Proxy) and 07 (SSE Reload) are now `medium` and `small` effort instead of `large` — we're wiring existing APIs, not inventing them.

**Also updated:** Feature 01 (Crate Scaffolding) Cargo.toml — replaced `libc`, `bytes`, `http` with `foundation_netio`. The dependency graph is simpler.

---

_Created: 2026-06-01_
