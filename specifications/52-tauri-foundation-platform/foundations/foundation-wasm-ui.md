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

### 4. Server-driven component delivery (multi-protocol)

This is not merely HTML-over-HTTP. `foundation_wasm_ui` components run entirely
on the Rust side — whether that Rust lives in WASM inside the WebView, on an HTTP
server, or at a remote endpoint. The delivery is multi-protocol:

- **HTML fragments** — server-rendered markup delivered over HTTP, closer to
  Hotwire/Turbo/htmx behavior. The UI layer morphs or replaces DOM regions.
- **Protocol messages (DomOps)** — typed operations streamed from a server or
  local service and applied by the JS runtime's DOM applicator.
- **Arrow / JSON payloads** — structured data delivered as Arrow IPC batches or
  JSON objects, rendered client-side by the UI runtime.

Because the full component definition (template, signals, event handlers) lives
in Rust, and data/streams are attached via `mount-data` and `mount-stream`
capabilities, new components can be delivered without app-store releases and
without feature flags — the server simply sends a new component definition and
the client renders it from the existing runtime.

Strengths:

- server can own component definitions and UI structure;
- design, form flows, and entirely new components can be deployed without
  app-store release when loaded from the server;
- multi-protocol delivery means the right format for the right payload (HTML for
  documents, DomOps for fine-grained updates, Arrow for data-heavy surfaces);
- `mount-data` / `mount-stream` decouple component identity from delivery,
  enabling dynamic component release without client-side feature gates.

Constraints and design responses:

- **HTML fragments can be heavier than typed operations** — this is exactly why
  the platform supports multiple formats. JSON and Arrow payloads are available
  as lighter, more efficient alternatives. The protocol is chosen per payload,
  not locked to HTML.
- **State ownership is server/WASM-side by design** — everything lives on the Rust
  side (WASM or server). This is intentional. From the page's perspective, only
  `mount-data` and `mount-stream` need to care about state, because they
  explicitly declare "I need to send some state to get state-backed updates."
  Components by definition do NOT own state by default; they receive state from
  the Rust side and render it. The user, as they build the app, defines what
  state is important, where it is sent, and why.
  
  Components can cache state if needed for their behaviour or to optimize
  rendering, but this is deliberate and thoughtful, not forced. Some components
  need not care about state at all — they just re-render when new data arrives.
  This design means a large portion of the UI is naturally stateless and very
  cache-friendly, because only the explicit `mount-data`/`mount-stream` bindings
  carry state semantics. Everything else is a pure projection of whatever the
  Rust side provides.
- **Multi-protocol delivery adds complexity** — this is a strength, not a
  weakness. When large updates are efficiently streamed over the right protocol
  (e.g. Arrow IPC for data tables, DomOps for fine-grained UI patches), the
  platform avoids the one-size-fits-all bottleneck that forces HTML for
  everything. The complexity is in the platform layer.

  Protocol selection is NOT a burden on component authors. The Rust side
  (WASM, server, or backend) already decides the protocol. Component and page
  authors don't pick formats — when they add a `mount-data` or `mount-stream`
  binding to a page they can optionally specify a protocol, but in the common
  case it defaults to the efficient Arrow/custom binary format already built
  into the platform. The component just receives data and renders it; the
  protocol is the transport layer's concern.

Offline and caching behaviour depends on where the component's Rust runs:

- **Local WASM** — offline is a no-op. The WASM module and JS runtime are bundled;
  the app runs fine without a server. WASM modules can also be hot-updated by
  downloading a new version through the browser, making runtime updates instant
  without an app-store release.
- **Remote content** — when offline, the platform caches the last rendered page
  as a snapshot and resurrects it instantly when the user returns, so the app
  feels swift even before the network responds. Once connectivity is restored,
  the app fetches the latest content from the server and streams updates in.
  Two update strategies apply depending on the scenario:

  - **Full replace** — the snapshot provides instant perceived responsiveness,
    and when the server sends the latest page, the platform replaces the whole
    view. This is the simplest path and works well for content-driven screens
    where surgical patching adds no value.
  - **Surgical update** — the client sends cached state to the server (e.g.
    "user was on step 3 of this form with these values"), and the server or
    local WASM surgically patches only what changed. This preserves in-progress
    work and avoids disrupting the user's context.

  The platform should support both strategies, selected per route or per
  component, not hardcode one model.
- **Hybrid** — local WASM handles the shell and interactive islands; remote
  content falls back to cache when offline. State continuity is maintained
  across connectivity changes.

### 5. Web Components / islands

Spec 39 completed web component and island support. This gives
`foundation_platform` a useful boundary:

- the outer app shell may be local or server-rendered;
- interactive regions can be mounted as WASM-backed islands;
- islands can be progressively hydrated;
- native/platform capabilities can be exposed as declarative component contracts.

This is not tied to any single configuration. Because `mount-data` and
`mount-stream` can inject content into the page at any point, the same island
and web component model works across every deployment topology:

- **local** — WASM-backed islands inside a fully bundled app;
- **IPC server** — islands driven by a local Rust process over Tauri IPC;
- **in-phone backend** — islands fed by a background service on the same device;
- **server-to-app** — islands hydrated from a remote server over HTTP/SSE/WS.

This universality means `foundation_platform` can mix and match rendering
intensity per region of the page, regardless of where the data or component
definition originates. Not every screen region needs the same runtime,
transport, or data source.

### 6. Morphing

DOM morphing is already complete and shipped in `foundation_wasm_ui` (spec 39,
feature 07, completed 2026-06-12). The `MorphDom` class in
`foundation-wasm-ui.js` provides four-phase reconciliation:

1. **Persistent-ID computation** — tag-mismatch and duplicate exclusion.
2. **Bottom-up ID maps** over both old and new trees.
3. **Child morphing** with best-match scanning (ID-set intersection first, soft
   match with equality lookahead anti-churn second).
4. **Script re-execution** with `WeakSet` guard.

Key capabilities already shipping:

- **Form preservation** — input value/checked, textarea value, select
  selectedIndex; file inputs untouched; form controls are morph leaves.
- **Pantry pattern** — parked nodes are id-bearing, lazily created, cleaned up
  in `finally` (never leaks, even when a morph throws).
- **`moveBefore`** with feature detection + structural fallback.
- **Escape hatches** — `data-ignore-morph` (both sides required),
  `data-preserve-attr` (comma list shields named attributes).
- **Op 16 integration** — `ReplaceChildren` morphs via
  `createContextualFragment` when the document can parse HTML; innerHTML
  fallback otherwise.
- 16 morph tests on the mock DOM, 48/48 JS suite passing.

This matters for `foundation_platform` because morphing is already a first-class
rendering primitive. The platform does not need to add or re-implement it — it
just needs to ensure the JS runtime is loaded and that server-driven and cached
updates flow through the morph path where appropriate. Morphing enables:

- server or local updates to preserve existing DOM state instead of replacing
  entire regions;
- form input and focus preservation across updates;
- lower visual churn;
- Hotwire-like page transition behavior;
- offline replay of cached UI states seamlessly blending into live updates.

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
boundaries. There are two distinct crossing directions, with very different
characteristics.

**Rust ↔ Native platform (iOS/Android/Desktop): the easy case**

On all platforms, Rust compiled as a static library (`.a` on iOS, `.so` on
Android, native binary on desktop) runs in the **same process** as the
application. There is no sandbox boundary, no IPC bridge, no serialization
required. A Rust function can:

1. Allocate an Arrow `RecordBatch` in its own memory.
2. Return a raw pointer (`*const u8`) and length to the caller (Swift, Kotlin,
   or native desktop code).
3. The caller reads the exact same bytes — same memory, same process, no copy.

Because Apache Arrow has a standardized, language-agnostic columnar memory
layout, the native side (Swift, Kotlin, C/C++) can interpret the bytes directly
without any deserialization step. No JSON parsing, no Protobuf decoding, no
`memcpy`. This is the fundamental value proposition of Arrow: the wire format
**is** the in-memory format.

This works today:

- **iOS**: UniFFI or raw C FFI exposes Rust functions to Swift. Swift receives
  an `UnsafeRawPointer` + length, wraps it in `Data`, and reads Arrow columns
  directly from the shared buffer.
- **Android**: JNI or UniFFI exposes Rust functions to Kotlin. Kotlin receives a
  `ByteBuffer`, which wraps the same native memory.
- **Desktop**: Same-process FFI or direct function call. No boundary at all.

The zero-copy guarantee here is real and measurable. A 10MB Arrow table crosses
the Rust→Swift boundary in microseconds — the cost of a pointer write, not a
memory copy.

**Native shell + WASM: zero-copy without compiling to a static library**

The static-library path above requires the user to compile their Rust logic as a
native `.a`/`.so` for each target. But there is an even more flexible
architecture: a **native shell** — a thin, pre-compiled static library that
embeds a WASM runtime — acts as an intermediary between native platform code and
user-provided WASM modules.

```
Native app (Swift/Kotlin/C++)
        │
        │ same process, shared memory
        ▼
Native Shell (compiled static library, shipped once)
  ├── Embedded WASM runtime (wasmtime / wasm3 / wasmi)
  └── Arrow buffers in WASM linear memory
        │
        │ loads and executes
        ▼
User WASM module (same artifact that runs in the WebView)
```

Because the shell is compiled as a static library, it runs in the **same
process** as the native app. The embedded WASM runtime allocates linear memory
that is directly readable by both the WASM module and the host (native) side.
Arrow data written by WASM into its linear memory is accessible to native code
as a raw pointer — same process, shared memory, zero copy.

This has powerful implications:

- **One artifact, two runtimes.** The same WASM module that powers the WebView
  UI also serves as the native-side data engine. No separate native compilation
  step, no per-platform FFI binding generation. The user compiles to WASM once.
- **Zero-copy Arrow across WASM ↔ native.** The WASM module writes Arrow
  `RecordBatch`es into its linear memory. The native shell reads those bytes
  directly (same process, same address space). Arrow's columnar layout IS the
  in-memory format — no serialization, no copy, no JSON bridge.
- **Safe by construction.** WASM's sandbox model means the module cannot access
  arbitrary native memory, only its linear memory. The native shell controls
  what capabilities are exposed (filesystem, network, database APIs). This is
  safer than raw FFI from an opaque native library.
- **App-store update advantage.** WASM modules are data, not native code. They
  can be downloaded and hot-swapped without app-store review. The native shell
  is versioned once and updated rarely. Most application logic changes ship as
  WASM updates.
- **Shell-owned update lifecycle.** The native shell can own the entire WASM
  update process: it knows how to fetch the latest version, talk to an update
  service or local update process, validate integrity, and hot-swap the WASM
  module. The app code and the user never think about updates — the shell
  handles version resolution, download, rollback on failure, and cache
  management. Zero-friction updates: the shell just gets the latest WASM and
  the app runs it.

This means `foundation_platform` can offer users two tiers of native
integration, both with zero-copy Arrow semantics:

| Tier | What the user ships | Zero-copy? | App-store update? |
|------|---------------------|------------|-------------------|
| Static library | Native `.a`/`.so` of their Rust logic | Yes (same process) | Full review needed for logic changes |
| Shell + WASM | Native shell (once) + WASM module | Yes (WASM linear memory in same process) | WASM hot-swap, no review |

The shell is the bridge between "I want zero-copy native performance" and "I
want server-style deployment velocity." You get both.

**Rust native → WebView: the boundary problem**

This is the harder direction. On desktop, native Rust and the WebView run in the
same process and can potentially share memory. On mobile (iOS WKWebView, Android
WebView), the WebView runs in a **separate OS process** — zero-copy across this
boundary is fundamentally limited by process sandboxing. The goal is to minimize
copies, not eliminate them at all costs.

**WebView-internal (WASM ↔ JS)**

Solved. Aligned buffers and TypedArray views give true zero-copy inside the
WebView's single memory domain. WASM writes into its linear memory, JS reads
from the same `ArrayBuffer`. This path is already built into `foundation_wasm`.

**Rust native → WebView: the boundary problem**

On desktop, native Rust and the WebView run in the same process and can
potentially share memory. On mobile (iOS WKWebView, Android WebView), the
WebView runs in a separate OS process — zero-copy across this boundary is
fundamentally limited by process sandboxing. The goal is to minimize copies, not
eliminate them at all costs.

**Concrete mechanisms, ordered by cost:**

1. **WASM memory as the shared buffer** — the Rust native side writes data
   directly into WASM linear memory (which the JS host already has access to).
   The JS side reads it as a TypedArray from the same `ArrayBuffer`. This is
   the closest to true zero-copy: data never leaves the WebView's memory space.
   Requires a mechanism for native Rust to obtain and write into the WASM
   module's memory — feasible when the native side manages the WASM instance
   or when using a Tauri custom protocol to inject data into WASM-accessible
   buffers at startup.

2. **Custom protocol + binary response** — the Rust side serves Arrow IPC bytes
   over a Tauri custom protocol. The WebView fetches the resource via `fetch()`
   or `<script type="import">`. The response body is binary (no JSON
   serialization), and the browser's fetch stack is heavily optimized. This is
   a single copy (Rust buffer → WebView fetch buffer), with zero per-field
   parsing cost since Arrow JS reads the raw `ArrayBuffer` directly. This is the
   recommended default for data payloads.

3. **postMessage / event with transferable ArrayBuffer** — where the WebView
   supports it, binary data can be transferred (not copied) via ownership
   transfer. The sending context loses access; the receiving context gains it.
   Tauri's event system or `postMessage` can carry these. Availability varies
   by platform and WebView version.

4. **SharedArrayBuffer** — true shared memory between native and WebView
   contexts. Requires COOP/COEP headers and specific security policies. Most
   feasible on desktop; mobile support is mixed and may require custom WebView
   configuration. Useful for high-frequency streaming where per-message copy
   costs would dominate.

5. **Tauri command IPC** — serialization is inherent (JSON or MessagePack over
   the bridge). Treat this as the control lane for small structured messages,
   not as a bulk data transport. The copy cost is acceptable for command-sized
   payloads but not for large data tables.

6. **Disk-based handoff (mmap)** — on desktop, the Rust side writes Arrow data
   to a memory-mapped file and serves it via custom protocol. The OS may share
   pages rather than copying. On mobile, process sandboxing typically forces a
   copy. Use this only when data is too large to buffer in memory.

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
