---
feature: "Codec system: CodecFor<M> + ProcedureCodecs authority (D02)"
description: "Codec/CodecFor traits, Proto/Json/Arrow codecs, the handler-owned ProcedureCodecs table, CodecSet::of"
status: "pending"
priority: "high"
phase: 1
depends_on: ["12-error-model"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 13-codec-system: Codec system: CodecFor<M> + ProcedureCodecs authority (D02)

## Description

The single codec authority: typed CodecFor<M> dispatch with the message type on the trait, three first-class codecs, and the handler-owned frozen ProcedureCodecs table (no registry, no dyn Any messages, no unified Message trait).

## Normative sources (single source of truth — read before writing code)

- decisions/02-codec-and-serialization.md — entire doc is normative (trait split, blanket impls, table API, flow)

## Scope

- Codec (name/is_binary supertrait) + CodecFor<M> (marshal/unmarshal/marshal_stable/marshal_append); dyn CodecFor object-safe
- ProtoCodec/JsonCodec blanket impls over buffa::Message (+ buffa json feature for canonical JSON — never hand-rolled); ArrowCodec over ToArrow+FromArrow
- Inherent zero-copy: ProtoCodec::unmarshal_owned_view (buffa OwnedView), ArrowCodec::unmarshal_batch (Arc-backed RecordBatch)
- ProcedureCodecs<Req,Res>: of(CodecSet tuple) / defaults() (bounded buffa::Message+Default) / only() / with_codec() / for_request/for_response / names(); frozen at registration
- NO request-time registry; name = Content-Type wire token; miss = 415/constructor error, never fallback

## Out of scope

- Codegen entry points (features 26/27)
- Envelope framing (feature 15)

## Acceptance criteria

- defaults() rejects non-proto types at compile time; of((Proto, Json, Arrow)) compiles for a type in all three families
- Table lookup is O(1) post-negotiation; canonical proto-JSON verified against buffa conformance vectors
