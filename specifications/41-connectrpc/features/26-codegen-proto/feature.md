---
feature: "Unified proto codegen: generator + build.rs + protoc plugin (D10 Modes 1–2)"
description: "One-pass generation of messages + service traits + registration + typed clients from .proto"
status: "pending"
priority: "high"
phase: 1
depends_on: ["13-codec-system", "22-router-dispatch", "24-client-core"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 26-codegen-proto: Unified proto codegen: generator + build.rs + protoc plugin (D10 Modes 1–2)

## Description

The single generator: proto in, everything out — message types, RPITIT service traits with incremental default impls, registration functions building the codec tables, and typed clients.

## Normative sources (single source of truth — read before writing code)

- decisions/10-codegen.md — entire doc is normative (output structure, RPITIT rules, entry points)

## Scope

- Generator in foundation_macros (messages + traits + clients, one pass, learns from buffa-codegen); binary protoc-gen-connect-ewe + connectrpc_build helper in foundation_netio
- Service traits: native RPITIT with explicit + Send (+ 'static via owned Ctx captures, S2); default unimplemented bodies with pinned hidden types (typed Err + stream::Empty)
- procedure constants (R1 leading slash), register_<svc> + register_<svc>_with_codec::<S,C>, Unimplemented<Svc>Handler (R2), <Svc>Name (R3), <Svc>Client struct + trait (R4), new_with_codec, WithSchema (R5), idempotency from proto options
- google.rpc.Status generation path (used by feature 21)

## Out of scope

- Code-first mode (27)

## Acceptance criteria

- Generated code for the D10 GreetService example compiles and serves/calls end-to-end over feature 22/24
- Default trait bodies compile (RPITIT hidden types pinned); zero-copy variant generates proto-only with fixed codec
