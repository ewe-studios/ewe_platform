---
feature: "Interceptor system: chains, UnaryCall/StreamCall, recover (D04)"
description: "Seam interceptors over bytes+metadata; composed once at registration; facade message-middleware hosting"
status: "complete"
priority: "high"
phase: 1
depends_on: ["16-ctx-request-context", "17-transport-seam"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 18-interceptors: Interceptor system: chains, UnaryCall/StreamCall, recover (D04)

## Description

The two extension points split by where the concrete type is known: cross-cutting seam interceptors over bytes + metadata (async, future-returning, once-composed), and per-procedure facade message-middleware where the codec and type live.

## Normative sources (single source of truth — read before writing code)

- decisions/04-handler-and-interceptor-model.md — §Interceptor System, §Streaming interceptor conn, Decided Details OQ#3 (normative)
- decisions/11-transport-seam.md — §Facade message-middleware

## Scope

- Interceptor trait (wrap_unary/wrap_streaming_client/wrap_streaming_handler) + InterceptorChain (reverse-order, composed once)
- UnaryFunc/UnaryCall{headers, codec_name, frame}/UnaryReply; StreamingHandlerFunc/StreamCall{codec_name, conn}; StreamingClientFunc
- Wrappers wrap the UNSPLIT conn; wrapper split() wraps the halves (interception survives split)
- Facade message-middleware chain (byte + optional one-time typed decode) on MessageSink/Source
- RecoverInterceptor (AssertUnwindSafe; conn poisoned after catch; unary + streaming-handler only, H18/H19)

## Acceptance criteria

- Chain composed at registration; per-call codec name rides the call values (unary and streaming)
- A wrapping interceptor observes frames in both directions after split (test)
- Panic in a handler produces Code::Internal via recover, connection not reused
