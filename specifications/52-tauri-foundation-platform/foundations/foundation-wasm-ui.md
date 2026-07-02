# Foundation: `foundation_wasm_ui` Capability Model

## Purpose of this foundation document

This document establishes what `foundation_wasm_ui` already provides and what
`foundation_platform` must preserve, host, and extend. It is not an implementation
plan for `foundation_wasm_ui`; that work is complete in
`specifications/completed/39-foundation-wasm-ui`. The purpose here is to avoid
mis-designing the Tauri layer by treating the UI foundation as a generic SPA, a
plain WebView page, or a future native-rendering abstraction. It is already a
multi-protocol, multi-rendering-mode runtime.

The central conclusion is:

> `foundation_platform` should not replace `foundation_wasm_ui`. It should host it,
> package it, feed it platform capabilities, and add native/offline/network/cache
> lanes around it.

## Architectural identity

`foundation_wasm_ui` is a Rust-first UI runtime for browser/WebView environments.
Its core architecture is:

```text
Rust application/state/signals/templates
        │
        │ emits typed UI instructions
        ▼
foundation_wasm_ui / foundation_ui_traits protocol layer
        │
        │ encodes DomOps / events / messages
        ▼
foundation_wasm ABI + JS runtime
        │
        │ applies operations through a small host runtime
        ▼
Browser/WebView DOM
```

The browser DOM remains the rendering surface. Rust owns component logic,
reactivity, template expansion, event handling semantics, and protocol encoding.
JavaScript is the host adapter that applies operations to the DOM and forwards
browser events back into the Rust runtime.

This matters for Tauri because Tauri also provides a WebView. The WebView should
be treated as the DOM renderer for `foundation_wasm_ui`, not merely as a static
HTML shell with ad-hoc JavaScript.

## Layer ownership

### `foundation_wasm`

`foundation_wasm` is the lower-level ABI/runtime boundary. Its role is to provide
host communication primitives without DOM ownership.

It owns:

- memory allocations and arena slots;
- binary function-call ABI;
- callback registry;
- timer/schedule hooks;
- protocol framing and dispatch;
- host import/export shape for the JS/Web host;
- custom batch instruction infrastructure.

It intentionally does **not** own DOM concepts. The spec-39 split removed DOM
specific APIs from this layer so that `foundation_wasm` can remain useful outside
UI contexts.

### `foundation_ui_traits`

`foundation_ui_traits` owns shared UI wire types and protocol traits. It provides
the common representation used by encoders and runtimes.

Important concepts include:

- `DomOp` as the canonical UI operation representation;
- `Row` as the shared mapping for DOM operations across encoders;
- `ProtocolEncoder` as the encoder interface;
- protocol envelopes and versioning;
- JSON, columnar, and Arrow-facing representations where appropriate.

This layer is important because `foundation_platform` should not invent a second
UI protocol. Platform-specific transports should carry the existing protocol
shapes.

### `foundation_wasm_ui`

`foundation_wasm_ui` owns UI semantics above the ABI:

- `html!` macro and template expansion;
- reactive signal integration;
- instruction receiver and operation flushing;
- web components and island boundaries;
- DOM morphing;
- event runtime;
- scoped style/theme behavior;
- server-driven UI hooks;
- runtime packaging and JS host integration.

### JS runtime

The JS runtime is intentionally small but architecturally critical. It owns what
only the WebView/browser can own:

- DOM node registry;
- DOM operation application;
- browser event listeners;
- Web Component registration;
- mutation observer wiring where needed;
- fetch/SSE/WebSocket host integration;
- runtime bootstrap and WASM instantiation.

For Tauri, this JS runtime is loaded inside WKWebView, Android WebView, or the
desktop WebView. It remains the DOM peer even when Rust owns platform services.

## Rendering modes already supported or implied

`foundation_wasm_ui` is not one rendering model. It is a family of compatible
modes sharing the same Rust-first UI model and DOM operation vocabulary.

### 1. Client-side WASM UI

The app ships WASM and JS runtime assets to the WebView. Rust/WASM owns signals,
templates, and event handlers. DOM operations are emitted locally and applied by
the JS runtime.

This is the default mental model for bundled Tauri apps:

```text
Bundled Tauri assets
  ├── HTML bootstrap
  ├── foundation-wasm.js
  ├── foundation-wasm-ui.js
  └── application.wasm
```

Strengths:

- instant local boot once assets are present;
- no server dependency for basic UI execution;
- low-latency event handling inside the WebView/WASM boundary;
- ideal for tools, offline-capable screens, and local-first flows.

Constraints:

- WASM and JS assets must be packaged and versioned;
- browser/WebView APIs remain the rendering substrate;
- platform access must pass through Tauri commands/plugins/custom protocols or
  through browser APIs exposed by the WebView.

### 2. Server-rendered first paint

The same template system can produce server-rendered HTML for first paint. The
client can then hydrate islands or attach runtime behavior.

This matters for `foundation_platform` because a Tauri app may load:

- fully bundled static HTML;
- server-rendered HTML from a remote origin;
- server-rendered HTML from a local embedded server/custom protocol;
- cached server-rendered HTML from local storage.

The platform layer must not assume a single SPA entry point.

### 3. Server-driven DOM operations

The server may stream DOM updates as protocol messages rather than ship a full
client-side application state transition. These messages can be applied by the
same DOM operation applicator used by local WASM.

Conceptually:

```text
Remote/local server state
        │
        │ emits DomOps / protocol messages
        ▼
Tauri/network/runtime transport
        │
        ▼
foundation-wasm-ui JS applicator
        │
        ▼
WebView DOM
```

This overlaps with Hotwire-style behavior, but the unit of update can be typed
operations rather than only HTML fragments.

### 4. HTML-over-HTTP / HTML fragment mode

The platform can also deliver HTML fragments and let the UI layer morph or replace
DOM regions. This is closer to Hotwire/Turbo/htmx behavior.

Strengths:

- server can own more UI structure;
- design and form changes can be deployed without app-store release when loaded
  from the server;
- simple mental model for server-rendered applications.

Constraints:

- HTML fragments can be heavier than typed operations;
- client-side state preservation must be explicit;
- offline behavior requires cached fragments and a navigation/cache policy.

### 5. Web Components / islands

Spec 39 completed web component and island support. This gives
`foundation_platform` a useful boundary:

- the outer app shell may be local or server-rendered;
- interactive regions can be mounted as WASM-backed islands;
- islands can be progressively hydrated;
- native/platform capabilities can be exposed as declarative component contracts.

This is especially important for a Tauri + server-driven hybrid because not every
screen needs the same runtime intensity.

### 6. Morphing

DOM morphing allows server or local updates to preserve existing DOM state where
possible instead of replacing entire regions. This is important for:

- form input preservation;
- focus preservation;
- lower visual churn;
- Hotwire-like page transition behavior;
- offline replay of cached UI states.

The platform layer should treat morphing as a rendering strategy available to the
UI runtime, not as a separate framework.

## Protocol and transport distinction

A recurring risk in this spec is confusing protocol, transport, and memory
placement. They must remain separate.

### Protocol

The protocol defines what bytes or structures mean. Examples:

- custom binary batch instructions;
- columnar DOM operation layout;
- JSON DOM operation representation;
- real Arrow IPC batches;
- event payloads;
- function-call ABI frames;
- request-batching envelopes.

### Transport

The transport defines how those bytes move. Examples:

- WASM memory + JS host imports;
- browser fetch;
- SSE;
- WebSocket;
- Tauri command IPC;
- Tauri event emission;
- Tauri custom protocol response;
- local files/assets;
- future native plugin lanes.

### Placement / copy behavior

Zero-copy depends on where memory lives and which runtime can view it. A format
being columnar or Arrow-like does not automatically make it zero-copy across all
boundaries.

For this spec:

- WASM-to-JS inside the WebView can use aligned buffers and TypedArray views.
- Server-to-WebView transport still crosses network/custom-protocol/fetch buffers.
- Tauri command IPC commonly serializes values and should be treated as a light
  command/event lane unless proven otherwise.
- Native Rust memory is not directly readable by WebView JavaScript merely because
  it contains Arrow bytes.
- True zero-copy is realistic inside one memory domain; across WebView/native
  boundaries the goal is usually copy-minimized binary transfer, not literal
  shared pointers.

## Columnar vs Arrow terminology

Spec 39 made an important naming correction:

- **Columnar v1** is the wasm-loop, no-std, typed-array-friendly layout used for
  efficient DOM operation batches.
- **Arrow IPC** means real Apache Arrow IPC, owned by `foundation_arrow` and used
  where a std-capable Arrow path is appropriate.

Therefore, spec 52 should not say “Arrow” for every efficient UI batch. The
platform can prefer Arrow for data sets and analytics-style payloads while still
using columnar v1 or custom batch instructions for DOM operations.

## Event and action model

The event runtime turns browser/WebView events into typed payloads and routes them
back into Rust signal callbacks or function-call ABI handlers. This gives the
platform layer an important contract:

- WebView events are not arbitrary JS callbacks by default;
- event payloads have a structured wire representation;
- signal stabilization and operation flushing are part of the synchronous event
  path where required;
- event wiring can be direct or delegated depending on runtime strategy.

When adding Tauri native capabilities, we should preserve this shape. Native
capability results should enter the UI as structured events, signals, or protocol
messages rather than as ad-hoc global JavaScript calls.

## Server-driven behavior already belongs in the UI foundation

The completed UI foundation includes owned SSE support, request batching, and
server/protocol documentation. This means `foundation_platform` does not need to
invent server-driven UI. It should provide platform-grade hosting around it:

- origin and asset policy;
- online/offline detection;
- cache and replay policy;
- authenticated request integration;
- local custom protocol serving;
- background sync triggers;
- secure native capability invocation;
- packaging and update boundaries.

## What Tauri changes

Tauri changes the environment around `foundation_wasm_ui`:

- the WebView is embedded in a native shell;
- Rust can own filesystem, database, networking, and platform APIs outside the
  WebView;
- commands and events can bridge JS and Rust;
- custom protocols can serve app-owned resources;
- desktop and mobile lifecycle constraints apply;
- native permission/security models become part of app design.

Tauri does **not** change the fact that `foundation_wasm_ui` renders into a DOM
inside a WebView.

## Architectural implication for `foundation_platform`

`foundation_platform` should be designed as a host/platform crate with these
responsibilities:

1. Package and serve `foundation_wasm_ui` assets.
2. Provide a safe bridge between WebView runtime and Rust platform services.
3. Offer transport adapters for local, remote, cached, and streamed UI/data.
4. Coordinate offline-first cache and sync behavior.
5. Integrate native capabilities using explicit permissions and typed contracts.
6. Keep protocol choices pluggable: custom binary, columnar, JSON, Arrow IPC,
   HTML fragments, and future protocols.

It should not:

- create a second UI framework;
- replace the JS DOM applicator with native UI mappings as a default path;
- assume all apps are SPAs;
- assume all server-driven UI is HTML-only;
- force all data through Tauri JSON IPC;
- claim direct native-memory zero-copy into WebView JavaScript without a proven
  mechanism.

## Candidate integration lanes

### UI command lane

For light user actions and control commands:

```text
WebView JS -> Tauri command -> Rust platform service -> response/event
```

Use for:

- open file picker;
- request permission;
- get device/app metadata;
- initiate sync;
- small structured responses.

Avoid for:

- large table transfers;
- high-frequency streaming;
- raw binary data sets unless Tauri support is measured and wrapped carefully.

### Runtime protocol lane

For UI operations and rendering updates:

```text
Rust/WASM/server -> DomOps/HTML/morph payload -> JS runtime -> DOM
```

Use for:

- signal-driven UI updates;
- server-driven patches;
- streamed updates;
- cached UI replay.

### Data/cache lane

For heavy local data:

```text
Rust platform service <-> SQLite/mmap/cache/foundation_db/foundation_arrow
```

The WebView should request views, slices, summaries, or renderable projections,
not necessarily raw full data sets.

### Native capability lane

For device/platform APIs:

```text
UI declarative request -> platform capability registry -> Tauri/plugin/native API
```

This is where Hotwire Bridge Component ideas can be adapted without copying the
Swift/Kotlin-only implementation model.

## Open cautions for later decisions

- Decide where app state lives per app mode: local WASM, Rust platform service,
  remote server, or a hybrid.
- Decide how navigation maps to Tauri windows/WebViews/history.
- Decide which protocols are supported over which transports.
- Decide how offline cache invalidation and mutation replay work.
- Decide whether native background workers are pure Rust/Tauri tasks, platform
  plugin code, or future UniFFI-style direct native wrappers.

## Summary

`foundation_wasm_ui` already provides the UI runtime, rendering protocols,
reactivity model, event model, server-driven hooks, and DOM operation machinery.
`foundation_platform` should become the host and integration layer that makes
those capabilities work well inside Tauri across desktop and mobile, with strong
attention to offline behavior, native capabilities, security, packaging, and
transport selection.
