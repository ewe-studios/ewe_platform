# Feature 04 — Status: COMPLETE (2026-06-13)

## What shipped (foundation-wasm-ui.js)

- **`protocol` attribute** on mount-data/mount-stream →
  `ProtocolHandler.named(html|json|arrow)`; plumbed through `mountSetup` →
  transport config. Precedence implemented: attribute > headers/event name
  > channel default (fetch + chunked + SSE + WS text).
- **`decodeEnvelopeFrame`**: the 6-byte envelope header is authoritative
  for header-less channels — protocol 1 v1 → ColumnarParser (DomOp columns
  route as `arrow`), protocol 1 v2 → `arrow-ipc` via a REGISTERED reader
  (`ProtocolHandler.arrowIpcReader`, e.g. apache-arrow `tableFromIPC`;
  typed error when unregistered — the core runtime stays lean), protocol 2
  → json. Length-validated; malformed = typed console.error, NOTHING
  applied (never a silent json attempt).
- **WebSocketTransport**: `binaryType = "arraybuffer"`, binary frames
  envelope-sniffed; text frames keep the pre-04 json default unless the
  attribute overrides; `wsFactory` injection for tests.
- **SSE `arrow` events** now decode (base64 → envelope → columns) —
  completes the F11 follow-up stub and pairs with `App::server()` frames
  (feature 03): server frames ship verbatim as WS binary or base64 SSE.
- **The negotiation table** documented at `ProtocolHandler` (JS module) and
  README §7.

## Verification

10 new JS tests (named() table; fetch override beats a LYING content-type +
no-attribute regression; SSE override beats event names; SSE base64 arrow
frame decodes to columns; WS binary v1 → arrow, binary json envelope, two
malformed-frame error cases applying nothing, text default + override; v2
reader registration round-trip). Full JS suite 98/98.
