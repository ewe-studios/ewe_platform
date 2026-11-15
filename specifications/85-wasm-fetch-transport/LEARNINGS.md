# LEARNINGS — Wasm Fetch Transport

## Why one owner for the pump (2026-11-15)

Two specifications planned the same code:

- Spec 77 feature 07 had a body pump of about 80 lines for the container port. It said: "It
  does not wait for it." That sentence is the defect, not the decision: it means the same fix
  lands two times, in two shapes, and the second one is never tested against the first.
- Spec 83 feature 05 had `FetchServe` and `FetchTransport` as T19, and `DoStubTransport` as
  part of T21.

A body pump is not a policy. It has one correct behaviour: read a chunk, write it, repeat,
with a bound and a cancel. There is no per-host variation. So it has one owner. The consumers
pass a `Fetcher` or a response body and keep their own policy (CORS, headers, identity).

## Why `FetchServe` lives in `foundation_connectrpc` (2026-11-15)

The alternative was a new crate `foundation_connectrpc_wasm`. It needs `ResponseSink`,
`ConnectRpcHandler`, `CallHooks`, `SimpleIncomingRequest`, `Extensions` and `TransportStream`,
which are all in `foundation_connectrpc` and mostly private. A second crate would force a
public re-export of each one, or `pub(crate)` would become `pub`.

The module `src/wasm/` with `cfg(target_family = "wasm")` keeps the native build unchanged and
the surface one crate.

## Why `web_sys` types and not a custom trait (2026-11-15)

A `Fetcher` that takes a custom request type would need a conversion in every implementation,
and both known implementations (`worker::Stub` and the global `fetch`) already speak
`web_sys::Request`. The custom type would have exactly one job: copy fields to and from
`web_sys::Request`. The `web_sys` type is the common shape.

The cost is a `web_sys` dependency on wasm. `foundation_netio` already has it, and the
`foundation_connectrpc` wasm build needs it for the stream anyway.

## Why the request body is collected (2026-11-15)

Root rule 4 says: stream bodies, never buffer. A fetch request body is the documented
exception, and the reason is in the platform: `fetch` takes the whole body as one value in a
browser, and `Request` with a stream body is not available on every browser that the web client
supports. A Connect unary call and a Connect server-streaming call each send one message, so
the body is small.

The bound is `max_request_bytes`, default 4 MiB, the gRPC default maximum send message size.
The transport refuses a larger body before it sends, so a mistake is an error and not memory
growth.

## Why `SendWrapper` is sound here (2026-11-15)

`TransportStream` holds `Arc<dyn SendBody>`, so the exchange must be `Send`. JS values are not
`Send`. wasm has one thread, so a value that never leaves its thread may claim `Send`.

`foundation_compact::SendWrapper` has `unsafe impl Send` and `unsafe impl Sync` under
`cfg(all(target_family = "wasm", not(target_os = "emscripten"), not(target_feature = "atomics")))`
(`backends/foundation_compact/src/send_wrapper.rs`). On every other target it is
`#[repr(transparent)]` with no assertion, so the bounds come from the inner type. It has **no
run-time check**: it does not panic on a drop from another thread, and it cannot, because the
target it is for has one thread. Do not describe it as an enforcement mechanism. The protection
is compile-time: a build with threads (native, emscripten, wasm with `atomics`) has no impl, so
wrapping a `!Send` value there does not compile.

The alternative, an `unsafe impl Send` at the use site, is what spec 76 removed from
`ContextBag` for a good reason: it hides a future mistake. `SendWrapper` keeps that one
assertion in one audited place, and it also gives the `Sync` half that `Transport` needs.

## Why the DO stub takes headers from the caller (2026-11-15)

The first design put `ewe-forwarded-principal` inside `StubFetcher`. That makes
`foundation_connectrpc` know the agent RPC protocol. Spec 77 feature 07 needs a different
header (its token) on the same transport.

`with_header(name, value)` keeps the transport neutral. Each host adds its own header at the
place that knows the protocol. The cost is one line in each host.

## Dead end: a `Transport` impl directly over `worker::Stub` (2026-11-15)

It removes the `Fetcher` trait. It also removes the browser client from the same code path, and
the browser client is a consumer (spec 83 feature 05 T22). The trait has two real
implementations, so it is not a speculative abstraction.

## What a consumer must not do

1. Read the whole response body. `recv_body` streams; a consumer that needs all of it gives it
   a bound and says why.
2. Write a second pump. If a response is not a `web_sys::Response`, call `response_stream`.
3. Set a CORS header in `FetchServe`. CORS is the policy of one host.
4. Depend on the `cloudflare` feature in a browser build.
