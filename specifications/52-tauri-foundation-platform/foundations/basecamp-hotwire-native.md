# Foundation: Basecamp Hotwire Native Architecture

## Purpose of this foundation document

This document captures what we should learn from Basecamp's Hotwire Native stack
when designing `foundation_platform`. Hotwire Native is not the target
architecture for this project. It is a mature reference point for server-driven
mobile applications, URL-driven navigation, WebView/native cooperation, and
bridge components.

The goal is to understand the architectural moves, not to copy the implementation
one-for-one.

## Core architectural idea

Hotwire Native turns a server-rendered web application into a native-feeling
mobile application by splitting responsibility three ways:

```text
Server
  owns URLs, HTML, forms, application state, authorization, screen content

Native shell
  owns navigation stacks, native presentation, route policy, device capability
  execution, and platform UI around the WebView

WebView
  renders HTML, runs Turbo/Stimulus, proposes navigations, and sends bridge
  messages to native
```

This is best understood as **URL-driven native navigation**, not simply “a website
inside a WebView”. The web application remains the semantic app. Native code
turns URL transitions into platform-native screen transitions.

A typical flow is:

```text
User taps link/form in WebView
        │
        ▼
Turbo/Hotwire JS intercepts and proposes a visit
        │
        ▼
Native Session receives proposal
        │
        ▼
Navigator applies path configuration and route policy
        │
        ▼
Native stack pushes/presents/replaces/pops a screen
        │
        ▼
WebView performs cold boot or Turbo JavaScript visit
        │
        ▼
HTML renders; bridge components may sync native UI/capabilities
```

The important insight is that navigation is not left to browser history alone.
The native shell interprets web navigation as app navigation.

## Global configuration model

Both iOS and Android expose a global Hotwire configuration surface. The exact
implementation differs, but the categories are consistent:

- application user agent configuration;
- default WebView/controller/fragment creation;
- path configuration sources;
- route decision handlers;
- webview policy decision handlers;
- bridge component registration;
- modal/navigation presentation defaults;
- logging/debug controls;
- JSON encoding/decoding behavior.

This matters for `foundation_platform` because a platform layer should also have
a central policy/configuration object. It should not scatter navigation, bridge,
cache, and security policy across ad-hoc commands.

## iOS architecture

### `Hotwire` and `HotwireConfig`

On iOS, `Hotwire` is the global entry point. It stores a `HotwireConfig`, bridge
component types, route decision handlers, webview policy handlers, and path
configuration sources.

`HotwireConfig` owns important application-wide policy:

- `applicationUserAgentPrefix`;
- modal/done-button behavior;
- back button display behavior;
- tab bar behavior;
- replace animation behavior;
- debug logging;
- path configuration;
- default view controller factory;
- default navigation controller factory;
- custom `WKWebView` factory;
- shared JSON encoder/decoder.

A notable design detail is that WebViews are produced through configuration. This
lets the framework inject shared behavior such as user agents, process pools, and
bridge scripts while still allowing the app to customize the concrete WebView.

### `Navigator`

The iOS `Navigator` coordinates navigation. It owns:

- a root `UINavigationController`;
- a modal navigation controller;
- a primary `Session`;
- a modal `Session`.

The two-session model is important: the normal stack and modal stack have
separate WebView/session contexts. Modal presentation is not just a CSS overlay;
it is native presentation.

A `Navigator` routes URLs or visit proposals by:

1. resolving path properties from path configuration;
2. consulting route decision handlers;
3. creating or resolving a controller;
4. passing the controller and proposal to the hierarchy controller.

### Navigation hierarchy behavior

The navigation hierarchy controller maps visit proposals to native navigation
operations. Presentations include patterns such as:

- default push/present;
- pop;
- replace;
- refresh;
- clear all;
- replace root;
- none/no-op.

This is one of Hotwire Native's strongest architectural contributions. Server or
path configuration can influence native presentation without rewriting screens in
Swift.

For `foundation_platform`, the lesson is not “copy UIKit navigation”. The lesson
is to introduce a routing model where URL/app route metadata can choose a native
presentation strategy.

### `Session`

A `Session` owns a single WebView and coordinates visits. It tracks:

- the current visit;
- topmost visit;
- previous visit;
- path configuration;
- whether the WebView has been initialized;
- a WebView bridge.

The key distinction is:

- the first page load is a **cold boot visit** using normal WebView navigation;
- later same-session navigations are **JavaScript visits** through Turbo.

This distinction allows the app to boot like a normal browser page and then
transition into an in-page Turbo-driven navigation system while still using
native navigation stacks.

### Shared WebView movement and screenshots

Hotwire Native commonly uses a shared WebView per session and moves it between
native view controllers. Controllers that are no longer active can display a
snapshot/screenshot. This produces native-feeling transitions without requiring a
separate WebView per screen.

The underlying insight is that the native stack can have more screen objects than
there are live WebViews.

For `foundation_platform`, this raises a design question: should a Tauri app use
one WebView per window, one WebView per navigation context, or multiple WebViews
for independent stacks? Tauri's model differs from UIKit/Android fragments, so we
should not assume the answer.

## Android architecture

Android mirrors the same conceptual layers with Android-native constructs:

- a global `Hotwire` object/configuration;
- core `Session` logic around Android WebView;
- navigation fragments for stack behavior;
- route/path configuration;
- bridge component registration;
- Kotlin classes for native capabilities.

The Android architecture reinforces that Hotwire Native's shared protocol is not
“Swift-specific”. The same web/server contract can drive different native stack
implementations.

Important implications:

- the server/web layer should not know whether it is driving iOS or Android;
- platform presentation policy lives in native configuration;
- bridge components need a stable cross-platform message contract;
- native implementations can differ internally while preserving the same web
  semantics.

For `foundation_platform`, this supports the idea of a cross-platform Rust-owned
policy layer above platform-specific details.

## Turbo bridge vs Native Bridge

Hotwire Native has two bridge concepts that should not be conflated.

### Turbo/navigation bridge

The Turbo bridge connects Turbo's JavaScript navigation lifecycle to native code.
It handles messages such as:

- page loaded;
- visit proposed;
- visit started;
- visit request started/completed/failed;
- visit rendered;
- visit completed;
- form submission started/finished;
- page invalidated;
- same-page anchor scroll;
- refresh.

This bridge exists so native code can participate in navigation lifecycle and
presentation decisions.

### Bridge Components

Bridge Components are a separate capability bridge. They let HTML/Stimulus code
request native behavior.

The general model is:

```text
HTML element + Stimulus/bridge controller
        │
        │ sends component/event/data/metadata
        ▼
Native bridge dispatcher
        │
        │ resolves registered native component by name
        ▼
Native component executes capability or updates native UI
        │
        │ optional reply/message back to web
        ▼
Web controller receives result
```

The message shape includes:

- message id;
- component name;
- event name;
- metadata, including URL/page identity;
- JSON data payload.

A particularly important correctness rule in the iOS source is that bridge
messages are gated by active destination and matching URL. Stale pages should not
be able to mutate the wrong native screen.

For `foundation_platform`, this suggests native capability messages should carry:

- component/capability name;
- event/action name;
- request id;
- route/page/screen identity;
- typed payload;
- permission context;
- optional response channel.

## Path configuration as server/native contract

Hotwire Native uses path configuration to map URLs to native behavior. A path can
carry properties such as:

- presentation style;
- whether pull-to-refresh is enabled;
- whether the screen should be modal;
- whether a route should be replaced/refreshed;
- whether a URL should be handled externally;
- native controller overrides.

This is one of the most valuable ideas for `foundation_platform`.

A Tauri-based platform can generalize it as **route metadata**:

```text
Route / URL / App location
        │
        ▼
Route metadata
        ├── presentation: push | replace | modal | root | external | none
        ├── source: bundled | remote | cache | local-protocol
        ├── offline policy: cache-first | network-first | online-only
        ├── auth policy
        ├── native capabilities allowed
        ├── transport preference: html | dom-ops | wasm | data
        └── window/webview policy
```

This route metadata could be loaded from local config, server config, or a merged
policy source.

## What Hotwire Native gets right

### Native navigation feel without native screen rewrites

Hotwire Native avoids the classic WebView app problem where every link is just a
browser page load. By letting native own the stack, users get platform-native
push/pop/modal behavior.

### Server-driven iteration

Because much of the UI lives on the server, many content and flow changes do not
require an app-store release.

### Clear separation of responsibilities

The server owns application state and HTML. Native owns navigation and device
capabilities. The WebView renders and runs web behavior.

### Bridge components are declarative

Native capabilities are invoked by declarative web-side components rather than
random JavaScript globals. This gives teams a manageable contract.

### URL identity is central

Route identity, screen identity, cache identity, and bridge message validity all
flow through URL/page identity.

## What Hotwire Native gives up

### Offline-first is not the default architecture

Hotwire Native assumes a live server-rendered application. Offline behavior can
be added, but it is not the core model. Without network access, many screens
cannot progress unless explicitly cached or made native.

### Large local data is not the strength

The WebView/server model works well for content and forms. It is less ideal for
heavy local datasets, large sync logs, local analytics, or high-throughput binary
payloads.

### Platform duplication exists

Deeper native behavior usually requires Swift and Kotlin implementations. The web
contract may be shared, but native capability code is duplicated by platform.

### Server is a runtime dependency

The core app behavior often depends on server reachability and server response
latency.

### The bridge is JSON/message based

Bridge Components are excellent for capability calls, but they are not designed
as a bulk data transport.

## What we should adopt

`foundation_platform` should adopt these ideas:

1. **Route-driven presentation policy** — URL/app route metadata should influence
   native/WebView presentation.
2. **Native shell owns presentation** — windows, modals, external links, and app
   lifecycle should not be left entirely to browser history.
3. **Declarative capability bridge** — UI should request native capabilities via
   named components/actions with typed payloads and permission policy.
4. **Active page identity checks** — capability messages and results should be
   scoped to the route/page/session that created them.
5. **Cold boot vs in-session navigation distinction** — initial load, cached
   replay, and same-session updates are different operations.
6. **Server-updatable route policy** — some presentation/cache/capability policy
   may come from the server, subject to local security limits.

## What we should not blindly copy

`foundation_platform` should not blindly copy:

1. **Server-only state** — our platform should support local-first and hybrid
   state.
2. **HTML-only update model** — `foundation_wasm_ui` can also use DomOps,
   columnar protocol, custom binary batches, real Arrow IPC, and WASM signals.
3. **Swift/Kotlin-first native capability implementation** — Tauri gives us a
   Rust platform layer. Platform-specific code should be isolated to plugins or
   narrow native bindings where required.
4. **Assumption of online navigation** — route policy must include offline/cache
   behavior.
5. **Bridge as bulk data transport** — large data should remain in Rust/cache/data
   lanes and expose renderable projections to the UI.

## Mapping to a Tauri + `foundation_wasm_ui` platform

A Tauri reinterpretation of Hotwire Native should look more like:

```text
Route/navigation request
        │
        ▼
foundation_platform route policy
        ├── choose WebView/window/modal/external handling
        ├── choose source: bundled, remote, cache, local protocol
        ├── choose rendering mode: WASM, HTML, DomOps, morph, island
        ├── choose offline behavior
        └── choose native capability permissions
        │
        ▼
Tauri WebView + foundation_wasm_ui runtime
        │
        ▼
Rust platform services: cache, sync, database, filesystem, native APIs
```

This preserves the Hotwire insight — native/platform shell participates in app
navigation — while using our existing Rust and `foundation_wasm_ui` foundations.

## Summary

Hotwire Native is valuable because it demonstrates a coherent server-driven,
URL-driven, native-feeling application architecture. Its strongest lessons are
navigation delegation, path configuration, bridge components, active page
identity checks, and the separation between server, WebView, and native shell.

For `foundation_platform`, we should treat it as inspiration for platform policy
and capability contracts, not as a replacement for `foundation_wasm_ui` or as a
mandate to build a server-only HTML app.
