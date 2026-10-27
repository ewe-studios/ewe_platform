---
description: "Research for a future foundation_ui redesign: the ideas in the rearchitecture document, compared with the code and specifications that exist now. Research only, not a design."
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
hash-based message authentication code (HMAC); Secure Hash Algorithm 256 (SHA-256);
Document Object Model (DOM); WebAssembly (WASM); foreign function interface (FFI);
inter-process communication (IPC); pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Purpose

The owner wrote a rearchitecture document for the UI stack:
[`foundation_ui_rearchitecture.md`](../../65-agent-rpc-serving-system/resources/foundation_ui_rearchitecture.md)
(212 lines). A full specification from that document was too large a step.

This directory does one thing. It examines each idea in the document and compares it with
what the repository has now. For each idea, the file gives:

1. What the document proposes.
2. What we have now, with file paths and line numbers.
3. The gap.
4. Open risks.
5. A recommendation: **adopt**, **adapt** or **drop**, with the reason.

This is research. It is not a design. The owner discusses the result in the PR. The design
specification comes after the owner answers [`questions.md`](questions.md).

## 2. Scope

In scope:

- The ideas in the rearchitecture document that are worth study.
- A comparison with the completed spec 39 (`foundation_wasm_ui`), spec 42 (UI components),
  spec 52 (platform) and spec 36 decision 10 (serialization format).
- A short study of how Laravel Livewire, Phoenix LiveView and similar frameworks protect
  signed component state from tampering and replay.

Out of scope:

- **Tauri.** This research has no Tauri content. Spec 52 is named only where its decisions
  conflict with the new direction.
- Implementation, crate layout, feature tickets and task lists.
- Rendering layers other than native mobile. The web path is named only where the current
  code already does the work.

## 3. Owner decisions (fixed input)

These decisions are fixed. The research does not argue against them.

| # | Decision |
|---|---|
| D1 | Start with the ideas in the document only. Research, not design. |
| D2 | No Tauri content for now. |
| D3 | The new direction **overrides** spec 36 decision 10 (FlatBuffers fallback dropped), spec 52, and spec 39 decision 025 where they conflict. The files note each conflict. They do not defend the old decisions. |
| D7 | (PR #157 review) **FlatBuffers is the top format.** Everything else, columnar included, is a type in the FlatBuffers schema, code-generated for every supported language (Rust, TypeScript, Swift, Kotlin). |
| D4 | **Native first.** Other rendering layers come later. |
| D5 | **State signing:** "Client never signs, its always from the server or the wasm bundle running on the client on the web – for on-device, this does not matter." State is always signed by the server. |
| D6 | **Replay protection:** "Look at industry standard, at Livewire and learn and refine what works best for us." |

## 4. Files

| File | Idea |
|---|---|
| [`wire-envelope.md`](wire-envelope.md) | FlatBuffers as the top format: one schema, every body (columnar, native layout, actions) a generated type. |
| [`columnar-zero-copy.md`](columnar-zero-copy.md) | 8-byte aligned columnar blocks and zero-copy typed arrays in the browser. No virtual DOM. |
| [`native-mobile-rendering.md`](native-mobile-rendering.md) | SwiftUI and Jetpack Compose renderers fed by layout operations, core compiled in through UniFFI or C bindings. |
| [`stateless-signed-state.md`](stateless-signed-state.md) | Stateless servers. Each element carries its state and an HMAC state token that any replica can verify. |
| [`replay-protection.md`](replay-protection.md) | Study of Livewire, LiveView, ASP.NET ViewState and Hotwire. What we take. |
| [`action-macro.md`](action-macro.md) | `#[action]` handlers that return UI updates. |
| [`app-store-compliance.md`](app-store-compliance.md) | App Store Review Guideline 2.5.2: declarative payloads and a static on-device action registry. |
| [`questions.md`](questions.md) | Numbered open questions for the owner, with options and a recommendation. |

## 5. Summary of ideas

| # | Idea | What we have now | Recommendation |
|---|---|---|---|
| 1 | No virtual DOM, no client diff | Already true. `DomOp` targets ids from the `html!` macro. | **Adopt** (keep). No new work. |
| 2 | FlatBuffers as the top format | 6-byte `Envelope` header with a protocol byte; hand-written bodies. No schema, no sequence id, no typed union. | **Adopt (D7).** One `.fbs` schema for every frame, body and action; `flatc` generates Rust, TypeScript, Swift and Kotlin. |
| 3 | MessagePack payload | Not in the UI stack. `rmp-serde` is used only in `foundation_keychain`. | **Drop.** It has no schema, so it cannot be generated for every language (D7). |
| 4 | Columnar zero-copy typed arrays | Done in spec 39 feature 19 by hand (`ColumnarBatch`, JS `ColumnarParser`). | **Adopt, as a schema type (D7).** A FlatBuffers table of aligned typed vectors; generated readers give the same zero-copy views. Retire the hand-written code after a benchmark. |
| 5 | Native mobile rendering (SwiftUI, Compose) through UniFFI or C | Only the `ViewKind::Native` enum variant, marked post-MVP. No renderer, no bindings. | **Adapt.** This is the first build target (D4). Needs a native layout vocabulary that is not HTML. |
| 6 | Stateless servers, element-riding signed state | Opposite model: server `App` holds signals in memory. Decision 025 says the client never sends state. | **Adapt.** Adopt server-signed state tokens (D5). Keep the client input separate from the signed state. Overrides decision 025. |
| 7 | Replay protection | Nothing in the UI stack. `foundation_auth` has a private HMAC signer. | **Adopt a layered model** from the study: key id, expiry, session and action binding, constant-time compare, typed decode, optional single-use nonce. |
| 8 | `#[action]` macro returns UI updates | `primal:onX` binds to local signal setters. No server action macro. | **Adapt.** Make it an `async fn` with typed state and typed input. Return a typed response, not `Vec<u8>`. |
| 9 | App Store 2.5.2 by design | Spec 52 plans WASM hot-swap without review. | **Adopt** the declarative model and the static action registry. Overrides spec 52 hot-swap where they conflict. |

## 6. Conflicts with earlier decisions

Per D3, these decisions give way to the new direction. Each idea file gives the detail.

| Earlier decision | Conflict | File |
|---|---|---|
| Spec 36 decision 10, "FlatBuffers Fallback — dropped" (`specifications/completed/36-agentic-api/decisions/10-serialization-format.md:116-118`) | That decision is about how `foundation_ai` stores conversation data (JSON plus Arrow), not the UI wire. It is the only written position on FlatBuffers. FlatBuffers is now the top format for the UI (D7); Q19 asks if the same rule applies to the stored data. | `wire-envelope.md` |
| Spec 39 decision 025, "The client never tells the server its state" (`specifications/completed/39-foundation-wasm-ui/decisions/025-server-client-state-sync.md:8`) | The client now sends back a server-signed state token with each action. | `stateless-signed-state.md` |
| Spec 39 decision 025, "No signal values need to be synced or embedded" (same file, line 40) | State is embedded on elements. | `stateless-signed-state.md` |
| Spec 52 `ViewKind::Native` is post-MVP, "MVP renders everything in WebViews" (`backends/foundation_ui_traits/src/platform_types.rs:180-187`) | Native is first (D4). | `native-mobile-rendering.md` |
| Spec 52 decision 04, "WASM hot-swap, no review" (`specifications/52-tauri-foundation-platform/decisions/04-deployment-surfaces.md:32-34`, `:715`) | Conflicts with the 2.5.2 position: no downloaded executable code on device. | `app-store-compliance.md` |
| Spec 39 decision 027 and `DomOp::MorphNode` with HTML string content (`backends/foundation_ui_traits/src/dom_op.rs:159`) | A native renderer cannot apply an HTML string. | `native-mobile-rendering.md` |

## 7. Bugs found during this research

Per `CLAUDE.md` rule 9, this change adds one entry to
[`docs/bug-backlog.md`](../../../docs/bug-backlog.md):

- `foundation_auth` `TokenSigner::verify` compares the HMAC tag with a short-circuit
  iterator, not in constant time (`backends/foundation_auth/src/shared/session.rs:384`).
  The state-token signer in this research must not copy that code.
