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

Authoritative per-feature status lives in `features/NN-*/status.md`; this table
is the at-a-glance view (kept in sync as features complete).

| Feature | Description | Phase | Status |
|---------|-------------|-------|--------|
| [00-foundation-wasm-refactor](features/00-foundation-wasm-refactor/) | megatron.js 1:1 port → foundation-wasm.js / foundation-wasm-ui.js; crate split (pure ABI vs UI) | 1 | **COMPLETE** (2026-06-11) |
| [01-foundation-ui-traits](features/01-foundation-ui-traits/) | Shared types crate: IntoHtml/Html/Part, 19-op DomOp, HtmlTag/AttrName wire ids, ProtocolEncoder + Arrow/JSON encoders, Envelope | 1 | **COMPLETE** (2026-06-11) |
| [02-signal-system](features/02-signal-system/) | foundation_signals: R3-style reactive graph — height-ordered bucket queue, ThreeState + Check short-circuit, (getter, setter) tuples, Context disposal, callback registry | 1 | **COMPLETE** (2026-06-12) |
| [03-html-macro](features/03-html-macro/) | html! macro: pure-Rust parse, Part emission, primal-id assignment, setter callback wiring | 2 | **COMPLETE** (2026-06-12) |
| [04-instruction-receiver](features/04-instruction-receiver/) | InstructionReceiver: DomOp queue/flush/ack over the protocol layer (decision 030) | 1 | **COMPLETE** (2026-06-12) |
| [05-arrow-encoding](features/05-arrow-encoding/) | Real Arrow IPC RecordBatch encoding behind ProtocolEncoder (no_std columnar layout shipped in F01 as the interim) | 1 | pending |
| [06-web-components](features/06-web-components/) | Island/mount web components, execution modes (decisions 013/014/021/023/024) | 2 | pending |
| [07-dom-morphing](features/07-dom-morphing/) | Datastar-style morphing (decision 027) behind MorphNode (minimal application shipped in F01 JS) | 2 | pending |
| [08-event-runtime](features/08-event-runtime/) | primal:on* wiring → EventData → invoke_callback → stabilize (JS side partially shipped: scanAndWire/EventDispatcher) | 2 | pending |
| [09-scoped-styles-theme](features/09-scoped-styles-theme/) | Scoped script/style tags + theme system (decisions 019/020) | 3 | pending |
| [10-build-pipeline](features/10-build-pipeline/) | Build pipeline (decision 016) | 3 | pending |
| [11-request-batching](features/11-request-batching/) | Fetch bundling (decision 026) | 3 | pending |
| [12-testbed-native-harness](features/12-testbed-native-harness/) | wasm-testbed node/deno/web runners on the owned runtime | 1 | **COMPLETE** (2026-06-11) |
| [13-wasm-test-native](features/13-wasm-test-native/) | `#[wasm_test]` + `__fwt_` discovery + host_report protocol | 1 | **COMPLETE** (2026-06-11) |
| [14-wasm-bindgen-interop-boundary](features/14-wasm-bindgen-interop-boundary/) | wasm-bindgen quarantined to explicit opt-ins; owned modules import only `abi` | 1 | **COMPLETE** (2026-06-11) |
| [15-typesafe-wasm-wat](features/15-typesafe-wasm-wat/) | wasmbin port → foundation_codegen::wasm + WAT + CLI | 1 | **COMPLETE** (2026-06-11) |
| [16-walrus-transform-port](features/16-walrus-transform-port/) | **DEFERRED** — walrus IR port; revisit when heavy structural rewrites are needed | 1 | deferred |
| [17-abi-function-call-codec](features/17-abi-function-call-codec/) | WASM↔JS function-call ABI codec, two-way port | 1 | **COMPLETE** (2026-06-11) |
| [18-valtron-signals-bridge](features/18-valtron-signals-bridge/) | Cross-thread signals over valtron: SignalHub actor, RemoteGetter/RemoteSetter, post-stabilize snapshot visibility (glitch-freedom across threads) | LAST | design ready for review (2026-06-12) |

> Features 12–17 + decision 031 were added 2026-06-10 (own WASM infra end-to-end; wasm-bindgen only at explicit integration points; wasmbin ported for type-safe edits; walrus port deferred).


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
