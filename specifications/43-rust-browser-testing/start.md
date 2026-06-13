---
workspace_name: "ewe_platform"
spec_directory: "specifications/43-rust-browser-testing"
this_file: "specifications/43-rust-browser-testing/start.md"
feature_name: "rust-browser-testing"
created: 2026-06-13
---

# Start: Spec 43 — Rust-native browser testing

A pure-Rust browser test driver + `#[wasm_ui_server]` macro, so we can drive a real
browser from Rust (commands delivered directly over the browser's native JSON-RPC
protocol — CDP/BiDi — plus a thin injected JS helper on our own SSE/WS transport for
in-page-suited verbs) and verify the spec-42 M1–M8 machinery against real paint,
layout, and trusted input.

## Read in order
1. `spec.md` — the full design (architecture, the two JSON channels, the macro,
   the driver API, protocol/browser roadmap, crate layout, build order).
2. The stack we build on (no new low-level code needed):
   - `foundation_netio::websocket::native::connection` — `WebSocketClient::connect`,
     `WebSocketConnection::{send,recv,messages}`, `MessageDelivery` (the CDP/BiDi dial-out).
   - `foundation_http` — `Server::serve`/`serve_with_listener`, App builder, `SseStream`.
   - `foundation_wasm_ui` — DOM-op protocol (`protocol/{json,columnar}`) + scoped-script hydration.
   - `foundation_netio::netcap::ssl` + `foundation_testing::http::tls_server` — TLS.
3. The references: `@formulas/src.UIFrameworks/src.playwright/{playwright,playwright-python}`
   — Chromium launch flags (`chromiumSwitches.ts`, `chromium.ts`) + the JSON-RPC/CDP
   connection shape (`_connection.py`, `_cdp_session.py`).
4. Retire the node path: `foundation_wasm_testbed/src/browser.rs`.

## Build order (see spec §11)
JSON-RPC engine → `CdpClient` + `BrowserSupervisor` (launch/navigate/screenshot) →
`Locator` + DOM/box-model/input/assertions → `#[wasm_ui_server]` macro + `TestServer`
→ `mise` browser tasks + dev cert + templates → injected `primal-test` helper →
phase-2 WebDriver BiDi (Firefox).

_Created: 2026-06-13. Depends on spec-42 (the catalog this verifies)._
