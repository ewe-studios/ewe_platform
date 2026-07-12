---
feature: "Lowercase header rendering + protocol header variants (D12 §4)"
description: "Wire-render known headers lowercase; add grpc-*/connect-*/te variants; keep case-insensitive parsing"
status: "complete"
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

## Verification (complete)

In `backends/foundation_netio/src/simple_http/shared/impls.rs`:
- `SimpleHeader`'s `Display` (wire rendering) of all ~85 known variants changed to canonical
  **lowercase** (`content-type`, `transfer-encoding`, `te`, `x-content-type-options`, …).
  `Custom(_)` still preserves the caller's exact casing. `From<String>` was already
  case-insensitive (uppercases then matches) — inbound parsing unchanged.
- Added 10 first-class gRPC/Connect variants (enum + `Display` + `From`):
  `grpc-status`, `grpc-message`, `grpc-encoding`, `grpc-accept-encoding`, `grpc-timeout`,
  `grpc-status-details-bin`, `connect-protocol-version`, `connect-timeout-ms`,
  `connect-content-encoding`, `connect-accept-encoding` (`te` already existed, now lowercase).

Existing rendered-output assertions that encoded the uppercase bug were updated to lowercase:
5 in `tests/simple_http/impls_simple_incoming_tests.rs` and 2 in
`foundation_testing/src/netcap/mod.rs` — these are the only workspace assertions on rendered
header casing (websocket/cookie suites already compare case-insensitively).

Tests (`tests/simple_http/header_lowercase_tests.rs`): known variants render lowercase; the 10
protocol variants render their tokens; `Custom` preserves case; inbound parsing maps any casing
to the variant and round-trips to lowercase; a rendered response has no uppercase leak.
`5 passed`; full simple_http `566 passed`; websocket+cookie+http_stream+event_source `230
passed`, 0 failed; foundation_http + foundation_testing compile.
