---
feature: "ErrorWriter + bundled google.rpc.Status/Any (D03/D05)"
description: "Protocol-aware error responses from middleware; Status generated from proto, Any reused from buffa-types"
status: "pending"
priority: "high"
phase: 1
depends_on: ["13-codec-system", "15-envelope-framing"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 21-error-writer: ErrorWriter + bundled google.rpc.Status/Any (D03/D05)

## Description

Protocol-correct error writing for pre-dispatch middleware, with the minimal bundled proto types that make structured details work regardless of the service codec.

## Normative sources (single source of truth — read before writing code)

- decisions/03-error-model.md — §ErrorWriter (H17), §Detail encoding model
- decisions/05-protocol-wire-formats.md — Decided Details #2 (Status bundling decision)

## Scope

- ErrorWriter { status_codec } — no owned pool (with_worker_buffer), no codec table; is_supported/write per protocol
- google.protobuf.Any reused from buffa-types; google.rpc.Status generated via our generator and bundled
- not_modified rendering: 304 + headers, no body, GET-only

## Acceptance criteria

- Auth-layer errors render correctly for all three protocols before dispatch
- Typed details round-trip against connect-go encodings (base64 RawStdEncoding)
