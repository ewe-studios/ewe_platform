---
feature: "Client<Req,Res>, options, per-call Ctx, stream facades (D07)"
description: "The typed client: unary/server/client/bidi surfaces, ClientOptions, per-call derived Ctx, GET support"
status: "pending"
priority: "high"
phase: 1
depends_on: ["19-connect-protocol", "23-h1-transport-client"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 24-client-core: Client<Req,Res>, options, per-call Ctx, stream facades (D07)

## Description

The client half: typed per-procedure clients over the streaming Transport with the propagation-first Ctx contract, split-half stream facades, and idempotent GET.

## Normative sources (single source of truth — read before writing code)

- decisions/07-client-architecture.md — entire doc is normative (contract table, facades, GET, options)

## Scope

- Client<Req,Res> { transport, config, protocol, codecs } + the four async methods
- Per-call Ctx derivation per the contract table (min-deadline merge; CancelSignal::linked; spec/peer overwritten; extensions cloned)
- ClientOptions/ClientConfig incl. codec_name selection, get_url_max_bytes/get_use_fallback/preferred_http_version/with_pipe_depth; Clone impl (R19)
- ServerStream/ClientStream/BidiStream facades over split halves + BidiSender/BidiReceiver; C1 simple variants; C3 spec()/peer(); C4 close
- Unary GET (idempotency-gated) + fallback per the decided rules

## Out of scope

- Generated typed clients (26)

## Acceptance criteria

- Caller cancel stops the call; call deadline never cancels the root ctx (linked one-way test)
- BidiStream::split enables concurrent send/receive (deadlock stress test)
- Unary/streaming round-trips against our own server (feature 22) for Connect + gRPC-Web
