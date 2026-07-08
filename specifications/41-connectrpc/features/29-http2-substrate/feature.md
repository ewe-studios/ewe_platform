---
feature: "HTTP/2 substrate: frame codec, HPACK, SETTINGS, flow-control math (D12 §5)"
description: "Direction-neutral h2 building blocks, tokio-free, replicated from h2 as reference"
status: "complete"
priority: "high"
phase: 2
depends_on: ["04-incremental-decoder"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 29-http2-substrate: HTTP/2 substrate: frame codec, HPACK, SETTINGS, flow-control math (D12 §5)

## Description

Phase-2 groundwork: the owned, tokio-free HTTP/2 binary layer — codec, header compression, settings and window arithmetic — with h2 as reference, never a fork.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §5, §6, §HTTP/2 module structure (normative substitutions + layout)

## Scope

- frame/ codec (9-byte headers, CONTINUATION) on IncrementalDecoder; hpack/ encoder-decoder; settings.rs; flow_control.rs arithmetic (stream + connection)
- Stream state machine (idle→open→half-closed→closed); priority tree deferred
- Module layout per the doc: foundation_netio/src/http2/{frame,hpack,stream,flow_control.rs,settings.rs,connection.rs}

## Out of scope

- Multiplexers + entry paths (feature 30)

## Acceptance criteria

- HPACK vectors from RFC 7541 pass; frame codec round-trips all frame types; flow-control arithmetic property-tested
