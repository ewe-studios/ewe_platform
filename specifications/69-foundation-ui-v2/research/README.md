---
description: "Research for the foundation_ui v2 redesign: every idea in the rearchitecture document, laid out technically, with the current state and what the new way means. Research that leads to a design, not the design itself."
status: "research"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
source: "specifications/65-agent-rpc-serving-system/resources/foundation_ui_rearchitecture.md"
related_specs:
  - "specifications/completed/39-foundation-wasm-ui"
  - "specifications/completed/42-ui-component"
  - "specifications/52-tauri-foundation-platform"
  - "specifications/completed/36-agentic-api"
---

# Spec 69 research: foundation_ui v2

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); server-driven user interface (SDUI);
Document Object Model (DOM); virtual DOM (VDOM); WebAssembly (WASM); JavaScript (JS);
hash-based message authentication code (HMAC); pull request (PR); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Purpose

The owner wrote a rearchitecture document for the UI stack:
[`foundation_ui_rearchitecture.md`](../../65-agent-rpc-serving-system/resources/foundation_ui_rearchitecture.md).
This directory lifts **every idea** in that document into its own file and lays it out
technically. The aim is to improve on the document, not to bend it to the code we have.

Each idea file has the same five parts:

1. **The idea, laid out:** what the document says, as schemas, flows and diagrams, and what it
   buys.
2. **Current state:** what the repository has now, with file paths.
3. **The new way:** the target design, which takes the idea further where the document is
   thin or wrong.
4. **What this changes for us:** what is replaced, what stays, what is new.
5. **Open questions:** links to [`questions.md`](./questions.md).

The design specification comes after the owner answers the questions.

## 2. Owner decisions (fixed input)

| # | Decision |
|---|---|
| D1 | Research the ideas in the document first; the design follows. |
| D2 | No Tauri content for now. |
| D3 | The new direction **overrides** spec 36 decision 10, spec 52 and spec 39 decision 025 where they conflict. |
| D4 | **Native first.** Other rendering layers come later. |
| D5 | **State signing:** "Client never signs, its always from the server or the wasm bundle running on the client on the web – for on-device, this does not matter." |
| D6 | **Replay protection:** "Look at industry standard, at Livewire and learn and refine what works best for us." |
| D7 | (PR #157 review) **FlatBuffers is the top format.** Everything else, columnar included, is a type in the FlatBuffers schema, code-generated for every supported language. |

## 3. The ideas

| # | Idea (document section) | Current state | The new way |
|---|---|---|---|
| [01](./01-compile-time-targeting.md) | No VDOM, no hydration, compile-time registered elements (§1, §3) | True on the web (`primal-id`, `DomOp` by node id); `MorphNode` still diffs HTML | Ids become schema fields shared by every platform; `MorphNode` replaced by a typed subtree replace; ids stable across versions |
| [02](./02-unified-wire-contract.md) | One FlatBuffers contract for every frame (§1, §2) | 6-byte header, protocol byte table, hand-written bodies, no inbound schema | One `.fbs` schema; `flatc` generates Rust, TypeScript, Swift, Kotlin; `Frame` and `ActionRequest` with stream and sequence ids; `ClientHello` handshake |
| [03](./03-payload-tiers.md) | Three payload tiers: FlatBuffers ops, aligned columns, Arrow bulk data (§1, §2, §3) | Tier 2 by hand on the web; Arrow used for `DomOp` | All tiers are schema types (`OpBatch`, `ColumnarBatch`, `DataTable`); generated zero-copy readers; tier picked by the framework; Arrow IPC only as an escape hatch |
| [04](./04-vocabulary-and-interning.md) | Numeric tag and attribute ids with string fallback; compiled primitive families (§2, §6) | HTML wire id tables; HTML-shaped `DomOp` | Two interned vocabularies: native-first layout primitives with typed properties and design tokens, and the web vocabulary |
| [05](./05-web-zero-copy-applicator.md) | JS applicator reads frames and writes the DOM without WASM (§3) | Exists for server frames, with a hand-written parser | Generated reader, one apply loop for all tiers and for local WASM frames, sequence-ordered and batched per animation frame |
| [06](./06-native-rendering.md) | SwiftUI and Compose, no WebView, core compiled in (§3, §6) | Type stub only; no Swift, Kotlin or bindings | Node store with one observable record per node; platform-language readers on the render path; UniFFI for on-device Rust logic |
| [07](./07-element-riding-state.md) | State rides on elements, no client caches; eight state formats (§1, §2, §4) | Stateful server; decision 025 says the client never sends state | One `SignedState` per component root, generated body type, harvested without parsing; authenticated encryption for private state |
| [08](./08-state-security-and-replay.md) | HMAC with a rotating secret, verified by any replica (§4) | Private HMAC signer with no key id and a timing bug | Tag binds version, key id, times, component, node and session; key ring; ordered checks; layered replay protection from the [Livewire and industry study](./08b-replay-protection-study.md) |
| [09](./09-stateless-execution.md) | Any replica rehydrates, runs and forgets (§4) | One-thread stateful `App` | Two named modes: stateless actions (any replica) and live streams (owned connection); honest cost model with the database load |
| [10](./10-action-model.md) | `#[action]` macro, `Interaction<T>`, platform-adaptive `Response`, state-aware `html!` (§5) | Local handlers only; ConnectRPC `#[service]` macro exists | `async fn` with separate signed state and typed unsigned input; thin macro on the ConnectRPC machinery; per-client encoding; templates never sign; event wiring by compile-time action id |
| [11](./11-app-store-and-command-bus.md) | App Store 2.5.2: data not code, compiled primitives, declarative command bus (§6) | Spec 52 plans WASM hot-swap; components use scoped JS | No code fields in the schema; one generated action registry shared by server and device; version skew handled by the handshake; clear OTA rules |

## 4. Conflicts with earlier decisions

| Earlier decision | Why it gives way | Idea |
|---|---|---|
| Spec 36 decision 10 (`specifications/completed/36-agentic-api/decisions/10-serialization-format.md`): stored `foundation_ai` conversation data is JSON plus Arrow; the FlatBuffers fallback was dropped | It covers stored AI data, not the UI wire. FlatBuffers is the top format for the UI (D7). Q5 asks whether the same rule covers stored data. | [02](./02-unified-wire-contract.md) |
| Spec 39 decision 025 (`…/39-foundation-wasm-ui/decisions/025-server-client-state-sync.md:8`, `:40`): the client never sends state; nothing is embedded | State is embedded and returned, signed by the server (D5). The client still never authors state. | [07](./07-element-riding-state.md) |
| Spec 39 decision 027 and `DomOp::MorphNode` (`backends/foundation_ui_traits/src/dom_op.rs:159`) | An HTML-string diff breaks compile-time targeting and cannot run natively | [01](./01-compile-time-targeting.md) |
| `ViewKind::Native` "post-MVP" (`backends/foundation_ui_traits/src/platform_types.rs:180-187`) | Native is first (D4) | [06](./06-native-rendering.md) |
| Spec 52 decision 04, "WASM hot-swap, no review" (`specifications/52-tauri-foundation-platform/decisions/04-deployment-surfaces.md:32-34`) | Downloaded logic conflicts with App Store 2.5.2 for native apps | [11](./11-app-store-and-command-bus.md) |

## 5. Bugs found during this research

Per `CLAUDE.md` rule 9, this change adds one entry to
[`docs/bug-backlog.md`](../../../docs/bug-backlog.md): `foundation_auth` `TokenSigner::verify`
compares the HMAC tag in non-constant time and has no key id
(`backends/foundation_auth/src/shared/session.rs:384`).
