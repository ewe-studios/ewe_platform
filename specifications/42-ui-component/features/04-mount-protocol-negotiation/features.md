# Feature 04: Mount Protocol Negotiation — headers decide, `protocol` overrides

## 1. The design (restated from the original intent)

`mount-data` / `mount-stream` get their payload over a chosen TRANSPORT and
interpret it via a negotiated PROTOCOL:

- **HTTP (fetch / mount-data)**: the response `content-type` header decides
  — `text/html` ⇒ html string, `…primal-json` ⇒ signal patches,
  `…primal-arrow` ⇒ DomOp columns.
- **SSE (mount-stream)**: the stream's messages self-describe — each event's
  `event:` name (`html` / `json` / `arrow`, unnamed ⇒ html) selects the
  handler per message.
- **WebSocket**: no headers — a BINARY frame carries the envelope, whose
  protocol byte + version tell the receiver what it is; TEXT frames are
  json/html by inspection or declared protocol.

## 2. Current state (verified 2026-06-13)

| channel | state |
|---------|-------|
| HTTP fetch | ✅ implemented — `ProtocolHandler.fromContentType` table (`primal-arrow`/`primal-json`/`primal-html`/`text/html` → handler, else Raw) |
| SSE | ✅ implemented — event-name demux (G46), unnamed ⇒ html |
| WebSocket | ❌ gap — `onmessage` hardcodes `streamEventResult("json", data)` for EVERY frame; binary frames are never envelope-sniffed |
| `protocol` attribute | ❌ gap — `mountSetup` reads `api`/`data`/`transport`/`target`/`method` only; no way to force a handler when headers are absent or wrong (proxies, third-party endpoints) |

## 3. Deltas to implement

1. **`protocol` attribute** on `mount-data`/`mount-stream`
   (`protocol="html" | "json" | "arrow"`): when present, the named handler is
   used REGARDLESS of headers/event names — an explicit override, not a
   fallback. When absent, behavior is exactly today's negotiation. Plumb as
   `mountSetup` → transport config → handler resolution
   (`ProtocolHandler.named(name)` beside `fromContentType`).
2. **WebSocket frame routing**: `binaryType = "arraybuffer"`; binary frames →
   parse the envelope (protocol byte 1 + VERSION demux v1 columnar / v2
   Arrow IPC) → DomOp columns route as `arrow`; text frames → declared
   `protocol` attr if set, else json (today's behavior preserved as the
   text default). Malformed envelope → typed error surfaced via the
   transport's error path, not a silent json attempt.
3. **Document the negotiation table** in the runtime JS module docs and the
   crate README §7 (it was implemented in F06/F21 but never written down as
   the contract).

## 4. Testing (JS suite, node:test)

- `protocol` attr forces each handler on mount-data (wrong/missing
  content-type still routes correctly) and per-message on mount-stream.
- WS binary frame with a v1 columnar envelope → DomOps applied; text frame →
  json (unchanged); malformed binary → error path, no patch applied.
- Existing negotiation untouched: content-type table + SSE event demux
  regression-covered (already in suite; extend for the override precedence:
  attribute > headers/event name > default).
