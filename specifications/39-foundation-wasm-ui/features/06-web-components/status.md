# Feature 06 — Status: COMPLETE (2026-06-12)

## What shipped (foundation-wasm-ui.js)

- **Transport layer** (G28 API: `send` request-response / `connect` streaming /
  `disconnect`): `Transport.create` factory; `FetchTransport` (injectable
  fetch, content-type routing, non-OK rejection), `SSETransport` (G46 — SSE
  `event:` field selects html/arrow/json; exponential reconnect),
  `WebSocketTransport` (same backoff), `ChunkedTransport` (AbortController +
  reader loop), `WorkerTransport`. `reconnectDelay`: 1s/2s/4s… capped 30s.
- **`RequestQueue`** (decision 026): passthrough until the
  `HEAD /primal/messages` probe succeeds, then microtask-batched single
  `POST /primal/messages` flushes.
- **Protocol handlers**: `fromContentType` table (primal-arrow/json/html +
  event-stream variants + text/html + Raw fallback); `HtmlHandler` detects the
  `<island data-target data-action>` wrapper → `html-morph`; `JsonHandler`
  detects the `{morph:{…}}` wrapper → `json-morph`; `ArrowHandler` →
  `ColumnarParser`.
- **`Patcher`**: `materialize` (fragment → hydrate → F08 `scanAndWire`,
  innerHTML fallback for mocks), `route` (one switch shared by both mounts:
  html/morphs→`MorphDom`/json→signal patches/arrow→applicator/raw→text),
  injectable `Patcher.runtime` seams (dispatcher, applicator, signalBridge).
- **`Hydrator`** (G2: styles + scripts ONLY — events belong to F08):
  `scopeCss` pure function (compound selectors each prefixed),
  `CSSStyleSheet` adoption (browser-guarded), per-script error isolation,
  `createScope` (targets/parent/querySelector/addEvent with tracked cleanup).
- **Custom elements** over a node-safe base class: `IslandComponent`
  (hydrate + F08 wiring, disconnect removes adopted sheets + listeners),
  `MountDataComponent` (one request → `resolveMountTarget` → route),
  `MountStreamComponent` (connect → route per result; SSE default).
  `resolveMountTarget` implements the §9 placement table incl. G29
  self-replacement. `registerWebComponents()` idempotent, browser-only.
- **`createPrimal`/`window.primal`** (G30): mountData/mountStream/unmount,
  on/onclick/onchange/off, scope, advanced class access.

## Verification

12 component tests (node): factory routing (1-3), FetchTransport send + 500
rejection (4, 36), content-type table (8-13), island/morph wrapper detection,
RequestQueue probe→bundle (one batch per tick), backoff sequence (7),
scopeCss compound selectors (29), scope addEvent/cleanup (22-24), the full
placement table incl. missing-target throw + self-replacement (33-35, 37 +
G29), mount-data e2e with injected transport (31-33), Patcher signal-bridge
seam. Full wasm_ui JS suite 60/60.

## Deviations / deferred

| Item | Status |
|------|--------|
| `SignalBridge.applyPatches` (JS signal counterpart) | Not in any feature's shipped scope — `Patcher.runtime.signalBridge` is the injection seam; warn-and-drop without it. |
| Live SSE/WS/chunked streaming, CSSStyleSheet adoption, customElements lifecycle | Browser-only; logic shipped behind environment guards, exercised via the web testbed. Backoff/factory/routing logic covered in node. |
| `<island>` tag name | Registered as `primal-island` (custom elements require a dash); the island WRAPPER detection in HtmlHandler still matches server-sent `<island>` payloads per decision 022. |
