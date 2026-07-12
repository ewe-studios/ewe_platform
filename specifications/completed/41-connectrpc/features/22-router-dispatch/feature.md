---
feature: "Router, ProcedureMeta, erased handlers, ConnectRpcHandler (D08)"
description: "Path-keyed dispatch: 404/405/415/505 flow, erased unary/stream handlers, foundation_http integration"
status: "complete"
priority: "high"
phase: 1
depends_on: ["17-transport-seam", "18-interceptors", "19-connect-protocol"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 22-router-dispatch: Router, ProcedureMeta, erased handlers, ConnectRpcHandler (D08)

## Description

The server spine: generic-free dispatch over erased handlers with the ProcedureMeta metadata view, exact status-code semantics, capability enforcement, and the foundation_http prefix-route integration.

## Normative sources (single source of truth — read before writing code)

- decisions/08-router-and-dispatch.md — entire doc is normative (entry shape, dispatch flow, registration signatures)

## Scope

- Router (leading-slash HashMap keys, R1) + HandlerEntry { spec, kind, options, protocol_handlers, codec_meta: Arc<dyn ProcedureMeta> }
- ProcedureMeta erased view (has_codec/codec_names/is_binary) — typed table stays inside the wrapper
- ErasedUnaryHandler/ErasedStreamHandler (codec_name param; typed pair resolved before future construction)
- Registration fns (unary/server_stream/client_stream/bidi_stream) taking ProcedureCodecs + async fns of the D04 shapes
- Dispatch flow: 404 → 405(+Allow) → protocol match → timeout/compression/codec name → capability check (505) → spawn exchange tasks → invoke chain
- ConnectRpcHandler as a single prefix route; HandlerOptions (+with_pipe_depth); into_handler(self) freeze; middleware ordering rule

## Out of scope

- Codegen (26)

## Acceptance criteria

- 405/415/505/404 statuses with correct Allow/Accept-Post from ProcedureMeta
- Bidi on HTTP/1.1 rejected 505 via check_compatible; unary GET routed
- Multiple services coexist (path uniqueness); into_handler freezes (no post-build mutation)
