---
feature: "Lowercase header rendering + protocol header variants (D12 §4)"
description: "Wire-render known headers lowercase; add grpc-*/connect-*/te variants; keep case-insensitive parsing"
status: "pending"
priority: "medium"
phase: 1
depends_on: []
estimated_effort: "small"
created: 2026-07-03
---
# Feature 06-header-lowercase-variants: Lowercase header rendering + protocol header variants (D12 §4)

## Description

Fix the uppercase leak in known SimpleHeader variant wire-rendering and add the gRPC/Connect protocol headers as first-class lowercase variants; the fix targets variant rendering only — From<String> already preserves case correctly.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §4 (variant list is normative)

## Scope

- Display/wire rendering of known SimpleHeader variants → canonical lowercase (HTTP/2 mandates it)
- New first-class variants: grpc-status/-message/-encoding/-accept-encoding/-timeout/-status-details-bin, te, connect-protocol-version/-timeout-ms/-content-encoding/-accept-encoding
- Audit inbound lookups for RFC 9110 case-insensitive matching

## Acceptance criteria

- Wire output lowercase for all known variants; inbound matching case-insensitive; existing tests pass
