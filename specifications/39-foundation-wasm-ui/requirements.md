---
description: "foundation_wasm_ui — Splits foundation_wasm into pure runtime/ABI (memory, binary messaging, function calling) and a new foundation_wasm_ui crate that owns ALL DOM/window/animation bindings. Includes a split JS SDK: foundation-wasm.js (standard communication) + foundation-wasm-ui.js (DOM interactions, animations, web components). Provides signal-based reactivity, no virtual DOM, Arrow-format batch messaging, and headless UI components. Includes foundation_auth_ui package for auth-specific components."
status: "pending"
priority: "high"
created: 2026-06-05
updated: 2026-06-05
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - ui-framework
    - wasm
    - web-components
    - signals
    - arrow
    - dom
    - javascript
    - rust
has_features: true
has_fundamentals: true
builds_on:
  - "specifications/21-http-framework"
  - "specifications/38-foundation-auth-server"
related_specs:
  - "specifications/21-http-framework"
  - "specifications/38-foundation-auth-server"
  - "specifications/37-overlay-vfs"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# foundation_wasm_ui — WASM-First UI Framework

## Overview

**TODO**: Alot of these is still vague in the features, we should be following our specification clarity expectations, read the specification management clarity requirements.


Build a WASM-first UI framework with a **clean crate and JS SDK split**:

- **foundation_wasm** stays as the pure runtime/ABI layer — memory management, binary message system, handle/pointer allocation, function calling API. No DOM, no window, no UI concepts and maybe refactor it as well so people can use different communication providers with clarity so both sides (wasm and js can agree), this way, someone may want just plain function calls over the wire without our custom binary message system, we need to review deeply and see how we can do this very well.
- **foundation_wasm_ui** is created as a new crate that owns ALL DOM/window/animation bindings. It depends on foundation_wasm and adds: signals, templates, components, Arrow-format DOM batching, web component bridge.
- The **JS SDK is split** into two files:
  - `foundation-wasm.js` — standard communication: memory allocation, batch encoding/decoding, function invoke, callbacks, timers (schedule, interval, animation frames), this becomes refactored to present our binary communication provider for the js side, refactored and well structured unlike the current messy js code to be easy to instantiate and use for communication supporting a multi format communication future.
  - `foundation-wasm-ui.js` — DOM-specific: element creation, attribute manipulation, event bridging, animation, web component registration, Arrow batch application, this then uses our binary commmunication and the new arrow communication system for where arrow should be used since we've now made it pluggable.

**No virtual DOM.** Reactive signals on both the Rust side and the DOM side target specific DOM nodes directly. Changes are batched in Arrow format for fast transfer, eliminating serialization costs.

## Research Sources & Inspirations

- **datastar** (`/home/darkvoid/Boxxed/@dev/repo-expolorations/src.datastar/datastar/markdown/`): Reactive signals, expression compiler, DOM morphing (idiomorph), SSE streaming, plugin system (attribute + action plugins), watchers
- **lit** (`/home/darkvoid/Boxxed/@dev/repo-expolorations/lit/exploration.md`): Tagged template literals, template caching by TemplateStringsArray identity, Part system (ChildPart, AttributePart, PropertyPart, EventPart), directive system, reactive property declarations, shadow DOM, SSR with declarative shadow DOM
- **stimulus** (`/home/darkvoid/Boxxed/@dev/repo-expolorations/stimulus/`): Controller pattern, action system (event → method dispatch), mutation observers, blessing system, value properties with type coercion
- **headlessui** (`/home/darkvoid/Boxxed/@dev/repo-expolorations/headlessui/exploration.md`): Unstyled accessible components, compound component pattern, render props, focus management, WAI-ARIA, keyboard navigation
- **astro/11ty**: Islands architecture, server-first rendering, progressive enhancement
- **livewire**: Server-driven UI with real-time DOM patching via SSE
- **tldraw**: Canvas-based rendering, collaborative state sync, fine-grained reactivity
- **r3**: Rendering pipeline, scene graph composition

## Architecture

### Crate Split

**TODO**: should `Animation Frame Hook`, hmmm, unsure node or other js runtime have that API, so should this not be moved to foundation_wasm_ui as well ?

```
foundation_wasm/                          foundation_wasm_ui/
┌──────────────────────────────┐          ┌──────────────────────────────┐
│ #![no_std]                   │          │ depends on foundation_wasm   │
│                              │          │                              │
│ Memory Management            │          │ Signal<T>, Computed<T>       │
│ Batch Encoding (ops.rs)      │          │ html! macro, Template, Parts │
│ Params / ReturnTypeHints     │          │ Component trait              │
│ ExternalPointer /            │          │ Controller, Directive        │
│   InternalPointer            │          │ ArrowBatch (DOM commands)    │
│ Function Registration        │          │ DomSignalBinding             │
│   (host_invoke_*)            │          │ EventSignalBinding           │
│ Callback Registry            │          │ Web Component Bridge         │
│ Timer Registry               │          │ JS Runtime: DOM applicator   │
│   (schedule, interval)       │          │ JS Runtime: Signal bridge    │
│ Animation Frame Hook         │          │ JS Runtime: Components       │
│                              │          │                              │
│ NO DOM concepts              │◄─────────┤ ALL DOM/window/animation     │
│ NO window/document           │          │   bindings live here         │
│ NO UI abstractions           │          │                              │
└──────────────────────────────┘          └──────────────────────────────┘
```

### JS SDK Split

**TODO**: Same, does other runtime provide this: `hook_up_animation_frames()`, if not lets move it to the other side.

```
foundation-wasm.js                          foundation-wasm-ui.js
┌──────────────────────────────┐          ┌──────────────────────────────┐
│                              │          │                              │
│ MemoryAllocations            │◄─────────┤ uses memory/batch from       │
│   create_allocation()        │          │   foundation-wasm.js         │
│   dispose_allocation()       │          │                              │
│                              │          │ ArrowParser                  │
│ Instructions encoder/        │          │   (TypedArray column views)  │
│   decoder                    │          │                              │
│                              │          │ ArrowDomApplicator           │
│ Function Registry            │          │   apply(buffer) → real DOM   │
│   register_function()        │          │   15 operation types         │
│   invoke_as_*()              │          │                              │
│   invoke_async()             │          │ NodeRegistry                 │
│                              │          │   (nodeId → DOM Element)     │
│ Callback Registry            │          │                              │
│   invoke_callback()          │          │ SignalBridge                 │
│   unregister_callback()      │          │   bindInput(), bindChange()  │
│                              │          │                              │
│ Timer Registry               │          │ ComponentRegistry            │
│   schedule_timeout()         │          │   register(tagName, ...)     │
│   schedule_interval()        │          │   WasmElement class          │
│   hook_up_animation_frames() │          │   lifecycle callbacks        │
│                              │          │                              │
│ Batch API                    │          │ SSEClient                    │
│   host_batch_apply()         │          │   server→DOM patches         │
│   host_batch_returning_apply()│         │                              │
│                              │          │ DOM Event Bridge             │
│                              │          │   register_dom_event()       │
│                              │          │   EventDispatcher            │
│                              │          │                              │
│ Rewritten for clarity        │          │ NEW — split from             │
│ and standard communication   │          │   foundation_wasm/runtime    │
└──────────────────────────────┘          └──────────────────────────────┘
```

### What Moves Where

**From foundation_wasm → foundation_wasm_ui:**

**TODO**: `host_cache_string`  string caching should be standard on the foundation_wasm side and the other side just ensures to support it, so everyone gets this as  this is important regardless of dom or not. So we should ensure to keep it in foundation_wasm and do it well if it needs to be refactored out of DOM specific places.


| Currently in foundation_wasm | Moves to |
|------------------------------|----------|
| `DOM_SELF`, `DOM_THIS`, `DOM_WINDOW`, `DOM_DOCUMENT`, `DOM_BODY` constants | `foundation_wasm_ui::dom::constants` |
| `allocate_dom_reference()` | `foundation_wasm_ui::dom::allocate_ref` |
| `invoke_for_dom()`, `invoke_for_object()` | `foundation_wasm_ui::dom::invoke` |
| `ReturnTypeId::DOMObject` | `foundation_wasm_ui::dom::DomReturnTypeId` |
| Animation frame callback registration (`register_animation_hook`, `trigger_animation_callbacks`) | `foundation_wasm_ui::animation` |
| `host_cache_string` (text caching for DOM) | `foundation_wasm_ui::text_cache` |
| `register_function`, `HostFunction` DOM-specific usage | `foundation_wasm_ui::js_functions` |
| JS runtime: DOM element creation, attribute setting | `foundation-wasm-ui.js` |
| JS runtime: event listener management | `foundation-wasm-ui.js` |
| JS runtime: animation frame handling | `foundation-wasm-ui.js` |

**Stays in foundation_wasm (no change):**

| Stays in foundation_wasm |
|--------------------------|
| MemoryAllocations (allocate, deallocate, get, clear) |
| Instructions encoder/decoder |
| Params enum (all types) |
| ReturnTypeHints / ReturnValues |
| ExternalPointer / InternalPointer |
| host_invoke_function (generic, not DOM-specific) |
| host_invoke_async_function |
| Callback registry (register_callback, invoke_callback) |
| Timer registry (schedule_timeout, schedule_interval) |
| `host_batch_apply` / `host_batch_returning_apply` |
| JS runtime: memory management, function registry, callback dispatch |

## Core Design Principles

*TODO**: This needs revision, I have consolidated my desired UI architecture in the initial design and thought process in ./learnings/primal-ui.md, so we should be reading that and talking about this different points you've listed here 1 by 1 and getting clarity on them, ask me as much questions, create different scenarios and why one way or the other might work better, lets get this right.

1. **WASM-first** — Component logic, state, reactivity all in Rust/WASM. JS is a thin host.
2. **No virtual DOM** — Signals on both sides target specific DOM nodes directly. Arrow batches apply changes in one shot.
3. **Arrow format for batching** — Zero-copy batch messages between WASM and DOM. No JSON serialization overhead.
4. **Web components as the boundary** — Custom elements with shadow DOM. Each component is a WASM-backed unit. 
5. **Signal-based reactivity** — Fine-grained subscriptions. Only affected nodes update. Inspired by datastar signals + lit's Part system.
6. **HTML templates via tagged literals** — Rust macro-based template syntax inspired by lit's `html\`` tagged templates. Template caching by identity.
7. **Compound component pattern** — Inspired by headlessui. Composable, accessible, unstyled components.
8. **foundation_wasm stays generic** — Low-level ABI (memory, batch, FFI). foundation_wasm_ui depends on it and adds UI concerns.
9. **Controller pattern** — Inspired by stimulus. Controllers attach to DOM elements, handle events, manage local state.
10. **Plugin system** — Attribute plugins (data-* bindings) and action plugins (on-click handlers). Extensible like datastar.
11. **Server-driven via SSE** — Server can push DOM patches over SSE (like livewire/datastar). WASM receives patches, applies to DOM.
12. **Islands architecture** — Server renders HTML, WASM hydrates interactive islands. Progressive enhancement.


## Crate Structure


```
backends/
├── foundation_wasm/              # Pure runtime/ABI — modified, DOM removed
│   ├── src/
│   │   ├── lib.rs                # #![no_std], re-exports
│   │   ├── base.rs               # ReturnTypeId, ReturnTypeHints, ReturnValues (stays)
│   │   ├── error.rs              # Error types (stays)
│   │   ├── frames.rs             # REMOVED — moves to foundation_wasm_ui
│   │   ├── intervals.rs          # Timer registry (stays — generic timers)
│   │   ├── jsapi.rs              # MODIFIED — DOM constants/funcs removed, generic stays
│   │   ├── mem.rs                # MemoryAllocations, MemoryId (stays)
│   │   ├── ops.rs                # Params, Instructions, batch encoding (stays)
│   │   ├── registry.rs           # Callback/pointer registry (stays)
│   │   ├── schedule.rs           # Schedule registry (stays)
│   │   └── wrapped.rs            # Wrappers (stays)
│   └── runtime/
│       └── foundation-wasm.js    # REWRITTEN — clean standard communication only
│           ├── MemoryAllocations class
│           ├── Instructions encoder/decoder
│           ├── FunctionRegistry (register_function, invoke_as_*)
│           ├── CallbackRegistry (register_callback, invoke_callback)
│           ├── TimerRegistry (schedule_timeout, schedule_interval)
│           └── Batch API (host_batch_apply, host_batch_returning_apply)
│
├── foundation_wasm_ui/           # NEW — ALL DOM/window/animation bindings
│   ├── Cargo.toml
│   │   [dependencies]
│   │   foundation_wasm = { workspace = true }
│   │   foundation_nostd = { workspace = true }
│   │   foundation_macros = { workspace = true }
│   │   serde, serde_json = { workspace = true }
│   │
│   └── src/
│       ├── lib.rs                # Feature-gated re-exports
│       ├── shared/               # wasm-compatible core
│       │   ├── mod.rs
│       │   ├── signal.rs         # Signal<T>, Computed<T>, Subscriptions
│       │   ├── template.rs       # html! macro, Template, Parts
│       │   ├── component.rs      # Component trait, ComponentHost
│       │   ├── controller.rs     # Stimulus-inspired controllers
│       │   ├── directive.rs      # Attribute/action directives
│       │   ├── arrow/            # Arrow-format DOM batching
│       │   │   ├── mod.rs
│       │   │   ├── schema.rs     # 15 DOM operation types
│       │   │   └── encode.rs     # Rust -> Arrow encoding
│       │   └── morph/            # DOM morphing (idempotent patch)
│       │       └── mod.rs
│       └── wasm/                 # wasm32-specific glue
│           ├── mod.rs
│           ├── dom/              # DOM bindings (moved from foundation_wasm)
│           │   ├── constants.rs  # DOM_SELF, DOM_WINDOW, etc.
│           │   ├── element.rs    # createElement, setAttribute, etc.
│           │   ├── event.rs      # Event listeners
│           │   └── query.rs      # querySelector
│           ├── animation.rs      # Animation frames (moved from foundation_wasm)
│           ├── text_cache.rs     # String interning (moved from foundation_wasm)
│           ├── dom_bridge.rs     # Signal <-> DOM bridge
│           └── custom_elements.rs # Web component registration
│   └── runtime/
│       └── foundation-wasm-ui.js # NEW — DOM-specific JS runtime
│           ├── ArrowParser (TypedArray views)
│           ├── ArrowDomApplicator (apply buffer -> real DOM)
│           ├── NodeRegistry (nodeId -> DOM Element)
│           ├── SignalBridge (DOM events -> WASM callbacks)
│           ├── ComponentRegistry (customElements.define)
│           ├── EventDispatcher (user actions -> WASM)
│           └── SSEClient (server -> DOM patches)
│
├── foundation_auth_ui/           # NEW — Auth UI components
│   ├── Cargo.toml
│   │   [dependencies]
│   │   foundation_wasm_ui = { workspace = true, features = ["full"] }
│   │   foundation_auth = { workspace = true }
│   └── src/
│       ├── lib.rs
│       ├── login_form.rs
│       ├── register_form.rs
│       ├── mfa_challenge.rs
│       ├── session_status.rs
│       ├── user_profile.rs
│       └── auth_layout.rs
│
├── foundation_ui_components/     # NEW — Headless UI component library
│   ├── Cargo.toml
│   │   [dependencies]
│   │   foundation_wasm_ui = { workspace = true, features = ["full"] }
│   └── src/
│       ├── lib.rs
│       ├── button.rs
│       ├── dialog.rs
│       ├── menu.rs
│       ├── tabs.rs
│       ├── popover.rs
│       ├── disclosure.rs
│       ├── combobox.rs
│       ├── listbox.rs
│       ├── switch.rs
│       ├── checkbox.rs
│       ├── input.rs
│       ├── toast.rs
│       ├── skeleton.rs
│       └── transitions/
└── ...
```

## Feature Index

| Feature | Description | Phase | Status |
|---------|-------------|-------|--------|
| [01-wasm-ui-core](features/01-wasm-ui-core/) | Crate split: foundation_wasm DOM removed, foundation_wasm_ui created with all UI bindings | 1 | pending |
| [02-signal-system](features/02-signal-system/) | Signal<T> reactive system — Rust signals, DOM signals, bidirectional bridge, fine-grained subscriptions | 1 | pending |
| [03-html-templates](features/03-html-templates/) | html! macro, template compilation, Part system (ChildPart, AttributePart, PropertyPart, EventPart), directive support | 2 | pending |
| [04-dom-batching-arrow](features/04-dom-batching-arrow/) | Arrow-format DOM batching — schema, encoding (Rust -> Arrow), decoding + DOM apply (JS), zero-copy transfer | 1 | pending |
| [05-web-component-base](features/05-web-component-base/) | Web component base class, shadow DOM, lifecycle (connected, disconnected, attributeChanged), custom element registration | 2 | pending |
| [06-js-runtime-core](features/06-js-runtime-core/) | Split JS SDK: foundation-wasm.js (rewritten standard comm) + foundation-wasm-ui.js (DOM, animation, components) | 1 | pending |
| [07-auth-ui-package](features/07-auth-ui-package/) | foundation_auth_ui — login form, register form, MFA challenge, session status, user profile, auth layout | 3 | pending |
| [08-headless-ui-components](features/08-headless-ui-components/) | foundation_ui_components — headless accessible components: button, dialog, menu, tabs, popover, combobox, etc. | 4 | pending |

## Big TODOs

1. Lets review all the todos across all features and this requirement file.
2. I want to lean more into the core ideas of primal-ui in specifications/39-foundation-wasm-ui/learnings/primal-ui.md (but we need to make it clear what exactly we are bring in and what is out)
3. Its now clear, i would like to expand our codegen tooling (foundation_codegen) with capability javascript and typescript generation capabilities after seeing how web-rs does this (see specifications/39-foundation-wasm-ui/learnings/web-gen-ts-binding-generation.md)
4. Also its clear i want to refactor and make communication protocol aware, so that interactions always start with protocol and version starters in the messages sent back and forth to support multiple protocol and versions e.g our current custom binary protocol and arrow messages.
5. Its seems reasonable to also have some central fetch wrapper that knows how to batch API requests to reduce the thundering heard problem and use this everywhere so that we can control and better manage outgoing requests and it wrapping the fetch allows us to be smart in how this works, how long it waits to batch or if it batches based on how many times it gets triggered in the shortest amount of time, we need to think about this, research and see what others do or if this is even a good idea.
6.Its clear we want to be smart with how we define our WebComponent setup and not go crazy creating many different types but instead a specific set of types which understand how to interact in some specific way e.g mount-api, mount-stream, mount-data, we need to clearly define this, how they work and create a generic web component where these build on and doing it well will allow them just automatically work since we move e.g http communication to a service worker when its available and transparently owns the communication and responds and properly proxies to the server, but for this like web-workers, we might want to maybe add a mount-from-worker (to indicate this is coming from a webworker? I am unsure if this is a good idea) or if there is something we can do to indicate via mount-api, mount-stream, mount-data if its going to a web worker which might be better, I think i like this better, users can probably add a `worker=name-of-worker` and a central system that knows the web-workers (probably our webworkers) add them selves to some list and then the name just cleaning map and uses the worker communication proxy to deliver the messages to it and workers send back their response to them - we figure the right way to identify whoes response hook will get the reply.
7. Its clear we want to support: direct invocation, web-worker execution, service workers (when possible, which will allow isomorphic http endpoints that get intercepted before they go to the server or remote endpoint) and so need to think more about how this should work.
  a. I was thinking just like we do with the #[wasm_bin] proc macro, we can mark functions further that specific use #[wasm_bin], new proc macros that indicate how its going to be executed:
    - `#[wasm_bin]` — regular WASM function, executed in the main thread
      - #[wasm_bin(js=single-file, encoded=b64|uint8array)] - generates also a js wrapper which will encoded the generate wasm beside it as a single js file and by default add it as a Uint8Array else base64 encoded data with the needed logic to decode and initialize it.
    - `#[wasm_worker]` — executed in a web worker and also will generate a js wrapper for it and could have a marker js=single-file to indicate when present to not just generate a wasm but then create a js file which will base64 encode the wasm into the js file and setup the necessary logic to have it running which can be served like a regular file and if not then it automatically assumes where ever its (the js) is served, it will just ask the server for the wasm file in the web-worker.
    - `#[wasm_worker(js=single-file, encoded=b64|uint8array)]` — executed in a web worker and also will generate a js wrapper file will base64 encode the wasm into the js file and setup the necessary logic to have it running which can be served like a regular file and if not then it automatically assumes where ever its (the js) is served, it will just ask the server for the wasm file in the web-worker. When the js property is present then we look for encoded which by default is `uint8array` where we just store the raw bytes in a Uint8Array (see specifications/39-foundation-wasm-ui/learnings/wasm-delivery.md) and letting the server compress it. 
    - `#[wasm_service]` — executed in a service worker - which will let users present a fetch endpoint (yes we are stealing from cloudflare) which lets us present a http endpoint to fetch content and a route() method that returns the routes the service worker should scope for going to the wasm else passing them along to the server.
    - `#[wasm_service(js=single-file, encoded=b64|uint8array)]` — executed in a service worker and following the same semantics as #[wasm_worker] to support how its encoded into the single file when we generate it.


## Module References

- `backends/foundation_wasm/` — existing low-level ABI, will be stripped of UI-specific concerns
- `backends/foundation_http/` — SSE streaming for server-driven UI
- `backends/foundation_auth/` — auth client library (foundation_auth_ui depends on this)
- `backends/foundation_db/` — Arrow serialization support (foundation_arrow planned)
- `backends/foundation_core/` — valtron for background processing

## Language Stack

- **Rust** — WASM component logic, signal system, template compilation, Arrow encoding
- **JavaScript** — Two runtimes: foundation-wasm.js (standard comm) + foundation-wasm-ui.js (DOM, components)
- **Web Components** — Custom elements, shadow DOM, slots

## Success Criteria

- [ ] foundation_wasm compiles with DOM concepts removed, no_std preserved
- [ ] foundation_wasm_ui compiles, depends on foundation_wasm without circular deps
- [ ] foundation-wasm.js rewritten — clean standard communication only
- [ ] foundation-wasm-ui.js created — all DOM operations, animations, web components
- [ ] Signal<T> supports Rust-side reactivity with fine-grained subscriptions
- [ ] html! macro compiles to template with Parts for dynamic expressions
- [ ] Arrow batch encoding sends DOM mutations in single host_batch_apply call
- [ ] JS runtime parses Arrow batches and applies to real DOM (no vDOM)
- [ ] Web component base class supports shadow DOM, lifecycle, attribute observation
- [ ] Bidirectional signal bridge: Rust signal change -> DOM update, DOM event -> Rust handler
- [ ] foundation_auth_ui login form connects to foundation_auth server endpoints
- [ ] Headless UI components pass accessibility tests (keyboard nav, ARIA, focus management)
- [ ] Server SSE stream delivers DOM patches that WASM applies incrementally
- [ ] Template caching by identity — same template string parsed only once

---

_Created: 2026-06-05_
