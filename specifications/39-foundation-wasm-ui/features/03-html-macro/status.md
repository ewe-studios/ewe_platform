# Feature 03 — Status: COMPLETE (2026-06-12)

## What shipped

- **`html!` proc macro** in `backends/foundation_macros/src/html_macro/`
  (parser.rs + codegen.rs + mod.rs), re-exported as `foundation_wasm_ui::html`.
  TokenTree walking only — zero runtime HTML parsing (decision 001); no
  template directives, control flow is plain Rust.
- **Two invocation forms**:
  - `html! { <div>..</div> }` — PURE: a typed `Html` expression. Slots
    evaluate once via `IntoHtml`; `Part` descriptors record dynamic positions.
  - `html! { ctx, receiver, <div>..</div> }` — REACTIVE: also allocates a
    runtime id block, queues the full DOM build as `DomOp`s on the
    `SharedInstructionReceiver` (F01 registry semantics: explicit
    `RegisterNode`, `AppendChild` after both ends exist, dedicated text node
    per slot so `SetText` never clobbers siblings), and creates ONE effect per
    slot/dynamic-attr. Effects run immediately (decision 008), so initial
    content arrives through them — each slot expression lives in exactly one
    closure.
- **Parser**: compound names (`primal:onclick`, `data-id`, `my-widget`), void
  elements, bare-text runs with punct-aware glue (`Count:` stays glued),
  quoted-string text, boolean attributes, every spec-table error with spans.
- **Numbering**: spec-visible element ids (depth-first, `primal-id`,
  `Part::node_id`) PLUS an all-nodes runtime index; reactive wire id =
  `__base + index` where `__base = ctx.allocate_id_block(total)` (new G20 API
  on `foundation_signals::Context`/`Runtime` — monotonic, never reused).
- **`MaybeCallback`** (G21) in `foundation_wasm_ui::html_macro`:
  `SignalSetter<T>` → `Some(callback_id)` → `primal:setter` attribute;
  `Fn(&EventData)` closures → `None` (wiring deferred to F08). The spec's
  `web_sys::Event` closure bound was replaced with our owned `EventData`.
- **`foundation_ui_traits::__macro`**: hidden alloc re-exports so generated
  code works in both `std` and `no_std` callers.

## Verification

- 10 parser unit tests INSIDE parser.rs covering spec error tests 35-39 +
  multiple-roots, unknown-primal, voids, text glue (in-file because
  `proc-macro = true` crates cannot export test hooks and exact-message
  asserts from `tests/` would need a compile-fail harness dependency).
- 26 integration tests in `foundation_wasm_ui/tests/html_macro_tests.rs`
  covering spec tests 1-34 + 40-42: structure/attrs/voids, depth-first ids
  (13-17), parts in document order incl. the spec-25 `[Text(0), Text(1),
  Text(0)]` case, attr-before-slot ordering, setter injection with real
  callback ids, distinct ids for two setters, closures clean, IntoHtml
  integration (primitives/Option/Vec), typed-Html specifics (known tags/attrs
  resolve to wire IDs, custom elements stay names), and the reactive form
  end-to-end on MockProtocol: build-op stream shape, initial slot render via
  immediate effect, re-render on `set()`, AddEventListener + primal:setter on
  the wire, dynamic attribute initial+update, disjoint id blocks across
  instances (G20).
- All four crates zero clippy warnings (`--all-targets`, uat); full test
  suites of macros/ui_traits/signals/wasm_ui green.

## Spec deviations (justified)

| Spec says | Shipped | Why |
|-----------|---------|-----|
| `crates/foundation_wasm_ui_macro/` companion crate | `foundation_macros::html` | Project rule: ALL proc macros live in foundation_macros — no companion macro crates. |
| `Html` with `String` fields | F01's typed `Html` (`HtmlTag`/`AttrName`/`Cow`) | F01 shipped the typed tree; codegen targets it (known names become wire ids). |
| Runtime prefix `"42:0"` strings + `prefix_ids` | `allocate_id_block` → wire id `base + index` | `DomOp`/NodeRegistry ids are `u32` (F01); string prefixes don't fit the wire. Block allocation gives the same per-instance disjointness in wire-compatible form. The `primal-id` attribute carries the final numeric id. |
| Slot effects `SetText` on the PARENT element | dedicated text node per slot | The spec's own test 25 (`{a}<span/>{b}`-style mixed content) breaks under parent-SetText — it would clobber sibling elements. Each slot owns a text node; `SetText` targets it. |
| Expression evaluated twice (Html + effect) | placeholder in Html; the immediate effect run renders initial content | Double evaluation would double-move captured handles (un-compilable for `move` closures). One closure per expression; handles used in MULTIPLE slots need per-slot clones (plain Rust rules — documented). |
| `mount(ctx, html, receiver, getters…)` variadic fn | mount logic INLINED by reactive codegen | The spec's signature (one parameter per Part) is not expressible as a single Rust fn; inlining is what its own §8 narrative describes. |
| `MaybeCallback` for `Fn(web_sys::Event)` | `Fn(&EventData)` | Owned runtime has no web_sys (decision 031); EventData is the F02/F08 event shape. |
| Reactive slots render arbitrary `Html` | v1 renders slot TEXT (via IntoHtml) | Non-text re-renders need diff/morph — F07's territory (`MorphNode`). Initial render of static Html/Vec slots works in the pure form; reactive rich-content slots are an F07 follow-up. |

## For downstream features

- F06: mount components via the reactive form; `Html.parts` still carries the
  descriptor list (compile-time element ids) for component-level tooling.
- F07: upgrade the slot effect to morph non-text content.
- F08: wire `primal:onX` closures (currently only setters get `primal:setter`)
  and read the `primal-id` attributes the build stamps.
